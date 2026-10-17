//! Shortcuts on [`Context`] for the things almost every game does.

use glam::Vec3;

use crate::audio::Sound;
use crate::engine::Context;
use crate::input::Key;
use crate::physics::BodyId;

/// A sound that follows a physics body.
#[derive(Debug)]
pub(crate) struct Attached {
    body: BodyId,
    sound: Sound,
    last: Vec3,
}

impl Context {
    /// Speaks text, interrupting current speech. Same as `ctx.speech.speak(text)`.
    pub fn speak(&mut self, text: impl AsRef<str>) {
        self.speech.speak(text);
    }

    /// Plays a sound once. Same as `ctx.audio.play(name)`.
    pub fn play(&mut self, name: &str) -> Sound {
        self.audio.play(name)
    }

    /// Plays a sound once at a position. Same as `ctx.audio.play_at(name, position)`.
    pub fn play_at(&mut self, name: &str, position: Vec3) -> Sound {
        self.audio.play_at(name, position)
    }

    /// Whether a key went down this frame. Same as `ctx.input.pressed(key)`.
    pub fn pressed(&self, key: Key) -> bool {
        self.input.pressed(key)
    }

    /// Whether a key is held. Same as `ctx.input.held(key)`.
    pub fn held(&self, key: Key) -> bool {
        self.input.held(key)
    }

    /// Returns `true` the first time it is called with `name`, and `false` afterwards.
    /// Handy for one-time setup in closure games and for "say this only once" hints.
    ///
    /// ```no_run
    /// # use audio_code::prelude::*;
    /// # fn f(ctx: &mut Context) {
    /// if ctx.once("intro") {
    ///     ctx.speak("Welcome! Press F1 for help.");
    /// }
    /// # }
    /// ```
    pub fn once(&mut self, name: &str) -> bool {
        self.once.insert(name.to_string())
    }

    /// Makes the listener (the player's ears) follow a body every frame: its position, its
    /// [`facing`](crate::physics::Body::facing) and its [`pitch`](crate::physics::Body::pitch). After this you never call
    /// `ctx.audio.set_listener` yourself.
    pub fn follow(&mut self, body: BodyId) {
        self.follow = Some(body);
        self.sync_world();
    }

    /// Stops following a body with the listener.
    pub fn unfollow(&mut self) {
        self.follow = None;
    }

    /// The body the listener follows, if any.
    pub fn followed(&self) -> Option<BodyId> {
        self.follow
    }

    /// Plays a sound once at a body; the sound moves with the body.
    pub fn play_on(&mut self, body: BodyId, name: &str) -> Sound {
        let sound = self.audio.play_at(name, self.physics.position(body));
        self.attach(body, sound.clone());
        sound
    }

    /// Plays a looping sound at a body. It moves with the body and stops automatically when
    /// the body is removed. Perfect for enemies, engines, coins and beacons:
    ///
    /// ```no_run
    /// # use audio_code::prelude::*;
    /// # fn f(ctx: &mut Context) {
    /// let enemy = ctx.physics.add(Body::kinematic(vec3(5.0, 5.0, 0.0)).with_tag("enemy"));
    /// ctx.loop_on(enemy, "growl");
    /// # }
    /// ```
    pub fn loop_on(&mut self, body: BodyId, name: &str) -> Sound {
        let sound = self.audio.play_looped_at(name, self.physics.position(body));
        self.attach(body, sound.clone());
        sound
    }

    /// Makes any positioned sound follow a body, e.g. one made with the sound builder:
    /// `let s = ctx.audio.sound("hum").volume(0.3).looped().at(pos).play(); ctx.attach(body, s);`
    pub fn attach(&mut self, body: BodyId, sound: Sound) {
        let position = self.physics.position(body);
        sound.set_position(position);
        self.attached.push(Attached { body, sound, last: position });
    }

    /// Stops every sound attached to a body.
    pub fn stop_sounds_on(&mut self, body: BodyId) {
        self.attached.retain(|a| {
            if a.body == body {
                a.sound.stop();
                false
            } else {
                true
            }
        });
    }

    /// Moves the listener and attached sounds to their bodies. Called by the engine every frame.
    pub(crate) fn sync_world(&mut self) {
        if let Some(id) = self.follow {
            match self.physics.get(id) {
                Some(body) => {
                    let (position, facing, pitch) = (body.position, body.facing, body.pitch);
                    if position != self.audio.listener_position()
                        || facing != self.audio.listener_yaw()
                        || pitch != self.audio.listener_pitch()
                    {
                        self.audio.set_listener_3d(position, facing, pitch);
                    }
                }
                None => self.follow = None,
            }
        }
        let physics = &self.physics;
        self.attached.retain_mut(|a| {
            if !a.sound.is_playing() {
                return false;
            }
            match physics.get(a.body) {
                Some(body) => {
                    if body.position != a.last {
                        a.sound.set_position(body.position);
                        a.last = body.position;
                    }
                    true
                }
                None => {
                    a.sound.stop();
                    false
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn follow_and_attach() {
        let mut runner = Engine::new("t").headless().build_fn(|_| {});
        let ctx = &mut runner.ctx;
        let player = ctx.physics.add(Body::kinematic(vec3(1.0, 2.0, 0.0)).with_facing(90.0));
        ctx.follow(player);
        assert_eq!(ctx.audio.listener_position(), vec3(1.0, 2.0, 0.0));
        assert_eq!(ctx.audio.listener_yaw(), 90.0);
        ctx.physics.move_forward(player, 3.0);
        ctx.sync_world();
        assert!((ctx.audio.listener_position().x - 4.0).abs() < 1e-4);

        assert!(ctx.once("a"));
        assert!(!ctx.once("a"));
    }
}
