# Net

Online play: `ctx.net` (type `Net`), plus `Packet`, `Arg`, `PeerId`, `NetEvent`, `Reason`
and `net::DEFAULT_PORT`. The guide is [online.md](../online.md).

One player hosts, the others join. Connecting, sending and receiving run on background
threads; the game never waits. Connections are WebSockets; packets arrive complete,
unchanged and in order. Everything is always encrypted (X25519 key exchange per
connection, XChaCha20-Poly1305 per message). Both sides ping every second, and a connection
that hears nothing for 10 seconds is closed.

```rust
use audio_code::prelude::*;   // Net, NetEvent, Packet, PeerId, Reason, net
use audio_code::net::Arg;     // only for matching on raw values
```

## Setup

### `ctx.net.set_name(name: impl Into<String>)`

The name others see. Default `"Player"`. Control characters are removed and the name is cut
to 32 characters. Call before `host` or `join`.

```rust
let name: String = ctx.storage.get_or("name", "Player".to_string());
ctx.net.set_name(name);
```

### `ctx.net.set_version(version: impl Into<String>)`

The version of your game. Default empty. A client with another version than the host is
turned away with `Reason::WrongVersion(host_version)`. Call before `host` or `join`.

```rust
ctx.net.set_version("1.2");
```

### `ctx.net.set_max_players(max: usize)`

How many clients may join. Default 32; the host does not count. Further clients get
`Reason::ServerFull`.

```rust
ctx.net.set_max_players(4);
```

### `Net::new(game: impl Into<String>) -> Net`

An offline network for the game `game`. The engine already creates `ctx.net` with the
game title; only clients with the same game name can join. Useful for tools and tests.

```rust
use audio_code::net::Net;
let mut net = Net::new("My game");
```

## Hosting and joining

### `ctx.net.host(port: u16) -> Result<()>`

Starts a WebSocket server others can join. Ends any open connection first. Port 0 picks a
free port (see `port`). Errors with `Error::Net` when the port is in use. Any URL path is
accepted, so a web server in front can pass on `wss://example.com/game`. Players outside
the local network need the host's public address and a forwarded port in the host's router.

```rust
match ctx.net.host(net::DEFAULT_PORT) {
    Ok(()) => ctx.speak("Waiting for players"),
    Err(e) => ctx.speak(e.to_string()),
}
```

### `ctx.net.join(address: &str)`

Joins a server. `address` is a host name or IP, with or without port (`"example.com"`,
`"192.168.1.20:7777"`, `"[::1]:7777"`), or a WebSocket URL (`"ws://example.com:7777/"`,
`"wss://example.com/game"`). Without a port, `net::DEFAULT_PORT` is used; URLs use 80 for
`ws://` and 443 for `wss://`. `wss://` checks the server's TLS certificate and needs the
`tls` feature (default on); without it, joining a `wss://` address ends with
`Reason::Failed`. Ends any open connection first. Returns at once: later a `NetEvent::Connected(PeerId::SERVER)`
says it worked, or a `NetEvent::Disconnected(PeerId::SERVER, reason)` says why not (after
at most about 10 seconds).

```rust
ctx.net.join("localhost");
ctx.net.join("wss://game.example.com/play");
ctx.speak("Connecting");
```

### `ctx.net.close()`

Leaves the game (the host sees `Reason::Left`), or stops the server (every client gets
`Reason::Closed`). Called automatically when the game quits. Produces no event for
yourself. Does nothing when offline.

```rust
if ctx.pressed(Key::Escape) {
    ctx.net.close();
    ctx.switch_scene(MainMenu);
}
```

### `ctx.net.kick(peer: PeerId, reason: &str)`

Host only: sends a player away. They get `Reason::Kicked(reason)`; `reason` may be empty.
The host gets `NetEvent::Disconnected(peer, Reason::Kicked(..))` next frame.

```rust
ctx.net.kick(player, "Please be nice in the chat");
```

### `net::DEFAULT_PORT: u16`

7777. Used by `join` when the address has no port.

```rust
ctx.net.host(net::DEFAULT_PORT)?;
```

### `Net::local_ip() -> Option<IpAddr>`

This computer's address in the local network (e.g. `192.168.1.20`): what players in the
same network type to join. `None` without a network. Sends nothing.

```rust
if let Some(ip) = Net::local_ip() {
    ctx.speak(format!("Your address is {ip}"));
}
```

## Sending

All sending functions take `impl Into<Packet>`: a `Packet`, a `&Packet`, or text for a
packet without values. Sending to a peer that is not connected does nothing. A packet larger
than 1 MB is dropped with a warning.

### `ctx.net.send(peer: PeerId, packet: impl Into<Packet>)`

Sends to one peer. On a client the only peer is `PeerId::SERVER`.

```rust
ctx.net.send(player, Packet::new("welcome").with("You are on the red team"));
```

### `ctx.net.send_all(packet: impl Into<Packet>)`

Sends to every connected peer. On a client: to the host.

```rust
ctx.net.send_all(Packet::new("move").with(position));
ctx.net.send_all("ready");
```

### `ctx.net.send_all_except(except: PeerId, packet: impl Into<Packet>)`

Sends to every connected peer except one. The usual way for a host to pass on what one
player did.

```rust
for (from, packet) in ctx.net.received("shot") {
    ctx.net.send_all_except(from, Packet::new("shot").with(from).with(packet.vec3(0)));
}
```

## Receiving

Events are collected at the start of every frame and kept until the next one, like key
presses. The helpers return owned lists, so you can send or speak inside the loop.

### `ctx.net.joined() -> Vec<PeerId>`

Players who connected this frame. On a client: `PeerId::SERVER` when the connection became
ready.

```rust
for player in ctx.net.joined() {
    ctx.speak(format!("{} joined", ctx.net.name(player)));
}
```

### `ctx.net.left() -> Vec<(PeerId, Reason)>`

Players who left this frame, and why. On a client: `PeerId::SERVER` when the connection
ended or could not be made.

```rust
for (_, reason) in ctx.net.left() {
    ctx.speak(ctx.lang.describe_reason(&reason));
}
```

### `ctx.net.received(kind: &str) -> Vec<(PeerId, Packet)>`

The packets of one kind that arrived this frame, with their sender, in order.

```rust
for (from, packet) in ctx.net.received("chat") {
    ctx.speak(format!("{}: {}", ctx.net.name(from), packet.text(0)));
}
```

### `ctx.net.messages() -> Vec<(PeerId, Packet)>`

All packets that arrived this frame.

```rust
for (from, packet) in ctx.net.messages() {
    match packet.kind() {
        "move" => {}
        "chat" => {}
        _ => {}
    }
}
```

### `ctx.net.events() -> &[NetEvent]`

Everything that happened this frame, in order.

```rust
let busy = ctx.net.events().len();
```

### `Scene::on_net(&mut self, event: &NetEvent, ctx: &mut Context)`

Called for every network event before `update`. See [engine.md](engine.md#scene).

## Players and state

### `ctx.net.players() -> Vec<PeerId>`

Connected peers in the order they joined. Host: all clients. Client: `PeerId::SERVER`
once connected.

```rust
for player in ctx.net.players() {
    ctx.net.send(player, "round_start");
}
```

### `ctx.net.player_count() -> usize`

Number of connected peers. The host does not count itself.

```rust
ctx.speak(format!("{} players online", ctx.net.player_count() + 1));
```

### `ctx.net.is_connected_to(peer: PeerId) -> bool`

Whether `peer` is connected right now.

```rust
if !ctx.net.is_connected_to(player) { /* forget them */ }
```

### `ctx.net.name(peer: PeerId) -> &str`

The name a peer chose with `set_name`; `""` if not connected. Your own id gives your own
name. A client knows the host's name (`PeerId::SERVER`) but not the other clients' names;
the host sends them in packets.

```rust
let me = ctx.net.name(ctx.net.id()).to_string();
```

### `ctx.net.ping(peer: PeerId) -> f32`

Round trip time to a peer in seconds, updated every second. 0.0 until measured or when not
connected.

```rust
let ms = (ctx.net.ping(PeerId::SERVER) * 1000.0).round();
ctx.speak(format!("Ping {ms} milliseconds"));
```

### `ctx.net.id() -> PeerId`

Your own id: `PeerId::SERVER` for the host (and while offline), the number the host gave
you on a client.

```rust
ctx.net.send_all(Packet::new("hello").with(ctx.net.id()));
```

### `ctx.net.is_host() -> bool` / `is_client() -> bool` / `is_joining() -> bool` / `is_online() -> bool`

Hosting; connected to a server; a `join` still in progress; hosting or connected.

```rust
if ctx.net.is_host() {
    // decide what happens
} else if ctx.net.is_client() {
    // send what the player wants to do
}
```

### `ctx.net.port() -> Option<u16>`

The port the server listens on while hosting, `None` otherwise.

```rust
ctx.net.host(0)?;
let port = ctx.net.port().unwrap();
```

## Packet

A kind plus a list of values.

### `Packet::new(kind: impl Into<String>) -> Packet`

A packet without values. Text converts into one too: `"ready".into()`.

```rust
let packet = Packet::new("door_opened");
```

### `packet.with(value: impl Into<Arg>) -> Packet` / `packet.push(value: impl Into<Arg>)`

Adds a value. Accepted: all integer types, `f32`, `f64`, `bool`, `&str`, `String`,
`&String`, `Vec2`, `Vec3`, `PeerId`, `Vec<u8>`, `&[u8]`.

```rust
let packet = Packet::new("hit").with(enemy_number).with(25).with(true);
let mut list = Packet::new("scores");
for score in [10, 20, 30] {
    list.push(score);
}
```

### `packet.kind() -> &str` / `packet.is(kind: &str) -> bool`

What the packet is about.

```rust
if packet.is("chat") { /* ... */ }
```

### `packet.int(index) -> i64` / `float(index) -> f32` / `bool(index) -> bool` / `text(index) -> &str` / `vec2(index) -> Vec2` / `vec3(index) -> Vec3` / `peer(index) -> PeerId` / `bytes(index) -> &[u8]`

The value at `index`, converted. Never panics: a missing or unfitting value gives `0`,
`0.0`, `false`, `""`, `Vec2::ZERO`, `Vec3::ZERO`, `PeerId::SERVER` or `&[]`. Whole and
decimal numbers convert into each other, `bool` gives 0 / 1, text is parsed as a number,
a `Vec2` becomes a `Vec3` with z = 0.

```rust
let packet = Packet::new("move").with(vec3(1.0, 2.0, 0.0)).with(90);
let position = packet.vec3(0);
let facing = packet.float(1);   // 90.0
```

### `packet.len() -> usize` / `is_empty() -> bool` / `args() -> &[Arg]` / `get(index) -> Option<&Arg>`

The raw values.

```rust
use audio_code::net::Arg;
if let Some(Arg::Text(t)) = packet.get(0) { ctx.speak(t); }
```

### `Arg`

```rust
pub enum Arg { Int(i64), Float(f64), Bool(bool), Text(String), Vec2(Vec2), Vec3(Vec3), Bytes(Vec<u8>) }
```

## PeerId

A connected player. `Copy`, comparable, usable as a map key, `Display` shows the number.

### `PeerId::SERVER`

The host, number 0. Clients receive everything from this peer.

### `peer.number() -> u32` / `PeerId::from_number(n: u32) -> PeerId` / `peer.is_server() -> bool`

Clients are numbered from 1 in the order they joined; numbers are not reused while the
server runs. In packets: `.with(peer)` and `packet.peer(i)`.

```rust
let sound_pitch = 1.0 + player.number() as f32 * 0.1;
```

## NetEvent

```rust
pub enum NetEvent {
    Connected(PeerId),             // host: a player joined; client: connection ready (SERVER)
    Message(PeerId, Packet),       // a packet arrived
    Disconnected(PeerId, Reason),  // host: a player left; client: connection ended or failed
}
```

## Reason

Why a connection ended. `Display` gives an English sentence;
`ctx.lang.describe_reason(&reason)` one in the current language.

| Variant | When |
|---------|------|
| `Left` | the player left (`close`, quitting) |
| `Closed` | the host closed the server |
| `Kicked(String)` | the host kicked the player, with its text |
| `ServerFull` | the server has `max_players` clients |
| `WrongGame(String)` | another game; contains the host's title |
| `WrongVersion(String)` | another version; contains the host's version |
| `TimedOut` | nothing heard for 10 seconds |
| `Failed(String)` | joining failed (wrong address, no server, no network, invalid TLS certificate) |
| `Lost(String)` | the connection broke, or a message failed decryption (changed on the way) |

```rust
for (_, reason) in ctx.net.left() {
    if reason == Reason::WrongVersion("2.0".into()) {
        ctx.speak("Please update the game");
    }
}
```
