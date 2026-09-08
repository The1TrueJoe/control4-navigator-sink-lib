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
use crate::model::{Device, Project, ProxyKind, Room};
use crate::rest::{proxy_kind_from_str, room_var, Item, RoomInfo, Variable};
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
    let items: Vec<Item> = get(src, "/api/v1/items")?;

    let mut project = Project::default();
    for r in rooms {
        project.rooms.insert(
            r.id,
            Room { id: r.id, name: r.name, floor: r.floor_name, ..Default::default() },
        );
    }
    for it in items {
        if !it.is_device() {
            continue;
        }
        let dev = Device {
            id: it.id,
            name: it.name.clone(),
            proxy: it
                .proxy
                .as_deref()
                .map(proxy_kind_from_str)
                .unwrap_or_else(|| ProxyKind::Other(String::new())),
            room_id: it.room_id.or(it.parent_id),
            props: Default::default(),
        };
        // Devices live under their room; parentId is the room for room-scoped items.
        match dev.room_id.and_then(|rid| project.rooms.get_mut(&rid)) {
            Some(room) => {
                room.devices.insert(dev.id, dev);
            }
            None => {
                project.devices.insert(dev.id, dev);
            }
        }
    }
    Ok(project)
}

/// Fetch one item's variables (`/api/v1/items/:id/variables`) — the live-state bus.
pub fn load_room_variables(src: &dyn ProjectSource, room_id: u32) -> Result<Vec<Variable>> {
    get(src, &format!("/api/v1/items/{room_id}/variables"))
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
