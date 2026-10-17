//! A tiny side scroller using gravity: run with Left/Right, jump with Space, collect coins.
//! Uses x for left/right and z for up/down. All sounds are generated in code.
//!
//! Run with `cargo run --example platformer`.

use std::collections::HashMap;
use std::f32::consts::TAU;

use audio_code::prelude::*;

struct Platformer {
    player: BodyId,
    coins: HashMap<BodyId, Sound>,
    run_sound: Sound,
    score: u32,
    won: bool,
}

impl Platformer {
    fn new(ctx: &mut Context) -> Self {
        let audio = &mut ctx.audio;
        audio.register("jump", SoundData::generate(0.2, |t| (t * (300.0 + t * 2000.0) * TAU).sin() * 0.3));
        audio.register("land", SoundData::generate(0.1, |t| random::range(-1.0, 1.0) * (-t * 40.0).exp() * 0.6));
        audio.register(
            "coin",
            SoundData::generate(0.25, |t| (t * if t < 0.08 { 988.0 } else { 1319.0 } * TAU).sin() * 0.3),
        );
        audio.register("run", SoundData::generate(0.3, |t| if (t * 6.6).fract() < 0.2 { random::range(-0.3, 0.3) } else { 0.0 }));

        let physics = &mut ctx.physics;
        physics.set_gravity(vec3(0.0, 0.0, -20.0));
        physics.add(Body::fixed(vec3(0.0, 0.0, -0.5)).with_box(200.0, 4.0, 1.0).with_tag("ground"));
        for (x, z) in [(8.0, 2.0), (14.0, 4.0), (20.0, 2.5)] {
            physics.add(Body::fixed(vec3(x, 0.0, z)).with_box(4.0, 4.0, 0.5).with_tag("platform"));
        }
        let player = physics.add(Body::dynamic(vec3(0.0, 0.0, 1.0)).with_box(0.8, 0.8, 1.6).with_tag("player"));

        // Coins are sensors with a looping sound, so you can hear where they are.
        let mut coins = HashMap::new();
        for (x, z) in [(5.0, 1.0), (14.0, 5.5), (25.0, 1.0), (40.0, 1.0)] {
            let pos = vec3(x, 0.0, z);
            let id = ctx.physics.add(Body::sensor(pos).with_sphere(0.6).with_tag("coin"));
            let sound = ctx.audio.sound("coin").looped().at(pos).volume(0.3).distance(1.0, 25.0).play();
            coins.insert(id, sound);
        }

        let run_sound = ctx.audio.sound("run").looped().volume(0.0).play();
        Self { player, coins, run_sound, score: 0, won: false }
    }
}

impl Scene for Platformer {
    fn start(&mut self, ctx: &mut Context) {
        ctx.speech.speak("Collect the coins. Left and right to run, space to jump, S for score, escape to quit.");
    }

    fn update(&mut self, ctx: &mut Context) {
        if ctx.input.pressed(Key::Escape) {
            ctx.quit();
        }

        let mut velocity = ctx.physics.velocity(self.player);
        let run = ctx.input.held(Key::Right) as i32 - ctx.input.held(Key::Left) as i32;
        velocity.x = run as f32 * 6.0;
        if ctx.input.pressed(Key::Space) && ctx.physics.on_ground(self.player) {
            velocity.z = 10.0;
            ctx.audio.play("jump");
        }
        ctx.physics.set_velocity(self.player, velocity);

        let running = ctx.physics.on_ground(self.player) && run != 0;
        self.run_sound.set_volume(if running { 0.6 } else { 0.0 });

        // Side view: the player faces "north", so x maps to left and right in the ears.
        let pos = ctx.physics.position(self.player);
        ctx.audio.set_listener(vec3(pos.x, 0.0, pos.z), 0.0);

        for event in ctx.physics.events().to_vec() {
            match event {
                PhysicsEvent::CollisionStarted { a, speed, .. } if a == self.player && speed > 3.0 => {
                    ctx.audio.sound("land").volume((speed / 15.0).min(1.0)).play();
                }
                PhysicsEvent::Entered { sensor, body } if body == self.player => {
                    if let Some(sound) = self.coins.remove(&sensor) {
                        sound.stop();
                        ctx.physics.remove(sensor);
                        self.score += 1;
                        ctx.audio.sound("coin").pitch(1.5).play();
                        ctx.speech.speak(format!("{} of 4 coins", self.score));
                    }
                }
                _ => {}
            }
        }
        if self.coins.is_empty() && !self.won {
            self.won = true;
            ctx.ui.message("won", "You collected every coin! Press enter to quit.");
        }
        if ctx.ui.closed("won") {
            ctx.quit();
        }

        if ctx.input.pressed(Key::S) {
            ctx.speech.speak(format!("Score {}, position {:.0}", self.score, pos.x));
        }
    }
}

fn main() -> audio_code::Result<()> {
    Engine::new("Platformer").echo_speech(true).run_with(Platformer::new)
}
