//! Deserialize the REST types against JSON captured from a live EA-3
//! (`tests/fixtures/`).

use control4_navigator_sink_lib::rest::{room_var, Variable};
use control4_navigator_sink_lib::{Item, Location, ProxyKind, RoomInfo};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR")))
        .unwrap_or_else(|e| panic!("read fixture {name}: {e}"))
}

#[test]
fn rooms_deserialize() {
    let rooms: Vec<RoomInfo> = serde_json::from_str(&fixture("rooms.json")).unwrap();
    assert!(!rooms.is_empty());
    let r = &rooms[0];
    assert_eq!(r.type_name.as_deref(), Some("room"));
    assert!(r.floor_name.is_some());
}

#[test]
fn locations_tree_deserializes() {
    // /api/v1/locations may be a single root or an array of roots.
    let txt = fixture("locations.json");
    let roots: Vec<Location> = serde_json::from_str::<Vec<Location>>(&txt)
        .or_else(|_| serde_json::from_str::<Location>(&txt).map(|l| vec![l]))
        .unwrap();
    // site(2) -> building(3) -> floor(4) -> room(8) nesting exists somewhere.
    fn find_kind(ns: &[Location], k: u32) -> bool {
        ns.iter().any(|n| n.kind == k || find_kind(&n.children, k))
    }
    assert!(find_kind(&roots, 8), "expected a room (type 8) in the tree");
}

#[test]
fn item_detail_deserializes() {
    // /api/v1/items/:id returns a one-element array.
    let items: Vec<Item> = serde_json::from_str(&fixture("item-detail.json")).unwrap();
    let it = &items[0];
    assert_eq!(it.type_name, "device");
    assert!(it.is_device());
    assert_eq!(it.proxy_kind(), Some(ProxyKind::Controller));
    // EA-3 presents controller + uidevice.
    assert!(it.proxy_meta.iter().any(|p| p.proxy.as_deref() == Some("uidevice")));
    assert!(it.categories.iter().any(|c| c == "controllers"));
}

#[test]
fn room_variables_state_bus() {
    let vars: Vec<Variable> = serde_json::from_str(&fixture("room14-variables.json")).unwrap();
    let get = |vid: u32| vars.iter().find(|v| v.variable_id == vid);
    assert_eq!(
        get(room_var::CURRENT_SELECTED_DEVICE).unwrap().var_name,
        "CURRENT_SELECTED_DEVICE"
    );
    // IN_NAVIGATION is a bool-ish var.
    let innav = get(room_var::IN_NAVIGATION).expect("IN_NAVIGATION present");
    assert!(innav.as_bool().is_some());
}
