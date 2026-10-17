//! Fly a drone through a big hall in full 3D and find the birds, which sit at different
//! heights. Sounds above you are clear, sounds below a bit dull, sounds behind you muffled.
//!
//! * Up / Down: fly forward / backward in the direction you look
//! * Left / Right: turn 30 degrees
//! * Page Up / Page Down: look 30 degrees up / down
//! * E / Q: rise / sink straight up or down
//! * C: where is the next bird?  F: where am I looking?  H: how high am I?
//! * Escape: quit
//!
//! All sounds are generated with the synth. Run with `cargo run --example flight` and use
//! headphones.

use audio_code::prelude::*;

const BIRDS: [Vec3; 4] = [
    vec3(10.0, 15.0, 8.0),  // high up in the north east
    vec3(-12.0, 5.0, 1.0),  // just above the floor in the west
    vec3(0.0, -14.0, 14.0), // under the roof in the south
    vec3(14.0, -10.0, 4.0),
];

struct Flight {
    drone: BodyId,
    birds: usize,
    /// Whether the last move was blocked, so the bump plays once and not every frame.
    blocked: bool,
}

impl Flight {
    fn new(ctx: &mut Context) -> Self {
        ctx.audio.register("bird", Synth::sine(2200.0).sweep_to(3200.0).length(0.07).then(Synth::silence(0.05)).then(Synth::sine(2600.0).length(0.06)).then(Synth::silence(0.7)));
        ctx.audio.register("hum", Synth::saw(70.0).length(1.0).volume(0.08));
        ctx.audio.register("bump", Synth::sine(80.0).length(0.15).decay(25.0).volume(0.8));
        ctx.audio.register("found", Synth::triangle(660.0).sweep_to(1320.0).length(0.4).decay(6.0).volume(0.3));

        // A hall of 40 x 40 meters and 16 meters high: floor, roof and four walls.
        // A box is given by its center and its full width (x), depth (y) and height (z).
        let physics = &mut ctx.physics;
        physics.add(Body::fixed(vec3(0.0, 0.0, -0.5)).with_box(40.0, 40.0, 1.0).with_tag("floor"));
        physics.add(Body::fixed(vec3(0.0, 0.0, 16.5)).with_box(40.0, 40.0, 1.0).with_tag("roof"));
        physics.add(Body::fixed(vec3(0.0, 20.5, 8.0)).with_box(40.0, 1.0, 16.0).with_tag("north wall"));
        physics.add(Body::fixed(vec3(0.0, -20.5, 8.0)).with_box(40.0, 1.0, 16.0).with_tag("south wall"));
        physics.add(Body::fixed(vec3(20.5, 0.0, 8.0)).with_box(1.0, 40.0, 16.0).with_tag("east wall"));
        physics.add(Body::fixed(vec3(-20.5, 0.0, 8.0)).with_box(1.0, 40.0, 16.0).with_tag("west wall"));

        // The drone starts in the middle, 2 meters above the floor. No gravity: it hovers.
        let drone = ctx.physics.add(Body::kinematic(vec3(0.0, 0.0, 2.0)).with_sphere(0.5).with_tag("drone"));
        ctx.follow(drone); // the ears use the drone's position, facing and pitch
        ctx.audio.play_looped("hum");

        for spot in BIRDS {
            let bird = ctx.physics.add(Body::sensor(spot).with_sphere(1.0).with_tag("bird"));
            ctx.loop_on(bird, "bird");
        }
        Self { drone, birds: BIRDS.len(), blocked: false }
    }

    fn fly(&mut self, ctx: &mut Context, result: MoveResult) {
        if result.blocked && !self.blocked
            && let Some(&hit) = result.hits.first()
        {
            ctx.play("bump");
            ctx.speech.speak(ctx.physics.tag(hit));
        }
        self.blocked = result.blocked;
    }
}

impl Scene for Flight {
    fn start(&mut self, ctx: &mut Context) {
        ctx.speak("Find the four birds. Arrows fly and turn, page up and down look up and down, E and Q rise and sink. C locates the nearest bird.");
    }

    fn update(&mut self, ctx: &mut Context) {
        if ctx.pressed(Key::Escape) {
            ctx.quit();
        }
        if ctx.pressed(Key::Left) || ctx.pressed(Key::Right) {
            let facing = ctx.physics.turn(self.drone, if ctx.pressed(Key::Left) { -30.0 } else { 30.0 });
            ctx.speak(math::direction_name(facing));
        }
        if ctx.pressed(Key::PageUp) || ctx.pressed(Key::PageDown) {
            let pitch = ctx.physics.tilt(self.drone, if ctx.pressed(Key::PageUp) { 30.0 } else { -30.0 });
            ctx.speak(describe_pitch(pitch));
        }

        // Smooth flight: 4 meters per second while a key is held.
        let speed = 4.0 * ctx.time.delta();
        let forward = ctx.input.axis(Key::Down, Key::Up);
        if forward != 0.0 {
            let result = ctx.physics.move_forward(self.drone, forward * speed);
            self.fly(ctx, result);
        }
        let rise = ctx.input.axis(Key::Q, Key::E);
        if rise != 0.0 {
            let result = ctx.physics.move_up(self.drone, rise * speed);
            self.fly(ctx, result);
        }

        if ctx.pressed(Key::C) {
            let position = ctx.physics.position(self.drone);
            match ctx.physics.nearest_with_tag(position, "bird") {
                Some(bird) => {
                    let text = ctx.physics.describe(self.drone, bird); // e.g. "2 o'clock, 9 meters, 6 meters above"
                    ctx.speak(text);
                }
                None => ctx.speak("No birds left"),
            }
        }
        if ctx.pressed(Key::F) {
            let (facing, pitch) = (ctx.physics.facing(self.drone), ctx.physics.pitch(self.drone));
            ctx.speak(format!("Facing {}, {}", math::direction_name(facing), describe_pitch(pitch)));
        }
        if ctx.pressed(Key::H) {
            let height = ctx.physics.position(self.drone).z;
            ctx.speak(format!("{height:.0} meters high"));
        }

        if let Some(bird) = ctx.physics.entered_tag(self.drone, "bird") {
            ctx.physics.remove(bird);
            self.birds -= 1;
            ctx.play("found");
            if self.birds == 0 {
                ctx.ui.message("done", "You found every bird!");
            } else {
                ctx.speak(format!("Bird found! {} left.", self.birds));
            }
        }
        if ctx.ui.closed("done") {
            ctx.quit();
        }
    }
}

fn describe_pitch(pitch: f32) -> String {
    match pitch.round() as i32 {
        0 => "looking straight ahead".to_string(),
        90 => "looking straight up".to_string(),
        -90 => "looking straight down".to_string(),
        p if p > 0 => format!("looking {p} degrees up"),
        p => format!("looking {} degrees down", -p),
    }
}

fn main() -> audio_code::Result<()> {
    Engine::new("Flight").echo_speech(true).run_with(Flight::new)
}
