//! A tiny side scroller using gravity: run with Left/Right, jump with Space, collect coins.
//! Uses x for left/right and z for up/down. A lift carries you to the last coin, and a
//! crate can be pushed around. All sounds are generated with the synth.
//!
//! Run with `cargo run --example platformer`.

use std::collections::HashMap;

use audio_code::prelude::*;

struct Platformer {
    player: BodyId,
    lift: BodyId,
    coins: HashMap<BodyId, Sound>,
    run_sound: Sound,
    score: u32,
    won: bool,
}

impl Platformer {
    fn new(ctx: &mut Context) -> Self {
        let audio = &mut ctx.audio;
        audio.register("jump", Synth::square(300.0).sweep_to(700.0).length(0.15).decay(10.0).volume(0.2));
        audio.register("land", Synth::noise().length(0.1).decay(40.0).with(Synth::sine(90.0).length(0.1).decay(30.0)));
        audio.register("coin", Synth::triangle(988.0).length(0.08).then(Synth::triangle(1319.0).length(0.17).decay(15.0)));
        audio.register("run", Synth::noise().length(0.06).decay(60.0).volume(0.3).then(Synth::silence(0.09)));
        audio.register("lift", Synth::saw(55.0).length(1.0).volume(0.1));
        audio.register("push", Synth::noise().length(0.3).volume(0.15));

        let physics = &mut ctx.physics;
        physics.add(Body::fixed(vec3(0.0, 0.0, -0.5)).with_box(200.0, 4.0, 1.0).with_tag("ground"));
        for (x, z) in [(8.0, 2.0), (14.0, 4.0), (20.0, 2.5)] {
            physics.add(Body::fixed(vec3(x, 0.0, z)).with_box(4.0, 4.0, 0.5).with_tag("platform"));
        }
        // A lift going up and down at x = 40, and a heavy crate at x = 30.
        let lift = physics.add(Body::kinematic(vec3(40.0, 0.0, 0.0)).with_box(3.0, 4.0, 0.5).with_tag("lift"));
        physics.add(Body::dynamic(vec3(30.0, 0.0, 0.5)).with_box(1.0, 1.0, 1.0).with_mass(5.0).with_tag("crate"));
        let player = physics
            .add(Body::dynamic(vec3(0.0, 0.0, 1.0)).with_box(0.8, 0.8, 1.6).with_max_speed(25.0).with_tag("player"));
        ctx.audio.sound("lift").looped().at(vec3(40.0, 0.0, 0.0)).distance(1.0, 20.0).play();

        // Coins are sensors with a looping sound, so you can hear where they are.
        let mut coins = HashMap::new();
        for (x, z) in [(5.0, 1.0), (14.0, 5.5), (25.0, 1.0), (40.0, 8.0)] {
            let pos = vec3(x, 0.0, z);
            let id = ctx.physics.add(Body::sensor(pos).with_sphere(0.6).with_tag("coin"));
            let sound = ctx.audio.sound("coin").looped().at(pos).volume(0.3).distance(1.0, 25.0).play();
            coins.insert(id, sound);
        }

        let run_sound = ctx.audio.sound("run").looped().volume(0.0).play();
        Self { player, lift, coins, run_sound, score: 0, won: false }
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

        // The lift goes up for 4 seconds and down for 4 seconds.
        let lift_up = (ctx.time.elapsed() % 8.0) < 4.0;
        ctx.physics.set_velocity(self.lift, vec3(0.0, 0.0, if lift_up { 2.0 } else { -2.0 }));

        let mut velocity = ctx.physics.velocity(self.player);
        let run = ctx.input.held(Key::Right) as i32 - ctx.input.held(Key::Left) as i32;
        velocity.x = run as f32 * 6.0;
        ctx.physics.set_velocity(self.player, velocity);
        if ctx.input.pressed(Key::Space) && ctx.physics.jump(self.player, 2.5) {
            ctx.audio.play("jump");
        }

        let running = ctx.physics.on_ground(self.player) && run != 0;
        self.run_sound.set_volume(if running { 0.6 } else { 0.0 });

        // Side view: the player faces "north", so x maps to left and right in the ears.
        let pos = ctx.physics.position(self.player);
        ctx.audio.set_listener(vec3(pos.x, 0.0, pos.z), 0.0);

        for event in ctx.physics.events().to_vec() {
            match event {
                PhysicsEvent::Landed { body, speed, .. } if body == self.player => {
                    ctx.audio.sound("land").volume((speed / 12.0).clamp(0.2, 1.0)).play();
                    if speed > 15.0 {
                        ctx.speech.speak("Ouch!");
                    }
                }
                PhysicsEvent::CollisionStarted { a, b, .. } if a == self.player && ctx.physics.tag(b) == "crate" => {
                    ctx.audio.play_at("push", ctx.physics.position(b));
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
    Engine::new("Platformer").gravity(EARTH_GRAVITY * 2.0).echo_speech(true).run_with(Platformer::new)
}
