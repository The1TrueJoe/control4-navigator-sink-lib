# The data a navigator holds — and how to get it

The relay stream (see `wire-protocol.md`) gives us the **live on-screen session**:
which room is navigating, keypresses, popups, brightness, source selection. That
is enough to *drive* a UI, but a real navigator also *displays a whole project*:
rooms, the things you can Watch/Listen to and control, what's playing, etc.

This doc explores what that data is, where to source it, and how we represent it
(`src/model.rs`) so `control4-fake-navigator` can render it our own way.

## 1. What a Control4 navigator shows

- **Rooms** (grouped by floor). One room is "current".
- **Per room:**
  - **Watch** sources — video devices (TV, cable/sat box, Apple TV, media players).
  - **Listen** sources — audio devices (receiver, streamers, tuners).
  - **current selected device**, **volume / mute / power state**.
  - **Now playing** — title / artist / album / cover art for the active source.
  - **Cameras** available to the room.
- **Experience menus:** Watch, Listen, Lighting, Comfort (thermostats, fans,
  pool), Security (locks, panels), Shades (blinds), Cameras, Scenes/Activities.
- **Per menu:** the device list + each device's controllable state (light level,
  setpoint, lock state, shade position…).
- **Room variables** (the live state bus): e.g. `1000` current selected device,
  `1019` in-navigation, plus volume/media/power variables.

Our types for this: [`Project`], [`Room`], [`Device`], [`ProxyKind`], [`Menu`],
[`NowPlaying`] in `src/model.rs`. [`NavigatorState`] holds the live-session slice.

## 2. Where the data comes from (ranked)

### a. The relay stream — this lib (live, partial)
Free with the driver. Gives nav/onscreen/selection/popup/brightness in real time.
Missing: the full room/device catalog and per-device state. **Fold with
`NavigatorState::apply`.**

### b. Controller REST API `/api/v1/…` (HTTPS 443, JWT) — recommended for structure
The controller exposes a REST API; the jailbreak uses `GET /api/v1/items` (Bearer
JWT) to enumerate project items/devices, and `/api/v1/sysman/…` for system ops.
This is the cleanest **structured** source for rooms + devices + capabilities.
Plan: fetch `items` once to build [`Project`], then refresh on change.
_TODO: confirm the exact endpoints for room state / variables / now-playing._

### c. Project backup `.c4p` → `project.xml` (full, static, offline)
The whole project: rooms, devices, drivers, every connection/binding. Perfect for
**bootstrapping and offline dev** (no controller needed). This is how the on-screen
model was derived. A `project.xml` loader can populate [`Project`] directly.

### d. Director protocol on `:5021` (mutual TLS, composer cert) — richest, hardest
The c4soap/XML protocol Composer speaks. Live variables, media database, bindings.
Most complete but proprietary; reach for it only if a/b/c fall short.

### e. Room variables via the driver (natural live extension)
Our DriverWorks driver can `C4:RegisterVariableListener(roomId, 1000)` / `1019` /
volume, and `C4:GetProxyDevicesByName(...)`, then relay changes as new frames.
This turns the driver into a live state feed without polling the REST API.

## 3. Suggested representation & flow

```
  .c4p/project.xml  ──▶ Project (rooms, devices)          [bootstrap, offline]
  REST /api/v1/items ─▶ Project (rooms, devices)          [bootstrap, live]
        │
        ▼
   Project + NavigatorState   ◀── relay Events (nav, selection, popup)  [live]
        ▲
        └────────────────────── relay `state` frames (room vars)  [future]
```

- Build [`Project`] once from (b) or (c).
- Keep [`NavigatorState`] updated from the relay [`Event`]s.
- Merge: e.g. on `SelectSource{room}` update `Room::now_playing.source_device`;
  on room-variable frames update volume/power/now-playing.

## 4. Proposed protocol extensions (for the driver → sink feed)

Add frames so the driver can push project/state without a second channel:

```json
{"type":"rooms","rooms":[{"id":31,"name":"Control4"},{"id":14,"name":"Living Room"}]}
{"type":"device","id":68,"name":"Apple TV","proxy":"dvd","room":14}
{"type":"state","room":14,"var":"CURRENT_SELECTED_DEVICE","value":"68"}
{"type":"state","room":14,"var":"IN_NAVIGATION","value":"1"}
{"type":"nowplaying","room":14,"title":"…","artist":"…","art":"http://…"}
```

Driver side: `RegisterVariableListener` for the room vars + `GetProxyDevicesByName`
enumeration on `OnDriverLateInit`, emitting the above. Lib side: add matching
`Frame` variants and fold them into [`Project`]/[`Room`]. Kept out of v0.1 so the
core (nav path) stays small and proven; this is the clear next milestone.

[`Project`]: ../src/model.rs
[`Room`]: ../src/model.rs
[`Device`]: ../src/model.rs
[`ProxyKind`]: ../src/model.rs
[`Menu`]: ../src/model.rs
[`NowPlaying`]: ../src/model.rs
[`NavigatorState`]: ../src/model.rs
[`Event`]: ../src/protocol.rs
