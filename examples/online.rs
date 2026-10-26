//! A small online game: walk around a field, hear the other players and chat.
//!
//! One player picks "Host a game", the others type the host's address and pick "Join".
//! To try it alone, start the example twice on the same computer and join `localhost`.
//!
//! * Arrows: walk (1 meter per step)
//! * Enter: chat
//! * Space: where are the others?
//! * P: ping to the server
//! * Escape: leave
//!
//! The host decides: clients only tell the host where they walked, and the host passes it
//! on to everybody else. Run with `cargo run --example online` and use headphones.

use std::collections::BTreeMap;

use audio_code::prelude::*;

struct Lobby {
    name: String,
    address: String,
}

impl Lobby {
    fn new(ctx: &mut Context) -> Self {
        Self {
            name: ctx.storage.get_or("name", "Player".to_string()),
            address: ctx.storage.get_or("address", "localhost".to_string()),
        }
    }

    fn menu(&self) -> Menu {
        Menu::new("lobby", "Online demo")
            .text_field("name", "Your name", self.name.clone())
            .button("host", "Host a game")
            .text_field("address", "Server address", self.address.clone())
            .button("join", "Join")
            .button("quit", "Quit")
            .closable(false)
    }

    fn start_game(ctx: &mut Context) {
        ctx.ui.close();
        let scene = Field::new(ctx);
        ctx.switch_scene(scene);
    }
}

impl Scene for Lobby {
    fn start(&mut self, ctx: &mut Context) {
        ctx.ui.open(self.menu());
    }

    fn on_ui(&mut self, event: &UiEvent, ctx: &mut Context) {
        match event {
            UiEvent::Changed { item, value, .. } if item == "name" => {
                self.name = value.as_str().to_string();
                ctx.storage.set("name", &self.name);
            }
            UiEvent::Changed { item, value, .. } if item == "address" => {
                self.address = value.as_str().to_string();
                ctx.storage.set("address", &self.address);
            }
            UiEvent::Activated { item, .. } => match item.as_str() {
                "host" => {
                    ctx.net.set_name(&self.name);
                    match ctx.net.host(net::DEFAULT_PORT) {
                        Ok(()) => {
                            let address = Net::local_ip().map_or("unknown".to_string(), |ip| ip.to_string());
                            ctx.speak(format!("Hosting. Your address is {address}"));
                            Self::start_game(ctx);
                        }
                        Err(e) => ctx.ui.message("error", e.to_string()),
                    }
                }
                "join" => {
                    ctx.net.set_name(&self.name);
                    ctx.net.join(&self.address);
                    ctx.speak(format!("Connecting to {}", self.address));
                }
                "quit" => ctx.quit(),
                _ => {}
            },
            _ => {}
        }
    }

    fn on_net(&mut self, event: &NetEvent, ctx: &mut Context) {
        match event {
            NetEvent::Connected(_) => Self::start_game(ctx),
            NetEvent::Disconnected(_, reason) => ctx.ui.message("error", ctx.lang.describe_reason(reason)),
            NetEvent::Message(..) => {}
        }
    }

    fn update(&mut self, _ctx: &mut Context) {}
}

/// Another player: where they are and the sound that marks them.
struct Other {
    name: String,
    position: Vec3,
    sound: Sound,
}

struct Field {
    position: Vec3,
    others: BTreeMap<PeerId, Other>,
}

impl Field {
    fn new(ctx: &mut Context) -> Self {
        ctx.audio.register("other", Synth::triangle(660.0).length(0.06).decay(40.0).then(Synth::silence(0.5)));
        ctx.audio.register("step", Synth::noise().length(0.05).decay(80.0).volume(0.5));
        ctx.audio.register("chat", Synth::sine(1200.0).length(0.05).volume(0.3));
        Self { position: Vec3::ZERO, others: BTreeMap::new() }
    }

    fn add(&mut self, ctx: &mut Context, id: PeerId, name: &str, position: Vec3) {
        let sound = ctx.audio.play_looped_at("other", position);
        sound.set_pitch(1.0 + id.number() as f32 * 0.15);
        self.others.insert(id, Other { name: name.to_string(), position, sound });
    }

    fn remove(&mut self, ctx: &mut Context, id: PeerId) {
        if let Some(other) = self.others.remove(&id) {
            other.sound.stop();
            ctx.speak(format!("{} left", other.name));
        }
    }

    fn moved(&mut self, id: PeerId, position: Vec3) {
        if let Some(other) = self.others.get_mut(&id) {
            other.position = position;
            other.sound.set_position(position);
        }
    }

    fn name(&self, ctx: &Context, id: PeerId) -> String {
        if id == ctx.net.id() {
            return ctx.net.name(id).to_string();
        }
        self.others.get(&id).map_or("Someone".to_string(), |o| o.name.clone())
    }

    /// The host's side of the protocol.
    fn host_events(&mut self, event: &NetEvent, ctx: &mut Context) {
        match event {
            NetEvent::Connected(id) => {
                let name = ctx.net.name(*id).to_string();
                ctx.speak(format!("{name} joined"));
                // Tell the newcomer about everybody, and everybody about the newcomer.
                ctx.net.send(*id, Packet::new("player").with(PeerId::SERVER).with(ctx.net.name(PeerId::SERVER)).with(self.position));
                for (other, o) in &self.others {
                    ctx.net.send(*id, Packet::new("player").with(*other).with(&o.name).with(o.position));
                }
                ctx.net.send_all_except(*id, Packet::new("player").with(*id).with(&name).with(Vec3::ZERO));
                self.add(ctx, *id, &name, Vec3::ZERO);
            }
            NetEvent::Message(from, packet) => match packet.kind() {
                "move" => {
                    self.moved(*from, packet.vec3(0));
                    ctx.net.send_all_except(*from, Packet::new("pos").with(*from).with(packet.vec3(0)));
                }
                "chat" => {
                    let text = packet.text(0);
                    ctx.play("chat");
                    ctx.speak(format!("{}: {text}", self.name(ctx, *from)));
                    ctx.net.send_all_except(*from, Packet::new("chat").with(*from).with(text));
                }
                _ => {}
            },
            NetEvent::Disconnected(id, _) => {
                self.remove(ctx, *id);
                ctx.net.send_all(Packet::new("gone").with(*id));
            }
        }
    }

    /// The client's side of the protocol.
    fn client_events(&mut self, event: &NetEvent, ctx: &mut Context) {
        match event {
            NetEvent::Message(_, packet) => match packet.kind() {
                "player" => self.add(ctx, packet.peer(0), packet.text(1), packet.vec3(2)),
                "pos" => self.moved(packet.peer(0), packet.vec3(1)),
                "gone" => self.remove(ctx, packet.peer(0)),
                "chat" => {
                    ctx.play("chat");
                    ctx.speak(format!("{}: {}", self.name(ctx, packet.peer(0)), packet.text(1)));
                }
                _ => {}
            },
            NetEvent::Disconnected(_, reason) => {
                ctx.speak(ctx.lang.describe_reason(reason));
                ctx.audio.stop_all();
                let scene = Lobby::new(ctx);
                ctx.switch_scene(scene);
            }
            NetEvent::Connected(_) => {}
        }
    }
}

impl Scene for Field {
    fn start(&mut self, ctx: &mut Context) {
        ctx.speech.speak_queued("Arrows walk, Enter chats, Space finds the others, Escape leaves.");
    }

    fn on_net(&mut self, event: &NetEvent, ctx: &mut Context) {
        if ctx.net.is_host() {
            self.host_events(event, ctx);
        } else {
            self.client_events(event, ctx);
        }
    }

    fn on_ui(&mut self, event: &UiEvent, ctx: &mut Context) {
        if let UiEvent::TextSubmitted { id, text } = event
            && id == "chat"
        {
            // The host sends to everybody, a client sends to the host, who passes it on.
            if ctx.net.is_host() {
                ctx.net.send_all(Packet::new("chat").with(PeerId::SERVER).with(text));
            } else {
                ctx.net.send_all(Packet::new("chat").with(text));
            }
            ctx.play("chat");
        }
    }

    fn update(&mut self, ctx: &mut Context) {
        let step = vec3(
            ctx.input.repeating(Key::Right, 0.25, 0.25) as i32 as f32 - ctx.input.repeating(Key::Left, 0.25, 0.25) as i32 as f32,
            ctx.input.repeating(Key::Up, 0.25, 0.25) as i32 as f32 - ctx.input.repeating(Key::Down, 0.25, 0.25) as i32 as f32,
            0.0,
        );
        if step != Vec3::ZERO {
            self.position = (self.position + step).clamp(Vec3::splat(-20.0), Vec3::splat(20.0));
            ctx.play("step");
            if ctx.net.is_host() {
                ctx.net.send_all(Packet::new("pos").with(PeerId::SERVER).with(self.position));
            } else {
                ctx.net.send_all(Packet::new("move").with(self.position));
            }
        }
        ctx.audio.set_listener(self.position, 0.0);

        if ctx.pressed(Key::Enter) {
            ctx.ui.input("chat", "Message");
        }
        if ctx.pressed(Key::Space) {
            let text = if self.others.is_empty() {
                "Nobody else is here".to_string()
            } else {
                let places: Vec<String> = self
                    .others
                    .values()
                    .map(|o| format!("{}: {}", o.name, ctx.lang.describe_position(self.position, 0.0, o.position)))
                    .collect();
                places.join(". ")
            };
            ctx.speak(text);
        }
        if ctx.pressed(Key::P) {
            if ctx.net.is_host() {
                ctx.speak(format!("You are the host, {} players connected", ctx.net.player_count()));
            } else {
                let ms = (ctx.net.ping(PeerId::SERVER) * 1000.0).round();
                ctx.speak(format!("Ping {ms} milliseconds"));
            }
        }
        if ctx.pressed(Key::Escape) {
            ctx.net.close();
            ctx.audio.stop_all();
            ctx.speak("You left");
            let scene = Lobby::new(ctx);
            ctx.switch_scene(scene);
        }
    }
}

fn main() -> audio_code::Result<()> {
    Engine::new("Online demo").run_with(Lobby::new)
}
