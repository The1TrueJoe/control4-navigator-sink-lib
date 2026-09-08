//! Data model for a navigator.
//!
//! Two layers:
//!
//! 1. [`NavigatorState`] — **live** state assembled purely from the relayed
//!    [`Event`] stream (what's bound, which room is navigating, last key, popup,
//!    brightness). This needs nothing but the driver.
//!
//! 2. [`Project`] / [`Room`] / [`Device`] — the **richer structure a real
//!    navigator holds** (rooms, the devices you can Watch/Listen/control, current
//!    source, now-playing, …). The relay stream alone doesn't carry all of this;
//!    it is meant to be populated from the controller's project data (see
//!    `docs/navigator-data-model.md`). It lives here so `control4-fake-navigator`
//!    can render against one shared vocabulary and merge live events on top.

use crate::protocol::{Event, NavKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Live state (driven by the Event stream)
// ---------------------------------------------------------------------------

/// State of one connection binding on our driver.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingInfo {
    pub class: String,
    pub bound: bool,
}

/// A popup Director asked us to show (`uidevice` `SHOW_POPUP`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PopupState {
    pub message: Option<String>,
    pub image_url: Option<String>,
    pub show_ok: bool,
}

/// Live navigator state built from the relayed events.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigatorState {
    /// Bindings by id (5001 controller, 5002 uidevice, 7500 onscreen, 4072 hdmi…).
    pub bindings: BTreeMap<u32, BindingInfo>,
    /// Room currently in navigation mode, set by `CONTROL4:<room>`.
    pub navigating_room: Option<u32>,
    /// Most recent navigation key.
    pub last_key: Option<NavKey>,
    /// Current popup, if any.
    pub popup: Option<PopupState>,
    /// Last requested display brightness (0–100).
    pub brightness: Option<u8>,
    /// Last status line pushed to us.
    pub status: Option<String>,
    /// True once the driver has said hello.
    pub online: bool,
}

impl NavigatorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fold one event into the state. Returns `true` if anything changed.
    pub fn apply(&mut self, ev: &Event) -> bool {
        match ev {
            Event::Hello { .. } => {
                let changed = !self.online;
                self.online = true;
                changed
            }
            Event::Binding { binding, class, bound } => {
                let e = self.bindings.entry(*binding).or_default();
                let changed = e.bound != *bound || e.class != *class;
                e.class = class.clone();
                e.bound = *bound;
                changed
            }
            Event::EnterNavigation { room } => {
                let changed = self.navigating_room != Some(*room);
                self.navigating_room = Some(*room);
                changed
            }
            Event::ExitNavigation { .. } => {
                let changed = self.navigating_room.is_some();
                self.navigating_room = None;
                changed
            }
            Event::Nav { key, .. } => {
                self.last_key = Some(*key);
                true
            }
            Event::Popup { show, message, image_url, show_ok } => {
                self.popup = show.then(|| PopupState {
                    message: message.clone(),
                    image_url: image_url.clone(),
                    show_ok: *show_ok,
                });
                true
            }
            Event::Status { message } => {
                self.status = message.clone();
                true
            }
            Event::Brightness { level } => {
                self.brightness = Some(*level);
                true
            }
            Event::SelectSource { .. } | Event::Query { .. } | Event::Other { .. } => false,
        }
    }

    /// Is the on-screen (`ONSCREEN_SELECTION`, binding 7500) currently bound?
    pub fn is_onscreen_bound(&self) -> bool {
        self.bindings.get(&7500).map(|b| b.bound).unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Project structure (what a navigator displays) — populated from Director.
// See docs/navigator-data-model.md for how to source these.
// ---------------------------------------------------------------------------

/// The Control4 proxy type behind a device — determines which menu it appears in
/// and how it's controlled. Extend as needed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyKind {
    Controller,
    UiDevice,
    Room,
    Tv,
    Receiver,
    Dvd,
    Cable,
    Satellite,
    MediaService,
    MediaPlayer,
    Tuner,
    Amplifier,
    AvSwitch,
    Light,
    Thermostat,
    Lock,
    Blind,
    Camera,
    Fan,
    Pool,
    Security,
    Other(String),
}

/// A navigator top-level menu (the "experience" tabs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Menu {
    Watch,
    Listen,
    Lighting,
    Comfort,
    Security,
    Shades,
    Cameras,
    Scenes,
}

/// A device in the project (a proxy instance).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub id: u32,
    pub name: String,
    pub proxy: ProxyKind,
    /// Room this device lives in, if room-scoped.
    #[serde(default)]
    pub room_id: Option<u32>,
    /// Controller-relative path to the device's navigator icon (no `controller://`
    /// prefix), e.g. `driver/um_netflix/icons/device/experience_300.png`. Serve it
    /// through the app's icon proxy. `None` if the driver ships no navigator icon.
    #[serde(default)]
    pub icon: Option<String>,
    /// Free-form capability flags/props learned from the project (e.g.
    /// `has_discrete_volume`, `is_video_source`).
    #[serde(default)]
    pub props: BTreeMap<String, String>,
}

/// A selectable A/V source in a room's Watch or Listen list, as Control4's own
/// navigator presents it (from `GET /api/v1/rooms/:id/media`). Unlike [`Device`],
/// a source is not 1:1 with a proxy — it can be a mini-app, an HDMI input, a
/// distributed-audio server, or a special entry — so it carries Control4's own
/// `kind` string rather than a [`ProxyKind`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub id: u32,
    pub name: String,
    /// Control4 source type: `HDMI`, `RF_MINI_APP`, `STEREO`,
    /// `DIGITAL_AUDIO_SERVER`, `COMPONENT`, `VIDEO_SELECTION`, …
    pub kind: String,
    /// True for a real A/V source; false for a `user_interface` shortcut.
    #[serde(default)]
    pub audio_video: bool,
    /// Controller-relative navigator icon path (see [`Device::icon`]), matched to
    /// the source by item id. `None` if the driver ships no icon.
    #[serde(default)]
    pub icon: Option<String>,
}

/// What is currently playing / selected in a room. `title`/`artist`/`album`/
/// `art_url` are the rich media fields (present for media services that expose
/// them); `app` is the current app/station/channel (e.g. a Roku app); `state` is
/// the playback state; `transports` is the set of supported transport controls.
/// These come from the selected device's own variables (driver-specific, matched
/// by name), not a single universal source.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NowPlaying {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub art_url: Option<String>,
    /// Device id currently selected as the room's source.
    pub source_device: Option<u32>,
    /// Current app / station / channel on the source (e.g. Roku `CURRENT_APP`).
    #[serde(default)]
    pub app: Option<String>,
    /// Playback state, if the driver reports one (e.g. `Playing`, `Paused`).
    #[serde(default)]
    pub state: Option<String>,
    /// Supported transport controls (e.g. `PLAY`, `PAUSE`, `SCAN_FWD`, `SCAN_REV`).
    #[serde(default)]
    pub transports: Vec<String>,
}

/// A room and the state a navigator shows for it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Room {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub floor: Option<String>,
    /// Devices selectable/controllable in this room, grouped by menu. Used for the
    /// non-A/V menus (Lighting, Comfort, Security, Shades, Cameras) and to resolve
    /// the name of the currently-selected source.
    #[serde(default)]
    pub devices: BTreeMap<u32, Device>,
    /// The room's Watch sources, in the controller's own order (from `/rooms/:id/media`).
    #[serde(default)]
    pub watch: Vec<Source>,
    /// The room's Listen sources, in the controller's own order (from `/rooms/:id/media`).
    #[serde(default)]
    pub listen: Vec<Source>,
    #[serde(default)]
    pub current_video_device: Option<u32>,
    #[serde(default)]
    pub current_audio_device: Option<u32>,
    #[serde(default)]
    pub volume: Option<u8>,
    #[serde(default)]
    pub is_muted: bool,
    #[serde(default)]
    pub power_on: bool,
    #[serde(default)]
    pub now_playing: NowPlaying,
    /// Item ids the user favorited for this room (from ui_configuration), in order.
    #[serde(default)]
    pub favorites: Vec<u32>,
}

impl Room {
    /// Devices that belong under a given navigator menu.
    pub fn devices_in(&self, menu: Menu) -> impl Iterator<Item = &Device> {
        self.devices.values().filter(move |d| menu_for_proxy(&d.proxy) == Some(menu))
    }

    /// Resolve a selected/now-playing id to a display name, checking Watch and
    /// Listen sources first, then room devices.
    pub fn name_of(&self, id: u32) -> Option<&str> {
        self.watch
            .iter()
            .chain(self.listen.iter())
            .find(|s| s.id == id)
            .map(|s| s.name.as_str())
            .or_else(|| self.devices.get(&id).map(|d| d.name.as_str()))
    }
}

/// The whole project as a navigator sees it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub name: Option<String>,
    pub rooms: BTreeMap<u32, Room>,
    /// Global devices not scoped to a room (agents, etc.).
    #[serde(default)]
    pub devices: BTreeMap<u32, Device>,
}

/// Which menu a proxy kind belongs under (first-order mapping; real Control4 uses
/// connection classes for edge cases).
pub fn menu_for_proxy(p: &ProxyKind) -> Option<Menu> {
    use ProxyKind::*;
    Some(match p {
        Tv | Dvd | Cable | Satellite | AvSwitch => Menu::Watch,
        Receiver | Amplifier | MediaService => Menu::Listen,
        Light => Menu::Lighting,
        Thermostat | Fan => Menu::Comfort,
        Security | Lock => Menu::Security,
        Blind => Menu::Shades,
        Camera => Menu::Cameras,
        Pool => Menu::Comfort,
        _ => return None,
    })
}
