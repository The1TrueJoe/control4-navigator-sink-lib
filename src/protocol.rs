//! Wire protocol between the on-screen DriverWorks driver (running on the Control4
//! controller) and this sink.
//!
//! The driver relays everything Director sends it as **NDJSON** — one JSON object
//! per line. A raw line deserializes into [`Frame`]; [`Event`] is the high-level,
//! ergonomic form a consumer actually wants (nav keys, enter/exit navigation,
//! popups, …), produced by [`Event::from_frame`].
//!
//! Ground truth for the frame set was captured from a live EA-3 (see
//! `docs/wire-protocol.md`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A parameter bag. Director sends command params as flat string key/values; a
/// few structured payloads (e.g. `BoundCall`) arrive as a single JSON-encoded
/// string under the `raw` key so framing stays one-line.
pub type Params = BTreeMap<String, String>;

/// A single line off the wire.
///
/// Serialized with an internal `type` tag so it is trivially matched on the
/// consumer side and stable to extend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Frame {
    /// Driver came online (sent once on `OnDriverLateInit`).
    Hello { message: String },
    /// A connection binding changed state (bound/unbound), e.g. binding `7500`
    /// class `ONSCREEN_SELECTION` becoming `bound: true` when the room selects us.
    Bind {
        binding: u32,
        class: String,
        bound: bool,
    },
    /// Any command Director sent the driver (`ReceivedFromProxy` / `ExecuteCommand`).
    Command {
        binding: u32,
        #[serde(default)]
        room: Option<u32>,
        command: String,
        #[serde(default)]
        params: Params,
    },
}

impl Frame {
    /// Parse one NDJSON line into a [`Frame`].
    pub fn parse(line: &str) -> Result<Frame, serde_json::Error> {
        serde_json::from_str(line.trim())
    }

    /// Serialize to a single NDJSON line (no trailing newline).
    pub fn to_line(&self) -> String {
        // Frame is always serializable; unwrap is safe.
        serde_json::to_string(self).expect("Frame serializes")
    }
}

/// The navigation keypresses the room forwards to the on-screen device while
/// `IN_NAVIGATION`. `START_*`/`STOP_*` are the press/release pair for held keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NavKey {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Back,
    Cancel,
    Guide,
    PageUp,
    PageDown,
    StartUp,
    StartDown,
    StartLeft,
    StartRight,
    StartPageUp,
    StartPageDown,
    StopUp,
    StopDown,
    StopLeft,
    StopRight,
    StopPageUp,
    StopPageDown,
}

impl NavKey {
    /// Map a Control4 command string to a [`NavKey`], if it is one.
    pub fn from_command(cmd: &str) -> Option<NavKey> {
        use NavKey::*;
        Some(match cmd {
            "UP" => Up,
            "DOWN" => Down,
            "LEFT" => Left,
            "RIGHT" => Right,
            "ENTER" => Enter,
            "BACK" => Back,
            "CANCEL" => Cancel,
            "GUIDE" => Guide,
            "PAGE_UP" => PageUp,
            "PAGE_DOWN" => PageDown,
            "START_UP" => StartUp,
            "START_DOWN" => StartDown,
            "START_LEFT" => StartLeft,
            "START_RIGHT" => StartRight,
            "START_PAGE_UP" => StartPageUp,
            "START_PAGE_DOWN" => StartPageDown,
            "STOP_UP" => StopUp,
            "STOP_DOWN" => StopDown,
            "STOP_LEFT" => StopLeft,
            "STOP_RIGHT" => StopRight,
            "STOP_PAGE_UP" => StopPageUp,
            "STOP_PAGE_DOWN" => StopPageDown,
            _ => return None,
        })
    }

    /// The Control4 command string for this key.
    pub fn as_command(self) -> &'static str {
        use NavKey::*;
        match self {
            Up => "UP",
            Down => "DOWN",
            Left => "LEFT",
            Right => "RIGHT",
            Enter => "ENTER",
            Back => "BACK",
            Cancel => "CANCEL",
            Guide => "GUIDE",
            PageUp => "PAGE_UP",
            PageDown => "PAGE_DOWN",
            StartUp => "START_UP",
            StartDown => "START_DOWN",
            StartLeft => "START_LEFT",
            StartRight => "START_RIGHT",
            StartPageUp => "START_PAGE_UP",
            StartPageDown => "START_PAGE_DOWN",
            StopUp => "STOP_UP",
            StopDown => "STOP_DOWN",
            StopLeft => "STOP_LEFT",
            StopRight => "STOP_RIGHT",
            StopPageUp => "STOP_PAGE_UP",
            StopPageDown => "STOP_PAGE_DOWN",
        }
    }

    /// Is this a "held key" edge (`START_*`/`STOP_*`) rather than a discrete tap?
    pub fn is_edge(self) -> bool {
        matches!(self.as_command().split('_').next(), Some("START") | Some("STOP"))
    }
}

/// High-level, typed event a consumer reacts to. Produced from a [`Frame`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Driver online.
    Hello { message: String },
    /// A binding's bound state changed.
    Binding { binding: u32, class: String, bound: bool },
    /// The room entered navigation mode (`CONTROL4:<room>`). Nav keys that follow
    /// belong to this room until [`Event::ExitNavigation`].
    EnterNavigation { room: u32 },
    /// The room left navigation mode (`EXIT_NAVIGATION` / `NICE_NAVIGATOR NICE=0`).
    ExitNavigation { room: Option<u32> },
    /// A navigation keypress.
    Nav { room: Option<u32>, key: NavKey },
    /// The room selected this device as a source (`SELECT_SOURCE`).
    SelectSource {
        room: Option<u32>,
        params: Params,
    },
    /// Show/hide an on-screen popup (`SHOW_POPUP` / `HIDE_POPUP`) — a `uidevice`
    /// command; useful when rendering our own navigator UI.
    Popup {
        show: bool,
        message: Option<String>,
        image_url: Option<String>,
        show_ok: bool,
    },
    /// A status line to display (`SET_STATUS`).
    Status { message: Option<String> },
    /// Requested display brightness 0–100 (`SET_BRIGHTNESS`).
    Brightness { level: u8 },
    /// Director handshake/query we may want to answer (`GET_CONTROLLER_SETUP`,
    /// `GET_CONTROLLER_DISABLED`, `GET_WIFI_STRENGTH`, `PROXY_NAME`, …).
    Query { binding: u32, command: String, params: Params },
    /// Anything not specially recognized — still fully available.
    Other {
        binding: u32,
        room: Option<u32>,
        command: String,
        params: Params,
    },
}

impl Event {
    /// Interpret a raw [`Frame`] as a high-level [`Event`].
    pub fn from_frame(frame: Frame) -> Event {
        match frame {
            Frame::Hello { message } => Event::Hello { message },
            Frame::Bind { binding, class, bound } => Event::Binding { binding, class, bound },
            Frame::Command { binding, room, command, params } => {
                // Enter navigation is encoded as "CONTROL4:<roomId>".
                if let Some(rest) = command.strip_prefix("CONTROL4") {
                    let r = rest.strip_prefix(':').and_then(|s| s.parse().ok());
                    if let Some(r) = r {
                        return Event::EnterNavigation { room: r };
                    }
                }
                if let Some(key) = NavKey::from_command(&command) {
                    return Event::Nav { room, key };
                }
                match command.as_str() {
                    "EXIT_NAVIGATION" => Event::ExitNavigation { room },
                    "NICE_NAVIGATOR" if params.get("NICE").map(String::as_str) == Some("0") => {
                        Event::ExitNavigation { room }
                    }
                    "SELECT_SOURCE" => Event::SelectSource { room, params },
                    "SHOW_POPUP" => Event::Popup {
                        show: true,
                        message: params.get("MESSAGE").cloned(),
                        image_url: params.get("IMGURL").filter(|s| !s.is_empty()).cloned(),
                        show_ok: params.get("SHOWOK").map(String::as_str) == Some("True"),
                    },
                    "HIDE_POPUP" => Event::Popup {
                        show: false,
                        message: None,
                        image_url: None,
                        show_ok: false,
                    },
                    "SET_STATUS" => Event::Status { message: params.get("MESSAGE").cloned() },
                    "SET_BRIGHTNESS" => Event::Brightness {
                        level: params.get("BRIGHTNESS").and_then(|s| s.parse().ok()).unwrap_or(0),
                    },
                    c if c.starts_with("GET_") || c == "PROXY_NAME" || c == "CAPABILITIES_CHANGED" => {
                        Event::Query { binding, command, params }
                    }
                    _ => Event::Other { binding, room, command, params },
                }
            }
        }
    }
}
