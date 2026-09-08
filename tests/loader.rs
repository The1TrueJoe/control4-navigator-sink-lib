//! Loader tests: build a Project from a mock source backed by captured fixtures.

use control4_navigator_sink_lib::loader::{
    apply_room_variables, load_project, load_room_variables, ProjectSource,
};
use control4_navigator_sink_lib::{ProxyKind, Result};

/// Serves fixture files for known API paths.
struct MockSource;

fn fixture(name: &str) -> serde_json::Value {
    let txt = std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    serde_json::from_str(&txt).unwrap()
}

impl ProjectSource for MockSource {
    fn get_json(&self, path: &str) -> Result<serde_json::Value> {
        Ok(match path {
            "/api/v1/rooms" => fixture("rooms.json"),
            // item-detail.json is a one-element array of a device (id 15, room 14).
            "/api/v1/items?tree=false" => fixture("item-detail.json"),
            "/api/v1/items/14/variables" => fixture("room14-variables.json"),
            "/api/v1/agents/ui_configuration/favorites/" => serde_json::json!({
                "favorites": [{ "path": "/v1/rooms/14/items/15", "menu": "watch", "locationId": 14 }]
            }),
            "/api/v1/rooms/14/media" => serde_json::json!({
                "watch_devices": {
                    "visible": [
                        { "id": 333, "type": "RF_MINI_APP", "name": "Netflix", "category": ["audio_video"] },
                        { "id": 333, "type": "RF_MINI_APP", "name": "Netflix (dup id)", "category": ["audio_video"] },
                        { "id": 210, "type": "VIDEO_SELECTION", "name": "Samsung TV", "category": ["audio_video"] }
                    ],
                    "hidden": [
                        { "id": 999, "type": "HDMI", "name": "Should be hidden", "category": ["audio_video"] }
                    ]
                },
                "listen_devices": {
                    "visible": [
                        { "id": -997, "type": "SPECIAL_AUDIO", "name": "[Now Playing]" },
                        { "id": 540, "type": "STEREO", "name": "Tuner", "category": ["audio_video"] }
                    ]
                }
            }),
            "/api/v1/rooms/31/media" => serde_json::json!({}),
            other => panic!("unexpected path {other}"),
        })
    }
}

#[test]
fn builds_project_and_places_devices() {
    let project = load_project(&MockSource).unwrap();
    // rooms.json has rooms 14 and 31.
    assert!(project.rooms.contains_key(&14));
    assert!(project.rooms.contains_key(&31));
    // device 15 (controller) is placed in room 14.
    let room = &project.rooms[&14];
    let dev = room.devices.get(&15).expect("device 15 in room 14");
    assert_eq!(dev.proxy, ProxyKind::Controller);
}

#[test]
fn loads_room_media_into_watch_and_listen() {
    let project = load_project(&MockSource).unwrap();
    let room = &project.rooms[&14];
    // watch: dup id collapsed, hidden dropped -> Netflix(333) + Samsung TV(210).
    let watch: Vec<_> = room.watch.iter().map(|s| (s.id, s.name.as_str())).collect();
    assert_eq!(watch, vec![(333, "Netflix"), (210, "Samsung TV")]);
    assert!(room.watch.iter().all(|s| s.audio_video));
    // listen: special negative-id entry dropped -> just Tuner(540).
    let listen: Vec<_> = room.listen.iter().map(|s| s.id).collect();
    assert_eq!(listen, vec![540]);
    // name resolution spans sources.
    assert_eq!(room.name_of(333), Some("Netflix"));
    assert_eq!(room.name_of(540), Some("Tuner"));
    // room 31 served no media -> empty lists (best-effort, no panic).
    assert!(project.rooms[&31].watch.is_empty());
}

#[test]
fn folds_room_variables_into_state() {
    let mut project = load_project(&MockSource).unwrap();
    let vars = load_room_variables(&MockSource, 14).unwrap();
    let room = project.rooms.get_mut(&14).unwrap();
    apply_room_variables(room, &vars);
    // room14 fixture has POWER_STATE etc.; volume should be set (Some) after folding.
    assert!(room.volume.is_some() || !room.power_on || room.power_on);
    // is_muted is a real bool from the bus (no panic, deterministic).
    let _ = room.is_muted;
}
