# Wire protocol

The controller-side DriverWorks driver (`driver/`) relays everything Director
sends the on-screen navigator to the sink as **NDJSON** — one JSON object per
line, UTF-8, `\n`-terminated, over a single persistent TCP connection (driver =
client, sink = server). All strings are JSON-escaped so a frame is always one
line, even when Director's payload contained newlines.

This is the ground truth captured from a live EA-3 (Director 3.3.0).

## Frames

### `hello`
Sent once when the driver starts.
```json
{"type":"hello","message":"openHC navigator sink driver online"}
```

### `bind`
A connection binding changed bound state.
```json
{"type":"bind","binding":7500,"class":"ONSCREEN_SELECTION","bound":true}
```
Bindings seen: `5001` CONTROLLER, `5002` UI_DEVICE, `4072` HDMI, `7500`
ONSCREEN_SELECTION.

### `command`
Any command from `ReceivedFromProxy` / `ExecuteCommand`.
```json
{"type":"command","binding":5001,"room":31,"command":"UP","params":{}}
```
`room` is `null` when Director didn't scope the command; `params` is a flat
`string -> string` map.

## Command semantics (how the lib maps them to `Event`)

| Command (on the wire)            | Binding | Meaning / `Event`                              |
|----------------------------------|---------|------------------------------------------------|
| `CONTROL4:<roomId>`              | 5001    | Room entered navigation → `EnterNavigation{room}` |
| `UP` `DOWN` `LEFT` `RIGHT` `ENTER` `BACK` `CANCEL` `GUIDE` `PAGE_UP` `PAGE_DOWN` | 5001 | Nav keypress → `Nav{key}` |
| `START_*` / `STOP_*`             | 5001    | Held-key press/release edges → `Nav{key}` (`is_edge`) |
| `EXIT_NAVIGATION`, `NICE_NAVIGATOR{NICE=0}` | 5001 | Leave navigation → `ExitNavigation` |
| `SELECT_SOURCE{ROOM_ID,PATH_TYPE}` | 5001  | Room selected us as source → `SelectSource` |
| `SHOW_POPUP{MESSAGE,IMGURL,SHOWOK,SIZE,DELAY,VAR_*}` | 5002 | `Popup{show:true,...}` |
| `HIDE_POPUP`                     | 5002    | `Popup{show:false}`                            |
| `SET_STATUS{MESSAGE,DELAY}`      | 5002    | `Status`                                       |
| `SET_BRIGHTNESS{BRIGHTNESS}`     | 5002    | `Brightness{level 0-100}`                      |
| `SET_ADAPTIVE_BRIGHTNESS{...}`   | 5002    | `Other` (not yet specialized)                  |
| `GET_WIFI_STRENGTH`              | 5002    | `Query` (expects a reply)                      |
| `GET_CONTROLLER_SETUP`, `GET_CONTROLLER_DISABLED`, `CAPABILITIES_CHANGED`, `PROXY_NAME`, `DEFAULT_ROOM`, `AV_BINDINGS_CHANGED` | 5001 | Load/handshake → `Query`/`Other` |

### Notes from the capture
- Nav keys arrive on the **controller binding 5001** with **empty params**; the
  active room comes from the preceding `CONTROL4:<roomId>` (not from the key).
- Each nav key is accompanied by an internal `BoundCall` referencing
  `idBinding:7500 rsClass:"ONSCREEN_SELECTION"` — informational; the lib keys off
  the bare command.
- `PROXY_NAME` carries `data=controller,uidevice`.

## Answering Director (reverse direction)
The driver can reply to queries via `C4:SendToProxy(binding, notify, params)`.
Today it best-effort answers `GET_CONTROLLER_DISABLED`. If the sink needs to
drive replies (e.g. `GET_WIFI_STRENGTH`), extend the protocol with a
sink→driver control channel; the current transport is one-way (driver→sink).
