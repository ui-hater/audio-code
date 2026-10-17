//! Shortcuts on [`Context`] for the things almost every game does.

use glam::Vec3;

use crate::audio::Sound;
use crate::engine::Context;
use crate::input::Key;
use crate::physics::{BodyId, PhysicsEvent};

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

    /// Drives a vehicle with the keyboard: Up or W is the gas pedal, Down or S brakes and,
    /// once the vehicle stands still, reverses, Left / Right or A / D steer, Space is the
    /// handbrake. Call it every frame. Does nothing for bodies without a vehicle. While a
    /// menu is open all pedals are released.
    ///
    /// ```no_run
    /// # use audio_code::prelude::*;
    /// # fn f(ctx: &mut Context) {
    /// let car = ctx.physics.add(Body::car(Vec3::ZERO));
    /// ctx.follow(car);
    /// // in update():
    /// ctx.drive(car);
    /// # }
    /// ```
    pub fn drive(&mut self, vehicle: BodyId) {
        let input = &self.input;
        let throttle = (input.held(Key::Up) || input.held(Key::W)) as u8 as f32;
        let brake = (input.held(Key::Down) || input.held(Key::S)) as u8 as f32;
        let right = (input.held(Key::Right) || input.held(Key::D)) as i32;
        let left = (input.held(Key::Left) || input.held(Key::A)) as i32;
        let handbrake = input.held(Key::Space);
        if let Some(v) = self.physics.vehicle_mut(vehicle) {
            v.throttle = throttle;
            v.brake = brake;
            v.steer = (right - left) as f32;
            v.handbrake = handbrake;
        }
    }

    /// Honks: plays the vehicle's horn sound at the vehicle. Returns a silent handle if it
    /// has none.
    pub fn horn(&mut self, vehicle: BodyId) -> Sound {
        match self.physics.vehicle(vehicle).and_then(|v| v.horn_sound.clone()) {
            Some(name) => self.play_on(vehicle, &name),
            None => Sound::none(),
        }
    }

    /// Starts the engine of a vehicle and plays its start sound. Returns `false` if the
    /// body is no vehicle, the engine already runs or the tank is empty.
    pub fn start_engine(&mut self, vehicle: BodyId) -> bool {
        let Some(v) = self.physics.vehicle_mut(vehicle) else {
            return false;
        };
        if v.is_engine_on() || !v.start_engine() {
            return false;
        }
        if let Some(name) = v.start_sound.clone() {
            self.play_on(vehicle, &name);
        }
        true
    }

    /// Stops the engine of a vehicle. Its engine sound fades out and it rolls out.
    pub fn stop_engine(&mut self, vehicle: BodyId) {
        if let Some(v) = self.physics.vehicle_mut(vehicle) {
            v.stop_engine();
        }
    }

    /// Keeps the engine loops of vehicles playing, and plays shift and crash sounds.
    /// Called by the engine once per frame.
    pub(crate) fn sync_vehicles(&mut self) {
        let (physics, audio) = (&self.physics, &mut self.audio);
        // Engine loops: start while the engine runs, follow rpm and throttle, stop otherwise.
        self.engine_sounds.retain(|(id, sound)| {
            let running = physics.vehicle(*id).is_some_and(|v| v.is_engine_on() && v.engine_sound.is_some());
            if !running || !sound.is_playing() {
                sound.fade_out(0.3);
            }
            running && sound.is_playing()
        });
        for (id, body) in physics.bodies() {
            let Some(v) = &body.vehicle else {
                continue;
            };
            let Some(name) = &v.engine_sound else {
                continue;
            };
            if !v.is_engine_on() {
                continue;
            }
            let (pitch, volume) = (v.engine_pitch(), 0.6 + 0.4 * v.throttle.clamp(0.0, 1.0));
            match self.engine_sounds.iter().find(|(e, _)| *e == id) {
                Some((_, sound)) => {
                    sound.set_position(body.position);
                    sound.set_pitch(pitch);
                    sound.set_volume(volume);
                }
                None => {
                    let sound = audio.sound(name).looped().at(body.position).pitch(pitch).volume(volume).play();
                    self.engine_sounds.push((id, sound));
                }
            }
        }
        for event in physics.events() {
            let (vehicle, name, volume) = match *event {
                PhysicsEvent::Crashed { vehicle, speed, .. } => {
                    (vehicle, physics.vehicle(vehicle).and_then(|v| v.crash_sound.as_deref()), (speed / 15.0).min(1.0))
                }
                PhysicsEvent::GearChanged { vehicle, gear } if gear != 0 => {
                    (vehicle, physics.vehicle(vehicle).and_then(|v| v.shift_sound.as_deref()), 1.0)
                }
                _ => continue,
            };
            if let Some(name) = name {
                audio.sound(name).at(physics.position(vehicle)).volume(volume).play();
            }
        }
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
