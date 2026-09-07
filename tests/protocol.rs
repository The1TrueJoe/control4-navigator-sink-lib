//! Parser tests against frames captured from a live EA-3.

use control4_navigator_sink_lib::{Event, Frame, NavKey, NavigatorState};

#[test]
fn parses_hello() {
    let f = Frame::parse(r#"{"type":"hello","message":"driver online"}"#).unwrap();
    assert!(matches!(Event::from_frame(f), Event::Hello { .. }));
}

#[test]
fn parses_bind_onscreen() {
    let f =
        Frame::parse(r#"{"type":"bind","binding":7500,"class":"ONSCREEN_SELECTION","bound":true}"#)
            .unwrap();
    match Event::from_frame(f) {
        Event::Binding { binding, class, bound } => {
            assert_eq!(binding, 7500);
            assert_eq!(class, "ONSCREEN_SELECTION");
            assert!(bound);
        }
        other => panic!("expected Binding, got {other:?}"),
    }
}

#[test]
fn enter_navigation_from_control4_room() {
    // "CONTROL4:31" observed on the wire when the C4 button is pressed for room 31.
    let f = Frame::parse(r#"{"type":"command","binding":5001,"command":"CONTROL4:31"}"#).unwrap();
    assert_eq!(Event::from_frame(f), Event::EnterNavigation { room: 31 });
}

#[test]
fn nav_keys() {
    for (cmd, key) in [("UP", NavKey::Up), ("DOWN", NavKey::Down), ("ENTER", NavKey::Enter)] {
        let line = format!(r#"{{"type":"command","binding":5001,"command":"{cmd}"}}"#);
        let f = Frame::parse(&line).unwrap();
        assert_eq!(Event::from_frame(f), Event::Nav { room: None, key });
    }
}

#[test]
fn show_popup_params() {
    let line = r#"{"type":"command","binding":5002,"command":"SHOW_POPUP","params":{"MESSAGE":"test","SHOWOK":"True","IMGURL":""}}"#;
    let f = Frame::parse(line).unwrap();
    match Event::from_frame(f) {
        Event::Popup { show, message, image_url, show_ok } => {
            assert!(show);
            assert_eq!(message.as_deref(), Some("test"));
            assert_eq!(image_url, None); // empty IMGURL -> None
            assert!(show_ok);
        }
        other => panic!("expected Popup, got {other:?}"),
    }
}

#[test]
fn brightness() {
    let line = r#"{"type":"command","binding":5002,"command":"SET_BRIGHTNESS","params":{"BRIGHTNESS":"96"}}"#;
    let f = Frame::parse(line).unwrap();
    assert_eq!(Event::from_frame(f), Event::Brightness { level: 96 });
}

#[test]
fn controller_queries_are_queries() {
    for cmd in ["GET_CONTROLLER_SETUP", "GET_CONTROLLER_DISABLED", "GET_WIFI_STRENGTH"] {
        let line = format!(r#"{{"type":"command","binding":5001,"command":"{cmd}"}}"#);
        let f = Frame::parse(&line).unwrap();
        assert!(matches!(Event::from_frame(f), Event::Query { .. }), "{cmd}");
    }
}

#[test]
fn state_tracks_navigation_session() {
    let mut s = NavigatorState::new();
    let frames = [
        r#"{"type":"hello","message":"online"}"#,
        r#"{"type":"bind","binding":7500,"class":"ONSCREEN_SELECTION","bound":true}"#,
        r#"{"type":"command","binding":5001,"command":"CONTROL4:31"}"#,
        r#"{"type":"command","binding":5001,"command":"UP"}"#,
        r#"{"type":"command","binding":5001,"command":"EXIT_NAVIGATION"}"#,
    ];
    for line in frames {
        s.apply(&Event::from_frame(Frame::parse(line).unwrap()));
    }
    assert!(s.online);
    assert!(s.is_onscreen_bound());
    assert_eq!(s.last_key, Some(NavKey::Up));
    assert_eq!(s.navigating_room, None); // exited
}

#[test]
fn round_trip_frame() {
    let f = Frame::Bind { binding: 5001, class: "CONTROLLER".into(), bound: true };
    assert_eq!(Frame::parse(&f.to_line()).unwrap(), f);
}
