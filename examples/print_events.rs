//! Minimal sink: listen for the driver, print events, track state.
//!
//! ```sh
//! cargo run --example print_events -- 0.0.0.0:9010
//! ```
//! Then point the DriverWorks driver's Target IP/Port at this host.

use control4_navigator_sink_lib::{NavigatorState, SinkEvent, SinkServer};

fn main() {
    let addr = std::env::args().nth(1).unwrap_or_else(|| "0.0.0.0:9010".into());
    let rx = SinkServer::listen(&addr).expect("bind");
    println!("Control4 Navigator Sink listening on {addr}");

    let mut state = NavigatorState::new();
    for msg in rx {
        match msg {
            SinkEvent::Connected(a) => println!("[+] driver connected: {a}"),
            SinkEvent::Disconnected(a) => println!("[-] driver disconnected: {a}"),
            SinkEvent::ParseError(l) => eprintln!("[!] unparsed: {l}"),
            SinkEvent::Event(ev) => {
                state.apply(&ev);
                println!(
                    "{ev:?}   | onscreen_bound={} navigating_room={:?}",
                    state.is_onscreen_bound(),
                    state.navigating_room
                );
            }
        }
    }
}
