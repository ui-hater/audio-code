# Online play

Play together over the internet or the local network. One player **hosts**, the others
**join**. Everyone sends small messages called **packets**. Everything is reached through
`ctx.net`.

```rust,no_run
use audio_code::prelude::*;

fn main() -> audio_code::Result<()> {
    audio_code::run("Chat", |ctx| {
        if ctx.once("start") {
            ctx.net.set_name("Anna");
            ctx.net.join("localhost"); // the host calls ctx.net.host(net::DEFAULT_PORT) instead
        }
        for _ in ctx.net.joined() {
            ctx.speak("Connected");
        }
        if ctx.pressed(Key::Space) {
            ctx.net.send_all(Packet::new("chat").with("Hello!"));
        }
        for (from, packet) in ctx.net.received("chat") {
            ctx.speak(format!("{}: {}", ctx.net.name(from), packet.text(0)));
        }
        for (_, reason) in ctx.net.left() {
            ctx.speak(ctx.lang.describe_reason(&reason));
        }
    })
}
```

The `online` example is a complete small game: `cargo run --example online`. Start it twice
on one computer, host in one window and join `localhost` in the other.

## Hosting and joining

| What | Code |
|------|------|
| Your name (before hosting or joining) | `ctx.net.set_name("Anna")` |
| Host | `ctx.net.host(7777)?` or `ctx.net.host(net::DEFAULT_PORT)` |
| Join | `ctx.net.join("example.com")`, `ctx.net.join("192.168.1.20:7777")`, `ctx.net.join("wss://example.com/game")` |
| Leave or stop hosting | `ctx.net.close()` |
| Your address for friends in the same network | `Net::local_ip()` |

- `host` returns an error when the port is in use. Speak it: `ctx.speak(e.to_string())`.
- `join` returns at once. Some frames later you get a `Connected` event, or a
  `Disconnected` event that says why it did not work (wrong address, no server, server
  full, ...). The game keeps running while it connects.
- Without a port, `join` uses `net::DEFAULT_PORT` (7777). `ws://` and `wss://` URLs use
  the usual web ports 80 and 443 unless they name one.
- Leaving happens automatically when the game quits. The others are told you left.

### Playing over the internet

In the same network (same Wi-Fi), players join the host's local address, for example
`192.168.1.20`. `Net::local_ip()` tells the host what it is.

Over the internet, players need the host's **public** address, and the host's router must
forward the port (7777 by default) to the host's computer. For games that many people play,
run a [dedicated server](#dedicated-servers) on a rented server with a fixed address.

## WebSockets and encryption

Every connection is a **WebSocket**, the same technique web pages use for chats and browser
games. That has two advantages: networks that only let web traffic through (schools,
offices, some public Wi-Fi) usually let it through too, and a server can sit behind an
ordinary web server.

Everything is **encrypted, always, without setup**. When a connection opens, both sides make
fresh keys (X25519 key exchange), and every message after that is encrypted and protected
against changes (XChaCha20-Poly1305). Names, chat and game data cannot be read by anyone
listening in on the network, and changed, repeated or missing messages close the
connection. Because the keys are new for every connection, recorded traffic cannot be read
later either.

What the built-in encryption cannot do is prove *who* the server is. Someone who controls
the network between the players (a hacked router, for example) could pretend to be the
server. For games with a public server, rule this out with a TLS certificate:

1. Run the game server on the server machine as usual (`ctx.net.host(7777)`).
2. Put a web server with a certificate in front of it. With [Caddy](https://caddyserver.com),
   which fetches certificates by itself, the whole configuration is:

   ```text
   game.example.com {
       reverse_proxy /play localhost:7777
   }
   ```

3. Players join `wss://game.example.com/play`. The certificate is checked against the
   usual web root certificates, so nobody can pose as your server.

`wss://` needs the `tls` feature, which is on by default. Without it, plain addresses and
`ws://` work and are still encrypted as described above.

## Packets

A packet has a **kind** (a short text that says what it is about) and a list of **values**:

```rust
let packet = Packet::new("move").with(vec3(3.0, 4.0, 0.0)).with(90.0);
ctx.net.send_all(packet);
```

Values can be whole numbers, decimals, `bool`, text, `Vec2`, `Vec3`, player ids
(`PeerId`) and raw bytes. They are read back by position:

```rust
for (from, packet) in ctx.net.received("move") {
    let position = packet.vec3(0);
    let facing = packet.float(1);
}
```

The getters never crash. A missing value or one of another type gives a neutral result:
`0`, `0.0`, `false`, `""`, `Vec3::ZERO`. Numbers convert between whole and decimal.

A packet without values can be written as text: `ctx.net.send_all("ready")`.

## Sending

| What | Code |
|------|------|
| To everybody (on a client: to the host) | `ctx.net.send_all(packet)` |
| To one player | `ctx.net.send(player, packet)` |
| To everybody except one | `ctx.net.send_all_except(player, packet)` |
| A client to the host | `ctx.net.send(PeerId::SERVER, packet)` or `send_all` |

Packets arrive complete, unchanged and in the order they were sent. A packet can be up to
1 MB. Sending while not connected does nothing.

## Receiving

Network events live for one frame, like key presses:

| What happened this frame | Code |
|--------------------------|------|
| Players who joined | `for player in ctx.net.joined() { .. }` |
| Packets of one kind | `for (from, packet) in ctx.net.received("chat") { .. }` |
| All packets | `for (from, packet) in ctx.net.messages() { .. }` |
| Players who left, and why | `for (player, reason) in ctx.net.left() { .. }` |
| Everything in order | `ctx.net.events()` |

On a client, the only peer is the host, `PeerId::SERVER`: `joined()` contains it when the
connection is ready, `left()` when it ends.

Scenes can also handle events in a callback, like `on_ui`:

```rust,no_run
use audio_code::prelude::*;

struct Lobby;

impl Scene for Lobby {
    fn on_net(&mut self, event: &NetEvent, ctx: &mut Context) {
        match event {
            NetEvent::Connected(player) => ctx.speak(format!("{} joined", ctx.net.name(*player))),
            NetEvent::Message(from, packet) if packet.is("chat") => ctx.speak(packet.text(0)),
            NetEvent::Disconnected(_, reason) => ctx.speak(ctx.lang.describe_reason(reason)),
            _ => {}
        }
    }

    fn update(&mut self, _ctx: &mut Context) {}
}

fn main() -> audio_code::Result<()> {
    Engine::new("Lobby").run(Lobby)
}
```

## Players

- Every player has a `PeerId`. The host is `PeerId::SERVER` (number 0). Clients are
  numbered from 1 in the order they join. A client learns its own number with
  `ctx.net.id()`.
- `ctx.net.name(player)` is the name the player chose. Names are at most 32 characters.
- `ctx.net.players()` lists the connected peers, `ctx.net.player_count()` counts them. The
  host does not count itself.
- `ctx.net.ping(player)` is the round trip time in seconds, measured every second.
- `ctx.net.kick(player, "reason")` sends a player away (host only).
- `ctx.net.set_max_players(8)` limits how many may join (default 32). Further players are
  turned away with `Reason::ServerFull`.

Ids fit into packets: `.with(player)` writes one, `packet.peer(0)` reads it. That is how the
host tells everybody who did what.

## Who decides what

The simplest reliable design: **the host decides**. Clients only say what their player wants
to do. The host checks it, updates the game, and tells everybody what happened.

```rust
// Client: tell the host where I walked.
ctx.net.send_all(Packet::new("move").with(position));

// Host: accept it and pass it on to the others.
for (from, packet) in ctx.net.received("move") {
    let position = packet.vec3(0);
    // ... check that the move is allowed, store it ...
    ctx.net.send_all_except(from, Packet::new("pos").with(from).with(position));
}

// Client: move the other player's sound.
for (_, packet) in ctx.net.received("pos") {
    let (player, position) = (packet.peer(0), packet.vec3(1));
}
```

Because the host has the final say, players cannot cheat by changing their own game, and
everybody hears the same game. Things that happen by chance (dice, loot, spawn points) are
rolled on the host and sent to the clients.

When a player joins, the host sends the current state (who is where, the score) to the new
player with `ctx.net.send(player, ..)`.

The host is also a player in this design. Its own actions do not travel over the network;
it handles them directly and tells the clients.

## Game and version checks

Only the same game can join: the host compares the engine title (`Engine::new("My game")`).
A client of another game gets `Reason::WrongGame`.

Set a version so old and new game code never play together:

```rust
ctx.net.set_version("1.2");
```

A client with another version gets `Reason::WrongVersion` with the host's version, so the
game can tell the player to update.

## When connections end

`Reason` says why, and `ctx.lang.describe_reason(&reason)` turns it into a sentence in the
current language (English and German are built in):

| Reason | When |
|--------|------|
| `Left` | the player left (`close`, quitting the game) |
| `Closed` | the host closed the server |
| `Kicked(text)` | the host kicked the player |
| `ServerFull` | the server has the maximum number of players |
| `WrongGame(title)` / `WrongVersion(version)` | not the same game or version |
| `TimedOut` | nothing was heard for 10 seconds (network gone, game frozen) |
| `Failed(text)` | joining failed: wrong address, no server, no network |
| `Lost(text)` | the connection broke |

## Dedicated servers

A dedicated server runs without window or sound, for example on a rented machine.
`Engine::run_server` runs a scene that way. `ctx.speak` prints to the console, which makes
it a log. The storage is saved as usual.

```rust,no_run
use audio_code::prelude::*;

fn main() -> audio_code::Result<()> {
    Engine::new("My game").run_server_fn(|ctx| {
        if ctx.once("start") {
            ctx.net.set_version("1.2");
            ctx.net.host(net::DEFAULT_PORT).expect("port 7777 is free");
            ctx.speak("Server started");
        }
        for player in ctx.net.joined() {
            ctx.speak(format!("{} joined", ctx.net.name(player)));
        }
        for (from, packet) in ctx.net.messages() {
            ctx.net.send_all_except(from, packet);
        }
    })
}
```

The same program can be both: start the server when it is called with `--server`, the
normal game otherwise.

## Limits

- WebSockets run over TCP: reliable and in order. A lost piece of data is sent again, which
  can delay later packets a little on a bad connection. For audio games, which send a few
  updates per second, this is rarely noticeable.
- Send changes (a step, a shot), not the full state every frame. Ten position updates per
  second are plenty for walking.
- Without `wss://`, the encryption protects against listening in, not against someone
  posing as the server (see [WebSockets and encryption](#websockets-and-encryption)).
- The host cannot serve `wss://` itself; a web server in front of it does that.
- The host's own address is not hidden from players who join.
