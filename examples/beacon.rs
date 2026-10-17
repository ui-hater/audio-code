//! Walk through a maze with 3D sound and find the beacons. Your best time is saved.
//!
//! * Up / Down: walk forward / backward
//! * Left / Right: turn 45 degrees
//! * C: where is the next beacon?  F: which way am I facing?
//! * Escape: pause menu
//!
//! All sounds are generated with the synth, so no files are needed.
//! Run with `cargo run --example beacon` and use headphones.

use audio_code::prelude::*;

const MAP: &str = "
    ###########
    #B....#..B#
    #.###.#.#.#
    #...#...#.#
    ###.#####.#
    #B........#
    #.#######.#
    #....P....#
    ###########
";

struct Maze {
    player: BodyId,
    beacons: Vec<BodyId>,
    clock: Timer,
}

impl Maze {
    fn new(ctx: &mut Context) -> Self {
        ctx.audio.register("step", Synth::noise().length(0.08).decay(60.0));
        ctx.audio.register("wall", Synth::sine(90.0).length(0.15).decay(25.0).volume(0.8));
        ctx.audio.register("beacon", Synth::sine(1320.0).length(0.08).then(Synth::silence(0.04)).then(Synth::sine(1320.0).length(0.08)).then(Synth::silence(0.8)));
        ctx.audio.register("found", Synth::sine(440.0).sweep_to(1200.0).length(0.6).decay(4.0).volume(0.3));

        // Every character is a 2 meter tile: walls, the player start and three beacons.
        let level = ctx.physics.build_level(MAP, 2.0);
        let player = ctx.physics.add(Body::kinematic(level.point('P').unwrap()).with_sphere(0.4).with_tag("player"));
        ctx.follow(player); // the ears go wherever the player goes and face where it faces

        let mut beacons = Vec::new();
        for spot in level.points('B') {
            let beacon = ctx.physics.add(Body::sensor(spot).with_sphere(0.8).with_tag("beacon"));
            ctx.loop_on(beacon, "beacon"); // stops by itself when the beacon is removed
            beacons.push(beacon);
        }
        Self { player, beacons, clock: Timer::new() }
    }
}

impl Scene for Maze {
    fn start(&mut self, ctx: &mut Context) {
        ctx.speak("Find all three beacons. Up and down to walk, left and right to turn. C locates the nearest beacon.");
    }

    fn update(&mut self, ctx: &mut Context) {
        if ctx.pressed(Key::Escape) {
            ctx.push_scene(Pause);
        }
        if ctx.pressed(Key::Left) || ctx.pressed(Key::Right) {
            let degrees = if ctx.pressed(Key::Left) { -45.0 } else { 45.0 };
            let facing = ctx.physics.turn(self.player, degrees);
            ctx.speak(math::direction_name(facing));
        }

        let forward = ctx.input.repeating(Key::Up, 0.35, 0.35);
        let backward = ctx.input.repeating(Key::Down, 0.35, 0.35);
        if forward || backward {
            let step = if forward { 1.0 } else { -1.0 };
            if ctx.physics.move_forward(self.player, step).blocked {
                ctx.play("wall");
            } else {
                ctx.audio.sound("step").random_pitch(0.85, 1.15).play();
            }
        }

        if ctx.pressed(Key::C) {
            let position = ctx.physics.position(self.player);
            match ctx.physics.nearest_with_tag(position, "beacon") {
                Some(beacon) => {
                    let text = ctx.physics.describe(self.player, beacon);
                    ctx.speak(text);
                }
                None => ctx.speak("No beacons left"),
            }
        }
        if ctx.pressed(Key::F) {
            let facing = ctx.physics.facing(self.player);
            ctx.speak(format!("Facing {}", math::direction_name(facing)));
        }

        if let Some(beacon) = ctx.physics.entered_tag(self.player, "beacon") {
            ctx.physics.remove(beacon);
            self.beacons.retain(|b| *b != beacon);
            ctx.play("found");
            if self.beacons.is_empty() {
                self.finish(ctx);
            } else {
                ctx.speak(format!("Beacon found! {} left.", self.beacons.len()));
            }
        }
        if ctx.ui.closed("done") {
            ctx.quit();
        }
    }

    // The clock should not run while the pause menu is open.
    fn pause(&mut self, _ctx: &mut Context) {
        self.clock.pause();
    }

    fn resume(&mut self, _ctx: &mut Context) {
        self.clock.resume();
    }
}

impl Maze {
    fn finish(&mut self, ctx: &mut Context) {
        let seconds = self.clock.elapsed_secs().round() as u32;
        let best: u32 = ctx.storage.get_or("best_time", u32::MAX);
        let mut text = format!("You found every beacon in {seconds} seconds.");
        if seconds < best {
            ctx.storage.set("best_time", seconds);
            text.push_str(" That is a new record!");
        } else {
            text.push_str(&format!(" Your record is {best} seconds."));
        }
        ctx.ui.message("done", text);
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
    Engine::new("Beacon").echo_speech(true).run_with(Maze::new)
}
