//! Walk through a walled room with 3D sound and find the beacon.
//!
//! * Up / Down: walk forward / backward
//! * Left / Right: turn 45 degrees
//! * C: where is the beacon?  F: which way am I facing?
//! * Escape: pause menu
//!
//! All sounds are generated in code, so no files are needed.
//! Run with `cargo run --example beacon` and use headphones.

use audio_code::prelude::*;

const STEP: f32 = 1.0;

struct Level {
    player: BodyId,
    beacon: BodyId,
    yaw: f32,
    beacon_sound: Sound,
    ping: Interval,
}

fn make_sounds(audio: &mut Audio) {
    // Short noise burst for footsteps.
    audio.register("step", SoundData::generate(0.08, |t| (random::range(-1.0, 1.0)) * (-t * 60.0).exp() * 0.5));
    // Dull thump for walls.
    audio.register("wall", SoundData::generate(0.15, |t| (t * 90.0 * std::f32::consts::TAU).sin() * (-t * 25.0).exp()));
    // Bright double beep for the beacon.
    audio.register(
        "beacon",
        SoundData::generate(0.25, |t| {
            let on = t < 0.08 || (0.12..0.2).contains(&t);
            if on { (t * 1320.0 * std::f32::consts::TAU).sin() * 0.4 } else { 0.0 }
        }),
    );
    audio.register("win", SoundData::generate(0.6, |t| (t * (440.0 + t * 800.0) * std::f32::consts::TAU).sin() * 0.3));
}

impl Level {
    fn new(ctx: &mut Context) -> Self {
        make_sounds(&mut ctx.audio);

        // A 20 x 20 room made of four walls, with one wall in the middle.
        let wall = |x: f32, y: f32, w: f32, d: f32| Body::fixed(vec3(x, y, 0.0)).with_box(w, d, 3.0).with_tag("wall");
        ctx.physics.add(wall(0.0, 10.5, 22.0, 1.0));
        ctx.physics.add(wall(0.0, -10.5, 22.0, 1.0));
        ctx.physics.add(wall(10.5, 0.0, 1.0, 22.0));
        ctx.physics.add(wall(-10.5, 0.0, 1.0, 22.0));
        ctx.physics.add(wall(0.0, 2.0, 12.0, 1.0));

        let player = ctx.physics.add(Body::kinematic(vec3(0.0, -8.0, 0.0)).with_sphere(0.4).with_tag("player"));
        let spot = vec3(random::range(-8.0, 8.0), random::range(4.0, 8.0), 0.0);
        let beacon = ctx.physics.add(Body::sensor(spot).with_sphere(1.0).with_tag("beacon"));

        Self { player, beacon, yaw: 0.0, beacon_sound: Sound::none(), ping: Interval::new(1.0) }
    }
}

impl Scene for Level {
    fn start(&mut self, ctx: &mut Context) {
        ctx.speech.speak("Find the beacon. Up and down to walk, left and right to turn. C locates the beacon.");
    }

    fn update(&mut self, ctx: &mut Context) {
        let level = self;
        if ctx.input.pressed(Key::Escape) {
            ctx.push_scene(Pause);
            return;
        }
        if ctx.input.pressed(Key::Left) {
            level.yaw = math::normalize_angle(level.yaw - 45.0);
            ctx.speech.speak(math::direction_name(level.yaw));
        }
        if ctx.input.pressed(Key::Right) {
            level.yaw = math::normalize_angle(level.yaw + 45.0);
            ctx.speech.speak(math::direction_name(level.yaw));
        }

        let forward = ctx.input.repeating(Key::Up, 0.35, 0.35);
        let backward = ctx.input.repeating(Key::Down, 0.35, 0.35);
        if forward || backward {
            let dir = if forward { 1.0 } else { -1.0 };
            let delta = math::forward(level.yaw) * STEP * dir;
            let result = ctx.physics.move_by(level.player, delta);
            if result.blocked {
                ctx.audio.sound("wall").volume(0.8).play();
            } else {
                ctx.audio.sound("step").random_pitch(0.85, 1.15).play();
            }
        }

        if ctx.input.pressed(Key::C) {
            let text = math::describe_position(ctx.physics.position(level.player), level.yaw, ctx.physics.position(level.beacon));
            ctx.speech.speak(text);
        }
        if ctx.input.pressed(Key::F) {
            ctx.speech.speak(format!("Facing {}", math::direction_name(level.yaw)));
        }

        // The ears follow the player.
        ctx.audio.set_listener(ctx.physics.position(level.player), level.yaw);

        // Ping the beacon once per second at its position.
        if level.ping.tick() {
            level.beacon_sound = ctx.audio.sound("beacon").at(ctx.physics.position(level.beacon)).distance(1.0, 40.0).play();
        }

        let found = ctx.physics.events().iter().any(|e| matches!(e, PhysicsEvent::Entered { sensor, .. } if *sensor == level.beacon));
        if found {
            level.beacon_sound.stop();
            ctx.audio.play("win");
            ctx.speech.speak("You found the beacon! A new one appears.");
            ctx.physics.set_position(level.beacon, vec3(random::range(-8.0, 8.0), random::range(-8.0, 8.0), 0.0));
        }
    }
}

struct Pause;

impl Scene for Pause {
    fn start(&mut self, ctx: &mut Context) {
        ctx.time.pause();
        ctx.ui.open(Menu::new("pause", "Paused").button("resume", "Resume").button("quit", "Quit game"));
    }

    fn update(&mut self, ctx: &mut Context) {
        if ctx.ui.clicked("pause", "resume") || ctx.ui.closed("pause") {
            ctx.ui.close_id("pause");
            ctx.pop_scene();
        }
        if ctx.ui.clicked("pause", "quit") {
            ctx.quit();
        }
    }

    fn end(&mut self, ctx: &mut Context) {
        ctx.time.resume();
    }
}

fn main() -> audio_code::Result<()> {
    Engine::new("Beacon").echo_speech(true).run_with(Level::new)
}
