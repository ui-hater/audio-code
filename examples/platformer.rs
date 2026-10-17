//! A tiny side scroller using gravity: run with Left/Right, jump with Space, collect coins.
//! Uses x for left/right and z for up/down. A lift carries you to the last coin, and a
//! crate can be pushed around. All sounds are generated with the synth.
//!
//! Run with `cargo run --example platformer`.

use audio_code::prelude::*;

const COINS: usize = 4;

struct Platformer {
    player: BodyId,
    lift: BodyId,
    run_sound: Sound,
    score: usize,
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
        // Facing north, so x maps to left and right in the ears.
        let player = physics
            .add(Body::dynamic(vec3(0.0, 0.0, 1.0)).with_box(0.8, 0.8, 1.6).with_max_speed(25.0).with_tag("player"));

        ctx.follow(player);
        ctx.loop_on(lift, "lift");
        // Coins are sensors with a looping sound, so you can hear where they are.
        for (x, z) in [(5.0, 1.0), (14.0, 5.5), (25.0, 1.0), (40.0, 8.0)] {
            let coin = ctx.physics.add(Body::sensor(vec3(x, 0.0, z)).with_sphere(0.6).with_tag("coin"));
            ctx.loop_on(coin, "coin").set_volume(0.3);
        }

        let run_sound = ctx.audio.sound("run").looped().volume(0.0).play();
        Self { player, lift, run_sound, score: 0 }
    }
}

impl Scene for Platformer {
    fn start(&mut self, ctx: &mut Context) {
        ctx.speak("Collect the coins. Left and right to run, space to jump, S for score, escape to quit.");
    }

    fn update(&mut self, ctx: &mut Context) {
        if ctx.pressed(Key::Escape) {
            ctx.quit();
        }

        // The lift goes up for 4 seconds and down for 4 seconds.
        let lift_up = (ctx.time.elapsed() % 8.0) < 4.0;
        ctx.physics.set_velocity(self.lift, vec3(0.0, 0.0, if lift_up { 2.0 } else { -2.0 }));

        let run = ctx.input.axis(Key::Left, Key::Right);
        let mut velocity = ctx.physics.velocity(self.player);
        velocity.x = run * 6.0;
        ctx.physics.set_velocity(self.player, velocity);
        if ctx.pressed(Key::Space) && ctx.physics.jump(self.player, 2.5) {
            ctx.play("jump");
        }
        let running = ctx.physics.on_ground(self.player) && run != 0.0;
        self.run_sound.set_volume(if running { 0.6 } else { 0.0 });

        if let Some(speed) = ctx.physics.landed(self.player) {
            ctx.audio.sound("land").volume((speed / 12.0).clamp(0.2, 1.0)).play();
            if speed > 15.0 {
                ctx.speak("Ouch!");
            }
        }
        if let Some((other, _)) = ctx.physics.collided(self.player)
            && ctx.physics.tag(other) == "crate"
        {
            ctx.play_on(other, "push");
        }
        if let Some(coin) = ctx.physics.entered_tag(self.player, "coin") {
            ctx.physics.remove(coin); // its looping sound stops by itself
            self.score += 1;
            ctx.audio.sound("coin").pitch(1.5).play();
            ctx.speak(format!("{} of {COINS} coins", self.score));
            if self.score == COINS {
                ctx.ui.message("won", "You collected every coin! Press enter to quit.");
            }
        }
        if ctx.ui.closed("won") {
            ctx.quit();
        }

        if ctx.pressed(Key::S) {
            let x = ctx.physics.position(self.player).x;
            ctx.speak(format!("Score {}, position {x:.0}", self.score));
        }
    }
}

fn main() -> audio_code::Result<()> {
    Engine::new("Platformer").gravity(EARTH_GRAVITY * 2.0).echo_speech(true).run_with(Platformer::new)
}
