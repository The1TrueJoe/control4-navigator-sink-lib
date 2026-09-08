//! Live-sync loader (Strategy A): build a [`Project`] from the controller's REST
//! API and fold live variable state into it.
//!
//! Transport-agnostic: you supply a [`ProjectSource`] that performs the actual
//! HTTPS GET (with the self-signed cert accepted and the JWT attached — see
//! `docs/navigator-data-model.md`). The lib stays dependency-light; the messy
//! TLS/JWT lives in the caller (e.g. the `control4-fake-navigator` backend).
//!
//! This is a **sync**, not a one-shot preset: re-run [`load_project`] / poll
//! [`load_room_variables`] (or drive from `/api/v1/subscriptions`) so
//! "Refresh Navigators" and project edits are always reflected.

use crate::error::{Error, Result};
use crate::model::{Device, Project, ProxyKind, Room, Source};
use crate::rest::{proxy_kind_from_str, room_var, Favorite, Item, RoomInfo, RoomMedia, Variable};
use serde::de::DeserializeOwned;

/// Something that can GET a controller API path and return parsed JSON.
///
/// Implement this over your HTTP client. Paths are absolute API paths like
/// `"/api/v1/rooms"`.
pub trait ProjectSource {
    fn get_json(&self, path: &str) -> Result<serde_json::Value>;
}

fn get<T: DeserializeOwned>(src: &dyn ProjectSource, path: &str) -> Result<T> {
    let v = src.get_json(path)?;
    serde_json::from_value(v).map_err(|source| Error::Parse { line: path.to_string(), source })
}

/// Fetch `/api/v1/rooms` + `/api/v1/items` and assemble a [`Project`]
/// (rooms, and devices placed in their rooms).
pub fn load_project(src: &dyn ProjectSource) -> Result<Project> {
    let rooms: Vec<RoomInfo> = get(src, "/api/v1/rooms")?;
    // `?tree=false` returns a flat list carrying proxy/room/categories on devices.
    // Parse per-item and skip anything that doesn't fit, so schema drift or an odd
    // agent/root entry never fails the whole sync.
    let items_val = src.get_json("/api/v1/items?tree=false")?;
    let items: Vec<Item> = items_val
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| serde_json::from_value::<Item>(v.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    let mut project = Project::default();
    for r in rooms {
        project.rooms.insert(
            r.id,
            Room { id: r.id, name: r.name, floor: r.floor_name, ..Default::default() },
        );
    }
    // Each real device appears twice: a "(Universal)" lua_gen minidriver wrapper and
    // the proxy instance. Drop the wrappers, then dedup same-name pairs preferring
    // the proxy (non-lua_gen) item — so the grid shows one clean, friendly name.
    let mut devs: Vec<(Device, bool)> = Vec::new(); // (device, is_lua_gen)
    for it in items {
        if !it.is_device() {
            continue;
        }
        if it.name.trim_end().ends_with("(Universal)") {
            continue;
        }
        let is_lua = it.control.as_deref() == Some("lua_gen");
        devs.push((
            Device {
                id: it.id,
                name: it.name.clone(),
                proxy: it
                    .proxy
                    .as_deref()
                    .map(proxy_kind_from_str)
                    .unwrap_or_else(|| ProxyKind::Other(String::new())),
                room_id: it.room_id.or(it.parent_id),
                props: Default::default(),
            },
            is_lua,
        ));
    }
    // dedup by (room, lowercased name): keep the non-lua_gen when they collide
    let mut chosen: std::collections::HashMap<(Option<u32>, String), usize> = Default::default();
    let mut keep = vec![true; devs.len()];
    for idx in 0..devs.len() {
        let key = (devs[idx].0.room_id, devs[idx].0.name.to_lowercase());
        match chosen.get(&key).copied() {
            Some(prev) => {
                if devs[prev].1 && !devs[idx].1 {
                    keep[prev] = false;
                    chosen.insert(key, idx);
                } else {
                    keep[idx] = false;
                }
            }
            None => {
                chosen.insert(key, idx);
            }
        }
    }
    for (idx, (dev, _)) in devs.into_iter().enumerate() {
        if !keep[idx] {
            continue;
        }
        match dev.room_id.and_then(|rid| project.rooms.get_mut(&rid)) {
            Some(room) => {
                room.devices.insert(dev.id, dev);
            }
            None => {
                project.devices.insert(dev.id, dev);
            }
        }
    }

    // Watch/Listen sources, per room, straight from the controller's own navigator
    // lists (`/rooms/:id/media`). This is authoritative — correct names, correct
    // menu split, and the user's hidden-source choices — so we don't guess A/V
    // menus from proxy types. Best-effort per room; a failure leaves that room's
    // lists empty rather than failing the whole sync.
    let room_ids: Vec<u32> = project.rooms.keys().copied().collect();
    for rid in room_ids {
        if let Ok(media) = load_room_media(src, rid) {
            if let Some(room) = project.rooms.get_mut(&rid) {
                room.watch = sources_from(media.watch_devices.visible);
                room.listen = sources_from(media.listen_devices.visible);
            }
        }
    }

    // Favorites (best-effort — endpoint may be absent on some controllers).
    if let Ok(v) = src.get_json("/api/v1/agents/ui_configuration/favorites/") {
        if let Some(arr) = v.get("favorites").and_then(|f| f.as_array()) {
            for fv in arr {
                if let Ok(fav) = serde_json::from_value::<Favorite>(fv.clone()) {
                    if let Some((room_id, item_id)) = fav.as_item() {
                        if let Some(r) = project.rooms.get_mut(&room_id) {
                            if !r.favorites.contains(&item_id) {
                                r.favorites.push(item_id);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(project)
}

/// Fetch a room's Watch/Listen source lists (`/api/v1/rooms/:id/media`).
pub fn load_room_media(src: &dyn ProjectSource, room_id: u32) -> Result<RoomMedia> {
    get(src, &format!("/api/v1/rooms/{room_id}/media"))
}

/// Convert controller media entries into model [`Source`]s: drop special entries
/// with non-positive ids (e.g. `[Zones]` `-998`, `[Now Playing]` `-997`) and
/// dedup by id, preserving the controller's order.
fn sources_from(entries: Vec<crate::rest::MediaSource>) -> Vec<Source> {
    let mut seen = std::collections::HashSet::new();
    entries
        .into_iter()
        .filter_map(|m| {
            let id = u32::try_from(m.id).ok().filter(|&id| id > 0)?;
            if !seen.insert(id) {
                return None;
            }
            Some(Source {
                id,
                audio_video: m.is_audio_video(),
                name: m.name,
                kind: m.kind.unwrap_or_default(),
            })
        })
        .collect()
}

/// Fetch one item's variables (`/api/v1/items/:id/variables`) — the live-state bus.
pub fn load_room_variables(src: &dyn ProjectSource, room_id: u32) -> Result<Vec<Variable>> {
    get(src, &format!("/api/v1/items/{room_id}/variables"))
}

/// Fold a selected source device's own variables into the room's [`NowPlaying`].
///
/// Device variables are **driver-specific** and their ids even collide within one
/// item (e.g. a Roku TV has two `1003`s: `CURRENT_APP` and `CURRENT_INPUT`), so we
/// match by variable **name**, case-insensitively, taking the first non-empty
/// string value. There is no single universal now-playing variable across proxies.
/// Returns `true` if anything changed.
pub fn apply_now_playing(room: &mut Room, device_vars: &[Variable]) -> bool {
    let before = room.now_playing.clone();

    let find = |names: &[&str]| -> Option<String> {
        device_vars.iter().find_map(|v| {
            let n = v.var_name.to_ascii_uppercase();
            if !names.iter().any(|w| n == *w) {
                return None;
            }
            let s = v.value.as_str().map(str::to_string)?;
            let s = s.trim();
            (!s.is_empty()).then(|| s.to_string())
        })
    };

    let np = &mut room.now_playing;
    np.title = find(&["MEDIA_TITLE", "CURRENT_TITLE", "SONG_TITLE", "NOW_PLAYING_TITLE", "TITLE"]);
    np.artist = find(&["MEDIA_ARTIST", "CURRENT_ARTIST", "ARTIST"]);
    np.album = find(&["MEDIA_ALBUM", "CURRENT_ALBUM", "ALBUM"]);
    np.art_url = find(&[
        "MEDIA_ART_URL", "ALBUM_ART_URL", "COVER_ART_URL", "CURRENT_MEDIA_URL", "ART_URL", "IMAGE_URL",
    ]);
    np.app = find(&["CURRENT_APP", "CURRENT_STATION", "CURRENT_CHANNEL", "CURRENT_SOURCE"]);
    np.state = find(&["CURRENT_PLAYBACK_STATE", "PLAYBACK_STATE", "PLAY_STATE", "MEDIA_STATE"]);
    np.transports = find(&["TRANSPORTS_SUPPORTED"])
        .map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
        .unwrap_or_default();

    room.now_playing != before
}

/// Clear a room's now-playing rich fields (keeps `source_device`) — call when the
/// room is off so stale metadata doesn't linger.
pub fn clear_now_playing(room: &mut Room) -> bool {
    let before = room.now_playing.clone();
    let src = room.now_playing.source_device;
    room.now_playing = crate::model::NowPlaying { source_device: src, ..Default::default() };
    room.now_playing != before
}

/// Fold room variables into a [`Room`]'s live state (power, volume, mute, selected
/// device). Returns `true` if anything changed.
pub fn apply_room_variables(room: &mut Room, vars: &[Variable]) -> bool {
    let before = room.clone();
    for v in vars {
        match v.variable_id {
            room_var::POWER_STATE => room.power_on = v.as_bool().unwrap_or(room.power_on),
            room_var::CURRENT_VOLUME => room.volume = v.as_i64().map(|n| n.clamp(0, 100) as u8),
            room_var::IS_MUTED => room.is_muted = v.as_bool().unwrap_or(room.is_muted),
            room_var::CURRENT_SELECTED_DEVICE => {
                room.now_playing.source_device = v.as_i64().map(|n| n as u32).filter(|&x| x != 0)
            }
            room_var::CURRENT_VIDEO_DEVICE => {
                room.current_video_device = v.as_i64().map(|n| n as u32).filter(|&x| x != 0)
            }
            room_var::CURRENT_AUDIO_DEVICE => {
                room.current_audio_device = v.as_i64().map(|n| n as u32).filter(|&x| x != 0)
            }
            _ => {}
        }
    }
    *room != before
}
