//! The protocol on the wire and the background threads that run every connection.
//!
//! Connections are WebSockets (`ws://`, or `wss://` with the `tls` feature). Every WebSocket
//! message is binary. The first two messages exchange X25519 public keys in the clear; every
//! later message is one [`Frame`] (a tag byte and its body), encrypted with [`Cipher`]. Each
//! connection has one thread that writes what the game queued, sends a ping every second and
//! reads what arrived. The game thread only talks to it through channels.

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Message, WebSocket};

use super::crypto::{Cipher, KeyPair};
use super::packet::{Packet, Reader, put_str};
use super::{DEFAULT_PORT, Reason};

/// Changes whenever the frames change. Both sides must match.
pub(crate) const PROTOCOL: u16 = 2;
/// Start of both key messages, so other programs that connect by mistake are dropped at once.
const MAGIC: &[u8; 8] = b"AUDIOCD2";
/// Largest frame accepted (before encryption). A bigger one closes the connection.
pub(crate) const MAX_FRAME: usize = 1 << 20;
/// How often each side sends a ping.
const PING_EVERY: Duration = Duration::from_secs(1);
/// A connection that hears nothing for this long is closed.
pub(crate) const TIMEOUT: Duration = Duration::from_secs(10);
/// Time allowed for connecting and for the key, hello and welcome exchange.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a connection thread waits for the game to queue something before it looks for
/// incoming data again. This is the most an arriving packet waits before it is reported.
const POLL: Duration = Duration::from_millis(2);
/// How long a closing connection waits for the other side to confirm.
const CLOSE_WAIT: Duration = Duration::from_secs(1);

pub(crate) enum Frame {
    Hello { protocol: u16, game: String, version: String, name: String },
    Welcome { id: u32, name: String },
    Ping(u64),
    Pong(u64),
    Data(Packet),
    Bye(Reason),
}

impl Frame {
    /// The frame as bytes, before encryption.
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Frame::Hello { protocol, game, version, name } => {
                out.push(0);
                out.extend_from_slice(&protocol.to_le_bytes());
                put_str(&mut out, game);
                put_str(&mut out, version);
                put_str(&mut out, name);
            }
            Frame::Welcome { id, name } => {
                out.push(1);
                out.extend_from_slice(&id.to_le_bytes());
                put_str(&mut out, name);
            }
            Frame::Ping(t) => {
                out.push(2);
                out.extend_from_slice(&t.to_le_bytes());
            }
            Frame::Pong(t) => {
                out.push(3);
                out.extend_from_slice(&t.to_le_bytes());
            }
            Frame::Data(packet) => {
                out.push(4);
                packet.encode(&mut out);
            }
            Frame::Bye(reason) => {
                out.push(5);
                let (code, text) = match reason {
                    Reason::Left => (0, ""),
                    Reason::Closed => (1, ""),
                    Reason::Kicked(t) => (2, t.as_str()),
                    Reason::ServerFull => (3, ""),
                    Reason::WrongGame(t) => (4, t.as_str()),
                    Reason::WrongVersion(t) => (5, t.as_str()),
                    // Never sent; the other side finds these out itself.
                    Reason::TimedOut | Reason::Failed(_) | Reason::Lost(_) => (0, ""),
                };
                out.push(code);
                put_str(&mut out, text);
            }
        }
        out
    }

    fn decode(data: &[u8]) -> Option<Self> {
        let mut r = Reader::new(data);
        let frame = match r.u8()? {
            0 => Frame::Hello { protocol: r.u16()?, game: r.str()?, version: r.str()?, name: r.str()? },
            1 => Frame::Welcome { id: r.u32()?, name: r.str()? },
            2 => Frame::Ping(r.u64()?),
            3 => Frame::Pong(r.u64()?),
            4 => Frame::Data(Packet::decode(&mut r)?),
            5 => {
                let code = r.u8()?;
                let text = r.str()?;
                Frame::Bye(match code {
                    1 => Reason::Closed,
                    2 => Reason::Kicked(text),
                    3 => Reason::ServerFull,
                    4 => Reason::WrongGame(text),
                    5 => Reason::WrongVersion(text),
                    _ => Reason::Left,
                })
            }
            _ => return None,
        };
        Some(frame)
    }
}

/// The unencrypted key message: magic plus a public key.
fn key_message(public: &[u8; 32]) -> Message {
    Message::binary([&MAGIC[..], public].concat())
}

fn parse_key(data: &[u8]) -> Option<[u8; 32]> {
    data.strip_prefix(&MAGIC[..])?.try_into().ok()
}

/// What the background threads report to the game thread.
pub(crate) enum Event {
    /// Server: a client said hello and waits for an answer.
    Hello { link: Link, protocol: u16, game: String, version: String, name: String },
    /// Client: the server accepted us.
    Welcome { link: Link, id: u32, name: String },
    Data { peer: u32, packet: Packet },
    Ping { peer: u32, seconds: f32 },
    Closed { peer: u32, reason: Reason },
}

/// What the game thread hands to a connection thread.
pub(crate) enum Out {
    /// An encoded frame, shared by all receivers of a broadcast. Encrypted per connection.
    Bytes(Arc<[u8]>),
    /// Send what is queued, then close the connection.
    Close,
}

/// The game thread's handle to one connection.
pub(crate) struct Link {
    pub(crate) id: u32,
    out: Sender<Out>,
}

impl Link {
    pub(crate) fn send(&self, bytes: Arc<[u8]>) {
        let _ = self.out.send(Out::Bytes(bytes));
    }

    pub(crate) fn send_frame(&self, frame: &Frame) {
        self.send(frame.encode().into());
    }

    /// Sends `bye` (if any), then closes the connection.
    pub(crate) fn close(&self, bye: Option<Reason>) {
        if let Some(reason) = bye {
            self.send_frame(&Frame::Bye(reason));
        }
        let _ = self.out.send(Out::Close);
    }
}

fn ws_config() -> WebSocketConfig {
    // Room for the encryption tag on top of the largest frame.
    WebSocketConfig::default().max_message_size(Some(MAX_FRAME + 64)).max_frame_size(Some(MAX_FRAME + 64))
}

fn is_timeout(e: &tungstenite::Error) -> bool {
    matches!(e, tungstenite::Error::Io(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut))
}

/// An open, encrypted connection. `control` is a second handle to the same socket, used to
/// change the read timeout under the WebSocket (and its TLS layer).
struct Conn<S: Read + Write> {
    ws: WebSocket<S>,
    cipher: Cipher,
    control: TcpStream,
}

impl<S: Read + Write> Conn<S> {
    /// Queues one frame; [`WebSocket::flush`] sends it.
    fn write(&mut self, plain: &[u8]) -> tungstenite::Result<()> {
        let sealed = self.cipher.seal(plain);
        self.ws.write(Message::binary(sealed))
    }

    fn send(&mut self, frame: &Frame) -> tungstenite::Result<()> {
        self.write(&frame.encode())?;
        self.ws.flush()
    }

    /// Reads the next frame during the handshake (blocking up to the read timeout).
    fn read_frame(&mut self) -> Result<Frame, Reason> {
        let data = read_binary(&mut self.ws)?;
        self.cipher
            .open(&data)
            .and_then(|plain| Frame::decode(&plain))
            .ok_or_else(|| Reason::Failed("the other side sent invalid data".into()))
    }

    /// Closes the WebSocket and waits briefly for the other side, so a bye sent just before
    /// is not lost. Expects the socket to be non-blocking.
    fn close(mut self) {
        let _ = self.ws.close(None);
        let deadline = Instant::now() + CLOSE_WAIT;
        while Instant::now() < deadline {
            let _ = self.ws.flush();
            match self.ws.read() {
                Ok(_) => {}
                Err(e) if is_timeout(&e) => thread::sleep(POLL),
                Err(_) => break,
            }
        }
        let _ = self.control.shutdown(std::net::Shutdown::Both);
    }
}

/// The next binary message; other messages (WebSocket pings, text) are skipped.
fn read_binary<S: Read + Write>(ws: &mut WebSocket<S>) -> Result<Vec<u8>, Reason> {
    loop {
        match ws.read() {
            Ok(Message::Binary(data)) => return Ok(data.to_vec()),
            Ok(Message::Close(_)) => return Err(Reason::Failed("the other side closed the connection".into())),
            Ok(_) => {}
            Err(e) if is_timeout(&e) => return Err(Reason::Failed("the other side did not answer".into())),
            Err(e) => return Err(Reason::Failed(e.to_string())),
        }
    }
}

/// Runs an open connection until it ends: sends what the game queued, pings every second,
/// answers pings directly (so the measured time does not depend on the game's frame rate) and
/// reports everything else.
///
/// The socket is non-blocking from here on. (A read timeout cannot be used instead: on Windows
/// a socket whose read timed out is broken.) The thread reads everything that arrived, then
/// waits up to [`POLL`] for the game to queue something.
fn run<S: Read + Write>(mut conn: Conn<S>, peer: u32, events: Sender<Event>, queue: Receiver<Out>) {
    if conn.control.set_nonblocking(true).is_err() {
        let _ = events.send(Event::Closed { peer, reason: Reason::Lost("the connection could not be set up".into()) });
        return;
    }
    let epoch = Instant::now();
    let mut next_ping = epoch + PING_EVERY;
    let mut heard = epoch;
    let lost = |e: tungstenite::Error| match e {
        tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed => {
            Reason::Lost("the connection was closed".into())
        }
        e => Reason::Lost(e.to_string()),
    };
    // Writing may report "would block": the data then waits in the WebSocket's buffer and goes
    // out with a later flush.
    let written = |result: tungstenite::Result<()>| match result {
        Err(e) if !is_timeout(&e) => Err(lost(e)),
        _ => Ok(()),
    };
    let reason = 'conn: loop {
        loop {
            let event = match conn.ws.read() {
                Ok(Message::Binary(data)) => {
                    heard = Instant::now();
                    let Some(frame) = conn.cipher.open(&data).and_then(|plain| Frame::decode(&plain)) else {
                        break 'conn Reason::Lost("the other side sent invalid or changed data".into());
                    };
                    match frame {
                        Frame::Ping(t) => {
                            if let Err(reason) = written(conn.write(&Frame::Pong(t).encode())) {
                                break 'conn reason;
                            }
                            continue;
                        }
                        Frame::Pong(t) => {
                            let seconds = epoch.elapsed().saturating_sub(Duration::from_micros(t)).as_secs_f32();
                            Event::Ping { peer, seconds }
                        }
                        Frame::Data(packet) => Event::Data { peer, packet },
                        Frame::Bye(reason) => break 'conn reason,
                        Frame::Hello { .. } | Frame::Welcome { .. } => continue,
                    }
                }
                Ok(Message::Close(_)) => break 'conn Reason::Lost("the connection was closed".into()),
                Ok(_) => {
                    heard = Instant::now();
                    continue;
                }
                Err(e) if is_timeout(&e) => break,
                Err(e) => break 'conn lost(e),
            };
            if events.send(event).is_err() {
                break 'conn Reason::Left; // the game closed the network; nobody listens any more
            }
        }
        if heard.elapsed() > TIMEOUT {
            break Reason::TimedOut;
        }
        if Instant::now() >= next_ping {
            next_ping += PING_EVERY;
            let micros = epoch.elapsed().as_micros() as u64;
            if let Err(reason) = written(conn.write(&Frame::Ping(micros).encode())) {
                break reason;
            }
        }
        if let Err(reason) = written(conn.ws.flush()) {
            break reason;
        }
        let mut next = queue.recv_timeout(POLL).map_err(|e| match e {
            RecvTimeoutError::Timeout => TryRecvError::Empty,
            RecvTimeoutError::Disconnected => TryRecvError::Disconnected,
        });
        loop {
            match next {
                Ok(Out::Bytes(bytes)) => {
                    if let Err(reason) = written(conn.write(&bytes)) {
                        break 'conn reason;
                    }
                }
                Ok(Out::Close) | Err(TryRecvError::Disconnected) => {
                    // Closed by our own game: it already knows, so no event.
                    conn.close();
                    return;
                }
                Err(TryRecvError::Empty) => break,
            }
            next = queue.try_recv();
        }
        if let Err(reason) = written(conn.ws.flush()) {
            break reason;
        }
    };
    conn.close();
    let _ = events.send(Event::Closed { peer, reason });
}

fn prepare(stream: &TcpStream) -> io::Result<TcpStream> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
    stream.try_clone()
}

/// Accepts clients until `stop` is set. Each client gets its own thread.
pub(crate) fn accept_loop(listener: TcpListener, stop: Arc<AtomicBool>, events: Sender<Event>) {
    let mut next_id = 1;
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let (id, events) = (next_id, events.clone());
                next_id += 1;
                let _ = thread::Builder::new()
                    .name(format!("audio-code net connection {id}"))
                    .spawn(move || serve(stream, id, events));
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(15)),
            Err(_) => thread::sleep(Duration::from_millis(100)),
        }
    }
}

/// Server side of one connection. Anything that is not one of our clients is dropped
/// without a word.
fn serve(stream: TcpStream, id: u32, events: Sender<Event>) {
    let Ok(control) = prepare(&stream) else {
        return;
    };
    let Ok(mut ws) = tungstenite::accept_with_config(stream, Some(ws_config())) else {
        return;
    };
    let Some(client_key) = read_binary(&mut ws).ok().and_then(|data| parse_key(&data)) else {
        return;
    };
    let Ok(keys) = KeyPair::new() else {
        return;
    };
    if ws.send(key_message(&keys.public)).is_err() {
        return;
    }
    let Some(cipher) = Cipher::new(keys, client_key, false) else {
        return;
    };
    let mut conn = Conn { ws, cipher, control };
    let Ok(Frame::Hello { protocol, game, version, name }) = conn.read_frame() else {
        return;
    };
    let (out, queue) = std::sync::mpsc::channel();
    let link = Link { id, out };
    if events.send(Event::Hello { link, protocol, game, version, name }).is_ok() {
        run(conn, id, events, queue);
    }
}

/// Connects to a server, exchanges keys, says hello and then runs like any other connection.
/// Reports `Welcome` on success or `Closed` with the reason on failure.
pub(crate) fn connect(address: String, hello: Frame, events: Sender<Event>) {
    let target = match Target::parse(&address) {
        Ok(target) => target,
        Err(reason) => {
            let _ = events.send(Event::Closed { peer: 0, reason });
            return;
        }
    };
    match handshake(&target, &hello) {
        Ok((conn, id, name)) => {
            let (out, queue) = std::sync::mpsc::channel();
            if events.send(Event::Welcome { link: Link { id: 0, out }, id, name }).is_ok() {
                run(conn, 0, events, queue);
            }
        }
        Err(reason) => {
            let _ = events.send(Event::Closed { peer: 0, reason });
        }
    }
}

#[cfg(feature = "tls")]
type ClientStream = tungstenite::stream::MaybeTlsStream<TcpStream>;
#[cfg(not(feature = "tls"))]
type ClientStream = TcpStream;

fn handshake(target: &Target, hello: &Frame) -> Result<(Conn<ClientStream>, u32, String), Reason> {
    let failed = |e: io::Error| Reason::Failed(e.to_string());
    let stream = open(&target.host, target.port)?;
    let control = prepare(&stream).map_err(failed)?;
    let mut ws = open_ws(target, stream)?;
    let keys = KeyPair::new().map_err(failed)?;
    ws.send(key_message(&keys.public)).map_err(|e| Reason::Failed(e.to_string()))?;
    let unexpected = || Reason::Failed("the server answered with something unexpected".into());
    let server_key = parse_key(&read_binary(&mut ws)?).ok_or_else(unexpected)?;
    let cipher = Cipher::new(keys, server_key, true).ok_or_else(unexpected)?;
    let mut conn = Conn { ws, cipher, control };
    conn.send(hello).map_err(|e| Reason::Failed(e.to_string()))?;
    loop {
        match conn.read_frame()? {
            Frame::Welcome { id, name } => return Ok((conn, id, name)),
            Frame::Bye(reason) => return Err(reason),
            Frame::Ping(_) | Frame::Pong(_) => {}
            _ => return Err(unexpected()),
        }
    }
}

fn handshake_failed<R: tungstenite::handshake::HandshakeRole>(e: tungstenite::HandshakeError<R>) -> Reason {
    match e {
        tungstenite::HandshakeError::Failure(e) if !is_timeout(&e) => Reason::Failed(e.to_string()),
        _ => Reason::Failed("the server did not answer".into()),
    }
}

/// The WebSocket handshake, with TLS for `wss://`.
#[cfg(feature = "tls")]
fn open_ws(target: &Target, stream: TcpStream) -> Result<WebSocket<ClientStream>, Reason> {
    let connector = if target.tls {
        let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
        let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|e| Reason::Failed(e.to_string()))?
            .with_root_certificates(roots)
            .with_no_client_auth();
        tungstenite::Connector::Rustls(Arc::new(config))
    } else {
        tungstenite::Connector::Plain
    };
    tungstenite::client_tls_with_config(target.url.as_str(), stream, Some(ws_config()), Some(connector))
        .map(|(ws, _)| ws)
        .map_err(handshake_failed)
}

/// The WebSocket handshake. `wss://` needs the `tls` feature.
#[cfg(not(feature = "tls"))]
fn open_ws(target: &Target, stream: TcpStream) -> Result<WebSocket<ClientStream>, Reason> {
    if target.tls {
        return Err(Reason::Failed("wss:// addresses need the `tls` feature of audio-code".into()));
    }
    tungstenite::client::client_with_config(target.url.as_str(), stream, Some(ws_config()))
        .map(|(ws, _)| ws)
        .map_err(handshake_failed)
}

/// Where to connect: the WebSocket URL plus the host and port for the TCP connection.
#[derive(Debug, PartialEq)]
struct Target {
    url: String,
    host: String,
    port: u16,
    tls: bool,
}

impl Target {
    /// `"example.com"`, `"10.0.0.2"` and `"::1"` get the default port; `"host:port"` and
    /// `"[::1]:port"` keep theirs. `ws://` and `wss://` URLs are used as they are, with the
    /// usual ports 80 and 443 when they have none.
    fn parse(address: &str) -> Result<Self, Reason> {
        let address = address.trim();
        let invalid = || Reason::Failed(format!("{address}: not a valid address"));
        let (tls, rest, default_port) = if let Some(rest) = address.strip_prefix("wss://") {
            (true, rest, 443)
        } else if let Some(rest) = address.strip_prefix("ws://") {
            (false, rest, 80)
        } else {
            (false, address, DEFAULT_PORT)
        };
        let (authority, path) = match rest.find('/') {
            Some(i) => rest.split_at(i),
            None => (rest, "/"),
        };
        let (host, port) = if authority.parse::<std::net::Ipv6Addr>().is_ok() {
            (authority.to_string(), default_port)
        } else if let Some(v6) = authority.strip_prefix('[') {
            let (host, after) = v6.split_once(']').ok_or_else(invalid)?;
            let port = match after.strip_prefix(':') {
                Some(p) => p.parse().map_err(|_| invalid())?,
                None if after.is_empty() => default_port,
                None => return Err(invalid()),
            };
            (host.to_string(), port)
        } else {
            match authority.rsplit_once(':') {
                Some((host, port)) => (host.to_string(), port.parse().map_err(|_| invalid())?),
                None => (authority.to_string(), default_port),
            }
        };
        if host.is_empty() {
            return Err(invalid());
        }
        let shown = if host.contains(':') { format!("[{host}]") } else { host.clone() };
        let scheme = if tls { "wss" } else { "ws" };
        Ok(Self { url: format!("{scheme}://{shown}:{port}{path}"), host, port, tls })
    }
}

/// Resolves the host and tries every address, IPv4 first.
fn open(host: &str, port: u16) -> Result<TcpStream, Reason> {
    let mut addrs: Vec<SocketAddr> =
        (host, port).to_socket_addrs().map_err(|e| Reason::Failed(format!("{host}: {e}")))?.collect();
    addrs.sort_by_key(|a| a.is_ipv6());
    let mut last = Reason::Failed(format!("{host}: no address found"));
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, HANDSHAKE_TIMEOUT) {
            Ok(stream) => return Ok(stream),
            Err(e) => last = Reason::Failed(e.to_string()),
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        let frames = [
            Frame::Hello { protocol: PROTOCOL, game: "g".into(), version: "1".into(), name: "Anna".into() },
            Frame::Welcome { id: 4, name: "Host".into() },
            Frame::Ping(99),
            Frame::Data(Packet::new("chat").with("hi")),
            Frame::Bye(Reason::Kicked("spam".into())),
        ];
        for frame in frames {
            let bytes = frame.encode();
            assert_eq!(Frame::decode(&bytes).unwrap().encode(), bytes);
        }
        assert!(Frame::decode(b"\x09GET / HTTP/1.1").is_none());
        assert!(parse_key(b"GET / HTTP/1.1").is_none());
    }

    #[test]
    fn addresses() {
        let t = |a: &str| Target::parse(a).map(|t| (t.url, t.host, t.port, t.tls));
        assert_eq!(t("example.com"), Ok(("ws://example.com:7777/".into(), "example.com".into(), 7777, false)));
        assert_eq!(t("10.0.0.2:5000"), Ok(("ws://10.0.0.2:5000/".into(), "10.0.0.2".into(), 5000, false)));
        assert_eq!(t("::1"), Ok(("ws://[::1]:7777/".into(), "::1".into(), 7777, false)));
        assert_eq!(t("[::1]:5000"), Ok(("ws://[::1]:5000/".into(), "::1".into(), 5000, false)));
        assert_eq!(t("ws://example.com/game"), Ok(("ws://example.com:80/game".into(), "example.com".into(), 80, false)));
        assert_eq!(t("wss://example.com:8443/x"), Ok(("wss://example.com:8443/x".into(), "example.com".into(), 8443, true)));
        assert!(t("example.com:port").is_err());
        assert!(t("").is_err());
    }
}
