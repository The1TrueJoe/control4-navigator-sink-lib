//! # control4-navigator-sink-lib
//!
//! Receive and model the commands a Control4 **on-screen navigator** gets, so you
//! can build your own navigator (see the companion `control4-fake-navigator`).
//!
//! ## How it fits together
//!
//! ```text
//!  C4 remote ──▶ Director / roomdevice.c4l
//!        │  enters navigation (CONTROL4:<room>), forwards nav keys to the
//!        │  ONSCREEN_SELECTION-bound device
//!        ▼
//!  DriverWorks controller-proxy driver (on the controller)  ── driver/ in this repo
//!        │  relays every command as NDJSON over a socket
//!        ▼
//!  this crate:  Frame ──▶ Event ──▶ NavigatorState        ── your app renders it
//! ```
//!
//! - [`Frame`] — one NDJSON line off the wire.
//! - [`Event`] — the ergonomic, typed form ([`Event::from_frame`]).
//! - [`NavigatorState`] — live state folded from events.
//! - [`Project`]/[`Room`]/[`Device`] — the richer structure a navigator displays,
//!   populated from the controller's project data (see `docs/navigator-data-model.md`).
//! - [`SinkServer`] (feature `sink`, default) — a TCP listener the driver connects to.
//! - [`read_events`] — parse events from any `BufRead`.
//!
//! ## Quick start
//!
//! ```no_run
//! use control4_navigator_sink_lib::{SinkServer, SinkEvent, NavigatorState};
//!
//! let rx = SinkServer::listen("0.0.0.0:9010").expect("bind");
//! let mut state = NavigatorState::new();
//! for msg in rx {
//!     match msg {
//!         SinkEvent::Event(ev) => {
//!             state.apply(&ev);
//!             println!("{ev:?}  navigating_room={:?}", state.navigating_room);
//!         }
//!         SinkEvent::Connected(a) => println!("driver connected: {a}"),
//!         SinkEvent::Disconnected(a) => println!("driver gone: {a}"),
//!         SinkEvent::ParseError(l) => eprintln!("bad line: {l}"),
//!     }
//! }
//! ```

pub mod error;
pub mod loader;
pub mod model;
pub mod protocol;
pub mod rest;
pub mod sink;

pub use error::{Error, Result};
pub use loader::{
    apply_room_variables, load_project, load_room_media, load_room_variables, ProjectSource,
};
pub use model::{
    menu_for_proxy, BindingInfo, Device, Menu, NavigatorState, NowPlaying, PopupState, Project,
    ProxyKind, Room, Source,
};
pub use protocol::{Event, Frame, NavKey, Params};
pub use rest::{
    Favorite, Item, Location, MediaList, MediaSource, ProxyMeta, RoomInfo, RoomMedia, Variable,
};
pub use sink::read_events;

#[cfg(feature = "sink")]
pub use sink::{SinkEvent, SinkServer};
