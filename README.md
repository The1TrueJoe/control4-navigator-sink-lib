# control4-navigator-sink-lib

A Rust library that lets a device **pretend to be a Control4 on-screen navigator**:
it receives the room/navigation commands Director sends an on-screen device and
models the state a navigator holds — so you can render your own navigator (see the
companion **control4-fake-navigator**, which imports this crate as a submodule).

Built from ground truth captured on a real Control4 **EA-3** (Director 3.3.0): the
on-screen navigator is the `controller` proxy, connection `7500`
`ONSCREEN_SELECTION`. The included DriverWorks driver was **validated end-to-end** —
it loads, auto-binds as a room's on-screen device, and the remote's arrow keys
arrive as events.

## Architecture

```
 C4 remote ──▶ Director / roomdevice.c4l
       │  CONTROL4:<room> → navigation; forwards nav keys to the
       │  ONSCREEN_SELECTION-bound device
       ▼
 driver/  — DriverWorks controller-proxy driver (runs on the controller)
       │  relays every command as NDJSON over a persistent TCP socket
       ▼
 this crate:  Frame ──▶ Event ──▶ NavigatorState / Project
       ▼
 control4-fake-navigator  — renders the UI (separate repo, uses this as a submodule)
```

## Crate layout

| Module | What |
|---|---|
| [`protocol`](src/protocol.rs) | `Frame` (wire NDJSON), `Event` (typed), `NavKey` |
| [`model`](src/model.rs) | `NavigatorState` (live) + `Project`/`Room`/`Device` (structure) |
| [`sink`](src/sink.rs) | `read_events` (any `BufRead`) + `SinkServer` (TCP listener) |
| [`error`](src/error.rs) | `Error` / `Result` |

## Use it

```rust
use control4_navigator_sink_lib::{SinkServer, SinkEvent, NavigatorState, Event, NavKey};

let rx = SinkServer::listen("0.0.0.0:9010")?;
let mut state = NavigatorState::new();
for msg in rx {
    if let SinkEvent::Event(ev) = msg {
        state.apply(&ev);
        if let Event::Nav { key: NavKey::Up, .. } = ev { /* move selection up */ }
    }
}
```

Or parse from any reader (a capture file, a pipe, your own transport):

```rust
use control4_navigator_sink_lib::read_events;
for ev in read_events(std::io::stdin().lock()) { println!("{:?}", ev?); }
```

Run the demo sink:

```sh
cargo run --example print_events -- 0.0.0.0:9010
```

## The controller-side driver

`driver/` is the DriverWorks driver that feeds this lib. Build and install it:

```sh
cd driver && ./package.sh          # -> ohc-nav-sink.c4z
```

In Composer Pro: add the driver, set **Target IP/Port** to the host running the
sink, bind **HDMI (Audio/Video)** to a TV input (the Onscreen Navigator
auto-binds). Press the room's **C4/menu** button and the nav keys stream to the sink.

## As a submodule (from control4-fake-navigator)

```sh
git submodule add https://github.com/The1TrueJoe/control4-navigator-sink-lib
```
```toml
# Cargo.toml
[dependencies]
control4-navigator-sink-lib = { path = "control4-navigator-sink-lib" }
```

## Docs
- [`docs/wire-protocol.md`](docs/wire-protocol.md) — the NDJSON frames + command semantics
- [`docs/navigator-data-model.md`](docs/navigator-data-model.md) — what a navigator holds and how to source it

## Status
v0.1 — the nav/on-screen command path is complete, typed, tested, and hardware-validated.
Next milestone: driver-relayed project/room-variable frames (see the data-model doc)
so `Project`/`Room` fill with live device + now-playing state.

## License
MIT
