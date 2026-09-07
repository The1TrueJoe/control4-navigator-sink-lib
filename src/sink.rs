//! Receiving the relay stream.
//!
//! [`read_events`] turns any `BufRead` (a TCP stream, a file of captured lines, a
//! pipe) into an iterator of [`Event`]s — so a consumer can feed frames from
//! whatever transport it likes. [`SinkServer`] is a batteries-included TCP
//! listener the DriverWorks driver connects to.

use crate::error::{Error, Result};
use crate::protocol::{Event, Frame};
use std::io::BufRead;

/// Parse a stream of NDJSON lines into [`Event`]s. Blank lines are skipped; a
/// malformed line yields an [`Error::Parse`] item (the iterator continues).
pub fn read_events<R: BufRead>(reader: R) -> impl Iterator<Item = Result<Event>> {
    reader.lines().filter_map(|line| match line {
        Err(e) => Some(Err(Error::Io(e))),
        Ok(l) if l.trim().is_empty() => None,
        Ok(l) => Some(match Frame::parse(&l) {
            Ok(frame) => Ok(Event::from_frame(frame)),
            Err(source) => Err(Error::Parse { line: l, source }),
        }),
    })
}

#[cfg(feature = "sink")]
pub use server::{SinkEvent, SinkServer};

#[cfg(feature = "sink")]
mod server {
    use super::*;
    use crate::protocol::Event;
    use std::io::BufReader;
    use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::thread;

    /// What the [`SinkServer`] emits: connection lifecycle plus parsed events.
    #[derive(Debug)]
    pub enum SinkEvent {
        /// The driver connected.
        Connected(SocketAddr),
        /// A parsed event from the driver.
        Event(Event),
        /// A line failed to parse (kept so nothing is silently lost).
        ParseError(String),
        /// The driver disconnected.
        Disconnected(SocketAddr),
    }

    /// A TCP listener the on-screen driver connects to and streams NDJSON into.
    ///
    /// ```no_run
    /// use control4_navigator_sink_lib::{SinkServer, SinkEvent};
    /// let rx = SinkServer::listen("0.0.0.0:9010").unwrap();
    /// for msg in rx {
    ///     if let SinkEvent::Event(ev) = msg {
    ///         println!("{ev:?}");
    ///     }
    /// }
    /// ```
    pub struct SinkServer;

    impl SinkServer {
        /// Bind and start accepting. Returns a receiver of [`SinkEvent`]s. The
        /// acceptor and per-connection readers run on background threads; drop the
        /// receiver to let them wind down as connections close.
        pub fn listen<A: ToSocketAddrs>(addr: A) -> std::io::Result<Receiver<SinkEvent>> {
            let listener = TcpListener::bind(addr)?;
            let (tx, rx) = channel();
            thread::spawn(move || Self::accept_loop(listener, tx));
            Ok(rx)
        }

        fn accept_loop(listener: TcpListener, tx: Sender<SinkEvent>) {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let peer = stream.peer_addr().ok();
                let tx = tx.clone();
                thread::spawn(move || handle_conn(stream, peer, tx));
            }
        }
    }

    fn handle_conn(stream: TcpStream, peer: Option<SocketAddr>, tx: Sender<SinkEvent>) {
        if let Some(p) = peer {
            let _ = tx.send(SinkEvent::Connected(p));
        }
        for item in read_events(BufReader::new(stream)) {
            let msg = match item {
                Ok(ev) => SinkEvent::Event(ev),
                Err(Error::Parse { line, .. }) => SinkEvent::ParseError(line),
                Err(Error::Io(_)) => break,
            };
            if tx.send(msg).is_err() {
                return; // receiver dropped
            }
        }
        if let Some(p) = peer {
            let _ = tx.send(SinkEvent::Disconnected(p));
        }
    }
}
