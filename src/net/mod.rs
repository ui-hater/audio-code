//! Online play: host a game or join one over the internet or the local network.
//! Available as `ctx.net`.
//!
//! One player (or a dedicated server, see [`Engine::run_server`](crate::Engine::run_server))
//! hosts, the others join. Everyone sends [`Packet`]s: a kind and a few values. What
//! arrives shows up as [`NetEvent`]s for one frame, like key presses.
//!
//! ```no_run
//! use audio_code::prelude::*;
//!
//! fn main() -> audio_code::Result<()> {
//!     audio_code::run("Chat", |ctx| {
//!         if ctx.once("start") {
//!             ctx.net.set_name("Anna");
//!             ctx.net.join("localhost"); // or: ctx.net.host(net::DEFAULT_PORT)
//!         }
//!         for player in ctx.net.joined() {
//!             ctx.speak(format!("Connected to {}", ctx.net.name(player)));
//!         }
//!         if ctx.pressed(Key::Space) {
//!             ctx.net.send_all(Packet::new("chat").with("Hello!"));
//!         }
//!         for (from, packet) in ctx.net.received("chat") {
//!             ctx.speak(format!("{}: {}", ctx.net.name(from), packet.text(0)));
//!         }
//!         for (_, reason) in ctx.net.left() {
//!             ctx.speak(ctx.lang.describe_reason(&reason));
//!         }
//!     })
//! }
//! ```
//!
//! ## What the engine does for you
//!
//! * Connecting, the handshake and all sending and receiving run on background threads.
//!   The game never waits for the network.
//! * Only clients of the same game (the [`Engine`](crate::Engine) title) and the same
//!   version ([`Net::set_version`]) get in. Others are turned away with
//!   [`Reason::WrongGame`] / [`Reason::WrongVersion`].
//! * Both sides ping each other every second ([`Net::ping`]). A connection that hears
//!   nothing for 10 seconds is closed with [`Reason::TimedOut`].
//! * Packets arrive complete, unchanged and in the order they were sent.
//! * Mistakes do not crash: sending while offline does nothing, joining a wrong address
//!   ends with a [`NetEvent::Disconnected`] that says why.
//!
//! ## WebSockets and encryption
//!
//! Connections are WebSockets, so they pass firewalls and web proxies that only let web
//! traffic through, and a server can sit behind a web server such as nginx or Caddy.
//! [`Net::join`] takes plain addresses (`"example.com"`, `"10.0.0.2:7777"`) as well as
//! `ws://` and `wss://` URLs.
//!
//! Everything is **always encrypted**, without any setup: both sides make fresh keys for
//! every connection (X25519) and encrypt every message (XChaCha20-Poly1305). Nobody who
//! listens in can read names, chat or game data, and changed messages are detected. This
//! does not prove *who* the server is, though: someone who can redirect the traffic could
//! pose as it. To rule that out, put the server behind a web server with a real TLS
//! certificate and join with `wss://` (feature `tls`, on by default).

mod crypto;
mod packet;
mod wire;

use std::collections::BTreeMap;
use std::fmt;
use std::net::{IpAddr, TcpListener, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, mpsc};

pub use packet::{Arg, Packet};

use crate::error::{Error, Result};
use wire::{Event, Frame, Link};

/// The port used when none is given: `ctx.net.join("example.com")` connects to it.
pub const DEFAULT_PORT: u16 = 7777;

/// Names are cut to this many characters.
const MAX_NAME: usize = 32;

/// A connected player. On a client the server is [`PeerId::SERVER`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct PeerId(u32);

impl PeerId {
    /// The host. Clients receive everything from this peer. The host itself has this id too.
    pub const SERVER: PeerId = PeerId(0);

    /// The number behind the id. The server is 0, clients are numbered from 1 in the order
    /// they joined. Numbers are not reused while the server runs.
    pub fn number(self) -> u32 {
        self.0
    }

    /// The id with this number, e.g. one read from a packet with [`Packet::peer`].
    pub fn from_number(number: u32) -> Self {
        Self(number)
    }

    /// Whether this is [`PeerId::SERVER`].
    pub fn is_server(self) -> bool {
        self == Self::SERVER
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Why a connection ended. Speak it with `ctx.lang.describe_reason(&reason)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// The player left on purpose ([`Net::close`] or quitting the game).
    Left,
    /// The host closed the server.
    Closed,
    /// The host kicked this player, with the text given to [`Net::kick`].
    Kicked(String),
    /// The server already has the maximum number of players ([`Net::set_max_players`]).
    ServerFull,
    /// The server runs another game; contains its title.
    WrongGame(String),
    /// The server runs another version of the game; contains its version.
    WrongVersion(String),
    /// Nothing was heard from the other side for 10 seconds.
    TimedOut,
    /// Joining failed: wrong address, no server running there, no network.
    Failed(String),
    /// The connection broke; contains the system's description.
    Lost(String),
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&crate::lang::Lang::new().describe_reason(self))
    }
}

/// Something that happened on the network this frame.
#[derive(Debug, Clone, PartialEq)]
pub enum NetEvent {
    /// Host: a player joined. Client: the connection to the server is ready
    /// (the peer is [`PeerId::SERVER`]).
    Connected(PeerId),
    /// A packet arrived.
    Message(PeerId, Packet),
    /// Host: a player left. Client: the connection to the server ended or could not be made.
    Disconnected(PeerId, Reason),
}

enum State {
    Offline,
    Hosting { port: u16, stop: Arc<AtomicBool> },
    Joining,
    Joined,
}

struct Peer {
    name: String,
    link: Link,
    ping: f32,
}

/// The network connection: `ctx.net`. See the [module docs](self).
pub struct Net {
    state: State,
    events_rx: Option<Receiver<Event>>,
    peers: BTreeMap<PeerId, Peer>,
    pub(crate) events: Vec<NetEvent>,
    /// Events made by the game itself (kick, failed start), shown next frame.
    pending: Vec<NetEvent>,
    game: String,
    version: String,
    name: String,
    max_players: usize,
    id: PeerId,
    warned_offline: bool,
}

impl fmt::Debug for Net {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = match self.state {
            State::Offline => "offline",
            State::Hosting { .. } => "hosting",
            State::Joining => "joining",
            State::Joined => "joined",
        };
        f.debug_struct("Net").field("state", &state).field("players", &self.peers.len()).finish_non_exhaustive()
    }
}

impl Net {
    /// An offline network for the game `game`. Only clients with the same game name can join.
    /// The engine creates this as `ctx.net` with the game title.
    pub fn new(game: impl Into<String>) -> Self {
        Self {
            state: State::Offline,
            events_rx: None,
            peers: BTreeMap::new(),
            events: Vec::new(),
            pending: Vec::new(),
            game: game.into(),
            version: String::new(),
            name: "Player".into(),
            max_players: 32,
            id: PeerId::SERVER,
            warned_offline: false,
        }
    }

    /// The name others see (default `"Player"`, at most 32 characters). Set it before
    /// [`Net::host`] or [`Net::join`].
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = clean_name(&name.into());
    }

    /// The version of your game, e.g. `"1.2"` (default empty). Clients with another version
    /// cannot join, so old and new game code never meet. Set it before hosting or joining.
    pub fn set_version(&mut self, version: impl Into<String>) {
        self.version = version.into();
    }

    /// How many clients may join (default 32). The host does not count.
    pub fn set_max_players(&mut self, max: usize) {
        self.max_players = max;
    }

    /// Starts a WebSocket server on `port` that others can join. Port 0 picks a free port
    /// (see [`Net::port`]). Ends any connection that is open. Fails when the port is in use.
    /// Any URL path is accepted, so a web server can pass on `wss://example.com/game`.
    ///
    /// Players outside your local network need the host's public address, and the port
    /// must be forwarded in the host's router.
    pub fn host(&mut self, port: u16) -> Result<()> {
        self.close();
        let listener = TcpListener::bind(("0.0.0.0", port))
            .map_err(|e| Error::Net(format!("could not host on port {port}: {e}")))?;
        let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let tx = self.new_channel();
        let flag = stop.clone();
        std::thread::Builder::new()
            .name("audio-code net server".into())
            .spawn(move || wire::accept_loop(listener, flag, tx))?;
        self.state = State::Hosting { port, stop };
        self.id = PeerId::SERVER;
        Ok(())
    }

    /// Joins the server at `address`: a host name or IP address, with or without port
    /// (`"example.com"`, `"192.168.1.20:7777"`, `"[::1]:7777"`), or a WebSocket URL
    /// (`"ws://example.com:7777/"`, `"wss://example.com/game"`). Without a port,
    /// [`DEFAULT_PORT`] is used (URLs: 80 for `ws://`, 443 for `wss://`). Ends any connection
    /// that is open.
    ///
    /// Returns at once. Later a [`NetEvent::Connected`] says the connection is ready, or a
    /// [`NetEvent::Disconnected`] says why it failed (after at most about 10 seconds).
    pub fn join(&mut self, address: &str) {
        self.close();
        let tx = self.new_channel();
        let hello = Frame::Hello {
            protocol: wire::PROTOCOL,
            game: self.game.clone(),
            version: self.version.clone(),
            name: self.name.clone(),
        };
        let address = address.to_string();
        let spawned = std::thread::Builder::new()
            .name("audio-code net client".into())
            .spawn(move || wire::connect(address, hello, tx));
        if let Err(e) = spawned {
            self.pending.push(NetEvent::Disconnected(PeerId::SERVER, Reason::Failed(e.to_string())));
            return;
        }
        self.state = State::Joining;
    }

    /// Leaves the game, or stops the server and sends every player away with
    /// [`Reason::Closed`]. Called automatically when the game quits. Does nothing when
    /// offline. Produces no events.
    pub fn close(&mut self) {
        let bye = if self.is_host() { Reason::Closed } else { Reason::Left };
        for peer in std::mem::take(&mut self.peers).into_values() {
            peer.link.close(Some(bye.clone()));
        }
        if let State::Hosting { stop, .. } = &self.state {
            stop.store(true, Ordering::Relaxed);
        }
        self.state = State::Offline;
        self.events_rx = None;
        self.id = PeerId::SERVER;
        self.warned_offline = false;
    }

    /// Host: sends a player away. They get [`Reason::Kicked`] with `reason` (may be empty).
    /// The host gets a [`NetEvent::Disconnected`] for them next frame.
    pub fn kick(&mut self, peer: PeerId, reason: &str) {
        if !self.is_host() {
            return;
        }
        if let Some(p) = self.peers.remove(&peer) {
            p.link.close(Some(Reason::Kicked(reason.to_string())));
            self.pending.push(NetEvent::Disconnected(peer, Reason::Kicked(reason.to_string())));
        }
    }

    /// Sends a packet to one peer. On a client, the only peer is [`PeerId::SERVER`]. Does
    /// nothing if the peer is not connected.
    pub fn send(&mut self, peer: PeerId, packet: impl Into<Packet>) {
        let bytes = encode(packet.into());
        match self.peers.get(&peer) {
            Some(p) => p.link.send(bytes),
            None => self.warn_offline(),
        }
    }

    /// Sends a packet to every connected peer. On a client that is the server.
    pub fn send_all(&mut self, packet: impl Into<Packet>) {
        self.send_filtered(packet.into(), |_| true);
    }

    /// Sends a packet to every connected peer except one. The usual way for a host to pass
    /// on what one player did to everybody else.
    pub fn send_all_except(&mut self, except: PeerId, packet: impl Into<Packet>) {
        self.send_filtered(packet.into(), |p| p != except);
    }

    fn send_filtered(&mut self, packet: Packet, keep: impl Fn(PeerId) -> bool) {
        if self.peers.is_empty() {
            if !self.is_host() {
                self.warn_offline();
            }
            return;
        }
        let bytes = encode(packet);
        for (_, p) in self.peers.iter().filter(|(id, _)| keep(**id)) {
            p.link.send(bytes.clone());
        }
    }

    fn warn_offline(&mut self) {
        if !self.warned_offline && !matches!(self.state, State::Joined | State::Hosting { .. }) {
            self.warned_offline = true;
            eprintln!("audio-code: a packet was sent while not connected; it was dropped");
        }
    }

    /// Everything that happened on the network this frame, in order.
    pub fn events(&self) -> &[NetEvent] {
        &self.events
    }

    /// Players who connected this frame. On a client: [`PeerId::SERVER`] when the
    /// connection became ready.
    pub fn joined(&self) -> Vec<PeerId> {
        self.events
            .iter()
            .filter_map(|e| match e {
                NetEvent::Connected(p) => Some(*p),
                _ => None,
            })
            .collect()
    }

    /// Players who left this frame and why. On a client: [`PeerId::SERVER`] when the
    /// connection ended or could not be made.
    pub fn left(&self) -> Vec<(PeerId, Reason)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                NetEvent::Disconnected(p, r) => Some((*p, r.clone())),
                _ => None,
            })
            .collect()
    }

    /// All packets that arrived this frame, with their sender.
    pub fn messages(&self) -> Vec<(PeerId, Packet)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                NetEvent::Message(p, packet) => Some((*p, packet.clone())),
                _ => None,
            })
            .collect()
    }

    /// The packets of one kind that arrived this frame, with their sender.
    pub fn received(&self, kind: &str) -> Vec<(PeerId, Packet)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                NetEvent::Message(p, packet) if packet.is(kind) => Some((*p, packet.clone())),
                _ => None,
            })
            .collect()
    }

    /// The connected peers in the order they joined. Host: all clients. Client: the server,
    /// once connected.
    pub fn players(&self) -> Vec<PeerId> {
        self.peers.keys().copied().collect()
    }

    /// Number of connected peers (the host does not count itself).
    pub fn player_count(&self) -> usize {
        self.peers.len()
    }

    /// Whether `peer` is connected right now.
    pub fn is_connected_to(&self, peer: PeerId) -> bool {
        self.peers.contains_key(&peer)
    }

    /// The name a peer chose with [`Net::set_name`], or `""` if not connected. Your own
    /// [`Net::id`] gives your own name.
    pub fn name(&self, peer: PeerId) -> &str {
        if peer == self.id && !matches!(self.state, State::Offline | State::Joining) {
            return &self.name;
        }
        self.peers.get(&peer).map_or("", |p| p.name.as_str())
    }

    /// The round trip time to a peer in seconds: how long a message takes there and back.
    /// Updated every second; 0.0 until measured or when not connected.
    pub fn ping(&self, peer: PeerId) -> f32 {
        self.peers.get(&peer).map_or(0.0, |p| p.ping)
    }

    /// Your own id: [`PeerId::SERVER`] for the host, the number the server gave you on a
    /// client.
    pub fn id(&self) -> PeerId {
        self.id
    }

    /// Whether this game is hosting.
    pub fn is_host(&self) -> bool {
        matches!(self.state, State::Hosting { .. })
    }

    /// Whether this game is a client that is connected to a server.
    pub fn is_client(&self) -> bool {
        matches!(self.state, State::Joined)
    }

    /// Whether a [`Net::join`] is still in progress.
    pub fn is_joining(&self) -> bool {
        matches!(self.state, State::Joining)
    }

    /// Whether this game is hosting or connected to a server.
    pub fn is_online(&self) -> bool {
        self.is_host() || self.is_client()
    }

    /// The port the server listens on while hosting.
    pub fn port(&self) -> Option<u16> {
        match self.state {
            State::Hosting { port, .. } => Some(port),
            _ => None,
        }
    }

    /// This computer's address in the local network, e.g. `192.168.1.20`: what players in
    /// the same network type to join. `None` without a network. Sends nothing.
    pub fn local_ip() -> Option<IpAddr> {
        let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
        // Connecting a UDP socket only picks the outgoing interface.
        socket.connect("8.8.8.8:80").ok()?;
        socket.local_addr().ok().map(|a| a.ip()).filter(|ip| !ip.is_unspecified())
    }

    fn new_channel(&mut self) -> Sender<Event> {
        let (tx, rx) = mpsc::channel();
        self.events_rx = Some(rx);
        tx
    }

    /// Collects what the background threads reported. Called by the engine every frame.
    pub(crate) fn update(&mut self) {
        self.events = std::mem::take(&mut self.pending);
        let Some(rx) = &self.events_rx else {
            return;
        };
        let reports: Vec<Event> = rx.try_iter().collect();
        for report in reports {
            self.handle(report);
        }
    }

    fn handle(&mut self, report: Event) {
        match report {
            Event::Hello { link, protocol, game, version, name } => {
                if !self.is_host() {
                    link.close(None);
                    return;
                }
                let refusal = if game != self.game {
                    Some(Reason::WrongGame(self.game.clone()))
                } else if version != self.version || protocol != wire::PROTOCOL {
                    Some(Reason::WrongVersion(self.version.clone()))
                } else if self.peers.len() >= self.max_players {
                    Some(Reason::ServerFull)
                } else {
                    None
                };
                if let Some(reason) = refusal {
                    link.close(Some(reason));
                    return;
                }
                let peer = PeerId(link.id);
                link.send_frame(&Frame::Welcome { id: link.id, name: self.name.clone() });
                self.peers.insert(peer, Peer { name: clean_name(&name), link, ping: 0.0 });
                self.events.push(NetEvent::Connected(peer));
            }
            Event::Welcome { link, id, name } => {
                self.state = State::Joined;
                self.id = PeerId(id);
                self.peers.insert(PeerId::SERVER, Peer { name: clean_name(&name), link, ping: 0.0 });
                self.events.push(NetEvent::Connected(PeerId::SERVER));
            }
            Event::Data { peer, packet } => {
                let peer = PeerId(peer);
                if self.peers.contains_key(&peer) {
                    self.events.push(NetEvent::Message(peer, packet));
                }
            }
            Event::Ping { peer, seconds } => {
                if let Some(p) = self.peers.get_mut(&PeerId(peer)) {
                    p.ping = seconds;
                }
            }
            Event::Closed { peer, reason } => {
                let peer = PeerId(peer);
                let known = self.peers.remove(&peer).is_some();
                let joining = self.is_joining() && peer == PeerId::SERVER;
                if known || joining {
                    self.events.push(NetEvent::Disconnected(peer, reason));
                }
                if !self.is_host() && peer == PeerId::SERVER {
                    self.state = State::Offline;
                    self.events_rx = None;
                    self.id = PeerId::SERVER;
                }
            }
        }
    }
}

impl Drop for Net {
    fn drop(&mut self) {
        self.close();
    }
}

fn encode(packet: Packet) -> Arc<[u8]> {
    let bytes = Frame::Data(packet).encode();
    if bytes.len() > wire::MAX_FRAME {
        eprintln!("audio-code: a packet of {} bytes is too large to send (at most 1 MB); it was dropped", bytes.len());
        return Arc::from(&[][..]);
    }
    bytes.into()
}

fn clean_name(name: &str) -> String {
    name.chars().filter(|c| !c.is_control()).take(MAX_NAME).collect::<String>().trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Updates all nets until `done` holds or eight seconds pass (Windows takes two seconds
    /// to report a refused connection).
    fn pump(nets: &mut [&mut Net], mut done: impl FnMut(&[&mut Net]) -> bool) -> Vec<Vec<NetEvent>> {
        let mut all = vec![Vec::new(); nets.len()];
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(8) {
            for (net, events) in nets.iter_mut().zip(&mut all) {
                net.update();
                events.extend(net.events.iter().cloned());
            }
            if done(nets) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        all
    }

    fn host() -> (Net, String) {
        let mut server = Net::new("game");
        server.set_name("Host");
        server.host(0).unwrap();
        let address = format!("127.0.0.1:{}", server.port().unwrap());
        (server, address)
    }

    #[test]
    fn join_chat_and_leave() {
        let (mut server, address) = host();
        let mut client = Net::new("game");
        client.set_name("Anna");
        client.join(&address);
        pump(&mut [&mut server, &mut client], |n| n[0].player_count() == 1 && n[1].is_client());
        let anna = server.players()[0];
        assert_eq!(server.name(anna), "Anna");
        assert_eq!(client.name(PeerId::SERVER), "Host");
        assert_eq!(client.id(), anna);
        assert_eq!(client.name(client.id()), "Anna");

        client.send_all(Packet::new("chat").with("hi"));
        let events = pump(&mut [&mut server, &mut client], |n| !n[0].received("chat").is_empty());
        assert_eq!(events[0], [NetEvent::Message(anna, Packet::new("chat").with("hi"))]);

        server.send(anna, "pong");
        let events = pump(&mut [&mut server, &mut client], |n| !n[1].messages().is_empty());
        assert_eq!(events[1], [NetEvent::Message(PeerId::SERVER, Packet::new("pong"))]);

        client.close();
        let events = pump(&mut [&mut server], |n| n[0].player_count() == 0);
        assert_eq!(events[0], [NetEvent::Disconnected(anna, Reason::Left)]);
    }

    #[test]
    fn refuses_wrong_game_version_and_full() {
        let (mut server, address) = host();
        server.set_version("2");
        server.set_max_players(1);
        let mut other = Net::new("other game");
        other.join(&address);
        let mut old = Net::new("game");
        old.set_version("1");
        old.join(&address);
        let events = pump(&mut [&mut server, &mut other, &mut old], |n| !n[1].is_joining() && !n[2].is_joining());
        assert_eq!(events[1], [NetEvent::Disconnected(PeerId::SERVER, Reason::WrongGame("game".into()))]);
        assert_eq!(events[2], [NetEvent::Disconnected(PeerId::SERVER, Reason::WrongVersion("2".into()))]);

        let mut first = Net::new("game");
        first.set_version("2");
        first.join(&address);
        pump(&mut [&mut server, &mut first], |n| n[1].is_client());
        let mut second = Net::new("game");
        second.set_version("2");
        second.join(&address);
        let events = pump(&mut [&mut server, &mut second], |n| !n[1].is_joining());
        assert_eq!(events[1], [NetEvent::Disconnected(PeerId::SERVER, Reason::ServerFull)]);
    }

    #[test]
    fn kick_and_server_close() {
        let (mut server, address) = host();
        let mut a = Net::new("game");
        let mut b = Net::new("game");
        a.join(&address);
        b.join(&address);
        pump(&mut [&mut server, &mut a, &mut b], |n| n[0].player_count() == 2 && n[1].is_client() && n[2].is_client());

        server.kick(a.id(), "spam");
        let events = pump(&mut [&mut server, &mut a], |n| !n[1].is_online());
        assert_eq!(events[1], [NetEvent::Disconnected(PeerId::SERVER, Reason::Kicked("spam".into()))]);

        server.close();
        let events = pump(&mut [&mut b], |n| !n[0].is_online());
        assert_eq!(events[0], [NetEvent::Disconnected(PeerId::SERVER, Reason::Closed)]);
    }

    #[test]
    fn join_failure_is_an_event() {
        // Find a port nobody listens on.
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let mut client = Net::new("game");
        client.join(&format!("127.0.0.1:{port}"));
        let events = pump(&mut [&mut client], |n| !n[0].is_joining());
        assert!(matches!(events[0].as_slice(), [NetEvent::Disconnected(PeerId::SERVER, Reason::Failed(_))]));
        // Sending while offline is ignored.
        client.send_all("hello");
    }
}
