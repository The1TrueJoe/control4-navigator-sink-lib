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
**Confirmed against a live EA-3** (OS 3.3.3) — see the captured schemas in the
openHC research (`c4-recon/research/api/DATA-MODEL.md`) and the typed structs in
[`src/rest.rs`](../src/rest.rs). Served by the `broker` node app.

- `GET /api/v1/locations` — location tree (site 2 / building 3 / floor 4 / room 8) → [`Location`]
- `GET /api/v1/rooms` — flat room list → [`RoomInfo`]
- `GET /api/v1/items` / `GET /api/v1/items/:id` — devices + agents (proxy, proxyMeta,
  categories, roomId, control/filename) → [`Item`]
- `GET /api/v1/items/:id/variables` — **live state bus** → [`Variable`] (room vars:
  1000 CURRENT_SELECTED_DEVICE, 1010 POWER_STATE, 1011 CURRENT_VOLUME, 1018 IS_MUTED,
  1019 IN_NAVIGATION, …; device vars per proxy)
- `GET /api/v1/items/:id/commands`, `/api/v1/agents`, `/api/v1/drivers`

**Auth:** Bearer JWT. Mint via `POST /api/v1/localjwt` presenting a trusted
Composer **client cert** to nginx:443 (`ssl_verify_client optional`), or on-box by
POSTing to `127.0.0.1:3000/api/v1/localjwt` with `X-SSL-CERT-VERIFY: SUCCESS` +
`X-SSL-CERT-CLIENT_S_DN: CN=Composer_…`. Open (no token): `locations`, `agents`,
`status`, `routes`, `common_name`. Live push: `/api/v1/subscriptions` + `/ws/token`.

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

## 2.5. How a REAL navigator gets its data (generated + pushed, not preset)

Important: a Control4 navigator does **not** load a static project snapshot. The
**cerebellum** app (`/opt/control4/tr3/cerebellum`, port 3001) *renders* the UI from
mustache templates in `filerepo/tr2/` against the live project, and **pushes** a live
data document to the navigator. **"Refresh Navigators"** regenerates it — the
`ui_configuration` agent logs *"Received Refresh Navigators - Cleared active state."*
and re-renders/re-pushes. So the nav data is a **living, regenerating feed**.

- **Screens** = `filerepo/tr2/gui/*.tpl.xml` (`main`, `proxies`, `popups`, `keypad`,
  `header`, `directory`, `dynamicScreens`, …) — the static UI, rendered per project.
- **Live data** = `guidata.tpl.xml` → an `<update>` doc, continuously pushed:
  `version(projectId)`, `localTime/clockType`, `screenBrightness`, `favorites`,
  `cachedValues`, `playerData` (now-playing), `tunerData`, `volumeInfo`,
  `volumePopupVisibility`, `dynamicRoomInfo`, `homeActiveMedia`, `listContent`
  (`updatedListItems`), `demoMode`. Mustache: `{{key}}`, `{{#section}}`, `{{{raw}}}`.
- **cerebellum auth** is navigator-specific (our broker JWT is rejected): a nav
  presents a **client cert** (`/etc/openvpn/client.pem`) and mints a token via
  `POST /api/v1/ws/token`. A device with no nav identity can't yet talk to cerebellum.

### Two intercompat strategies
- **A — raw project, our own UI (recommended for "represent it our way"):** consume
  broker `/api/v1` (`locations`/`rooms`/`items`/`variables` + `/subscriptions` push)
  and render our own navigator. The API is always live, so "Refresh Navigators" never
  leaves us stale — we just keep syncing. Version-independent; no tr2 XML coupling.
- **B — true navigator emulation:** obtain a nav client cert + `ws/token`, consume
  cerebellum's rendered `gui` + `guidata` `<update>` stream, and honor Refresh
  Navigators. Truest intercompat, but couples us to the tr2/tr3 format and needs a
  paired navigator identity.

## 3. Suggested representation & flow (live-sync, not one-shot)

```
  .c4p/project.xml  ──▶ Project (rooms, devices)          [bootstrap, offline]
  REST /api/v1/items ─▶ Project (rooms, devices)          [bootstrap, live]
        │
        ▼
   Project + NavigatorState   ◀── relay Events (nav, selection, popup)  [live]
        ▲
        └────────────────────── relay `state` frames (room vars)  [future]
```

- Build [`Project`] from (b)/(c) and **re-sync on change** — the API is the live
  source of truth, never a cached preset. "Refresh Navigators" / project edits are
  picked up by re-fetching `items`/`rooms` (cheap) or via `/subscriptions` push;
  don't hold a stale snapshot.
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
[`Location`]: ../src/rest.rs
[`RoomInfo`]: ../src/rest.rs
[`Item`]: ../src/rest.rs
[`Variable`]: ../src/rest.rs
