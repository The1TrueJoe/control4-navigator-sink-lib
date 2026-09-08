//! Types for the controller's **public REST API** (`broker` `/api/v1/*`).
//!
//! These `serde` structs match the JSON captured from a live EA-3 (see
//! `docs/navigator-data-model.md` and the fixtures under `tests/fixtures/`). They
//! let a consumer (e.g. `control4-fake-navigator`) fetch the project structure and
//! live variable state and deserialize it straight into typed values, then lift it
//! into the transport-agnostic [`crate::model`] types.
//!
//! This module is data-only — it does not perform HTTP (bring your own client and
//! JWT; see the data-model doc for how tokens are minted).

use crate::model::ProxyKind;
use serde::{Deserialize, Serialize};

/// A node in `GET /api/v1/locations` (a tree). `type_`: 2 site, 3 building,
/// 4 floor, 8 room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    pub id: u32,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: u32,
    #[serde(default)]
    pub type_name: Option<String>,
    #[serde(default)]
    pub children: Vec<Location>,
}

/// An entry in `GET /api/v1/rooms`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomInfo {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub site_id: Option<u32>,
    #[serde(default)]
    pub site_name: Option<String>,
    #[serde(default)]
    pub building_id: Option<u32>,
    #[serde(default)]
    pub building_name: Option<String>,
    #[serde(default)]
    pub floor_id: Option<u32>,
    #[serde(default)]
    pub floor_name: Option<String>,
    #[serde(default)]
    pub parent_id: Option<u32>,
    #[serde(default)]
    pub type_name: Option<String>,
    #[serde(default)]
    pub room_hidden: bool,
}

/// One proxy a device presents (`proxyMeta[]` on an item detail).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyMeta {
    #[serde(default)]
    pub proxybindingid: Option<u32>,
    pub proxy: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub small_image: Option<String>,
    #[serde(default)]
    pub large_image: Option<String>,
}

/// A full item from `GET /api/v1/items/:id` (returned as a one-element array) or
/// `GET /api/v1/items` (list form — most fields optional there).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: u32,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: u32,
    pub type_name: String,
    #[serde(default)]
    pub parent_id: Option<u32>,
    /// Driver control name (e.g. `control4_ea3`, `tv`, `lua_gen`).
    #[serde(default)]
    pub control: Option<String>,
    /// Primary proxy type (device items).
    #[serde(default)]
    pub proxy: Option<String>,
    #[serde(default)]
    pub proxy_meta: Vec<ProxyMeta>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub room_id: Option<u32>,
    #[serde(default)]
    pub room_name: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub version: Option<u32>,
}

impl Item {
    /// Map the primary `proxy` string to a [`ProxyKind`].
    pub fn proxy_kind(&self) -> Option<ProxyKind> {
        self.proxy.as_deref().map(proxy_kind_from_str)
    }

    pub fn is_device(&self) -> bool {
        self.type_name == "device"
    }
    pub fn is_agent(&self) -> bool {
        self.type_name == "agent"
    }
}

/// A variable from `GET /api/v1/items/:id/variables` — the live state bus. For a
/// room, `id` is the room id and `variable_id` is the well-known id (1000
/// CURRENT_SELECTED_DEVICE, 1010 POWER_STATE, 1011 CURRENT_VOLUME, 1018 IS_MUTED,
/// 1019 IN_NAVIGATION, …). `value` is number|string|bool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub id: u32,
    pub var_name: String,
    pub variable_id: u32,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    pub value: serde_json::Value,
    #[serde(default)]
    pub hidden: Option<u8>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub room_name: Option<String>,
}

impl Variable {
    /// `value` as i64 if it is (or parses as) an integer.
    pub fn as_i64(&self) -> Option<i64> {
        self.value
            .as_i64()
            .or_else(|| self.value.as_str().and_then(|s| s.parse().ok()))
    }
    /// `value` as bool (Control4 often encodes bools as 0/1 or "True"/"False").
    pub fn as_bool(&self) -> Option<bool> {
        self.value
            .as_bool()
            .or_else(|| self.as_i64().map(|n| n != 0))
            .or_else(|| match self.value.as_str() {
                Some(s) => match s.to_ascii_lowercase().as_str() {
                    "true" | "1" => Some(true),
                    "false" | "0" => Some(false),
                    _ => None,
                },
                None => None,
            })
    }
}

/// One selectable source in `GET /api/v1/rooms/:id/media` — an entry in a room's
/// Watch or Listen list, exactly as Control4's own navigator shows it. `id` can be
/// negative for special audio entries (`-998` `[Zones]`, `-997` `[Now Playing]`).
/// `kind` is the source type: `HDMI`, `RF_MINI_APP` (a "mini app" like Netflix),
/// `STEREO`, `DIGITAL_AUDIO_SERVER`, `COMPONENT`, `VIDEO_SELECTION`, …
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSource {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub category: Vec<String>,
    #[serde(default)]
    pub room_id: Option<u32>,
    #[serde(default)]
    pub room_name: Option<String>,
}

impl MediaSource {
    /// A real A/V source (vs a `user_interface` shortcut like a UI Button).
    pub fn is_audio_video(&self) -> bool {
        self.category.iter().any(|c| c == "audio_video")
    }
}

/// A `visible`/`hidden` pair of sources.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaList {
    #[serde(default)]
    pub visible: Vec<MediaSource>,
    #[serde(default)]
    pub hidden: Vec<MediaSource>,
}

/// `GET /api/v1/rooms/:id/media` — a room's Watch and Listen source lists. This is
/// the authoritative source for the Watch/Listen menus (correct names, correct
/// menu split, and the user's hidden-source choices), rather than guessing from a
/// device's proxy type.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomMedia {
    // The controller emits these top-level keys in snake_case (unlike most of the
    // API), so no `rename_all` here.
    #[serde(default)]
    pub watch_devices: MediaList,
    #[serde(default)]
    pub listen_devices: MediaList,
}

/// A favorite from `GET /api/v1/agents/ui_configuration/favorites/`. `path` is
/// either a menu (`/v1/rooms/14/watch`) or a pinned item (`/v1/rooms/14/items/475`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Favorite {
    #[serde(default)]
    pub id: Option<String>,
    pub path: String,
    #[serde(default)]
    pub menu: Option<String>,
    #[serde(default)]
    pub location_id: Option<u32>,
}

impl Favorite {
    /// If this favorite pins an item, return `(room_id, item_id)`.
    pub fn as_item(&self) -> Option<(u32, u32)> {
        // path like "/v1/rooms/<room>/items/<item>"
        let mut it = self.path.split('/').filter(|s| !s.is_empty());
        let mut room = None;
        let mut item = None;
        let mut parts = Vec::new();
        while let Some(seg) = it.next() {
            parts.push(seg);
        }
        for w in parts.windows(2) {
            match w {
                ["rooms", r] => room = r.parse().ok(),
                ["items", i] => item = i.parse().ok(),
                _ => {}
            }
        }
        Some((room?, item?))
    }
}

/// Well-known room variable ids (`GET /api/v1/items/:roomId/variables`).
pub mod room_var {
    pub const CURRENT_SELECTED_DEVICE: u32 = 1000;
    pub const CURRENT_AUDIO_DEVICE: u32 = 1001;
    pub const CURRENT_VIDEO_DEVICE: u32 = 1002;
    pub const CURRENT_MEDIA: u32 = 1005;
    pub const CURRENT_AUDIO_PATH: u32 = 1007;
    pub const CURRENT_VIDEO_PATH: u32 = 1008;
    pub const POWER_STATE: u32 = 1010;
    pub const CURRENT_VOLUME: u32 = 1011;
    pub const IS_MUTED: u32 = 1018;
    pub const IN_NAVIGATION: u32 = 1019;
}

/// Map a Control4 proxy string to a [`ProxyKind`].
pub fn proxy_kind_from_str(p: &str) -> ProxyKind {
    use ProxyKind::*;
    match p {
        "controller" => Controller,
        "uidevice" | "ui_device" => UiDevice,
        "room" => Room,
        "tv" => Tv,
        "receiver" => Receiver,
        "dvd" => Dvd,
        "cable" | "rf_cable" => Cable,
        "satellite" => Satellite,
        "media_service" => MediaService,
        "media_player" => MediaPlayer,
        "tuner" => Tuner,
        "amplifier" => Amplifier,
        "avswitch" | "av_switch" => AvSwitch,
        "light" | "light_v2" => Light,
        "thermostat" | "thermostatV2" => Thermostat,
        "lock" => Lock,
        "blind" => Blind,
        "camera" => Camera,
        "fan" => Fan,
        "pool" => Pool,
        "security" => Security,
        other => Other(other.to_string()),
    }
}
