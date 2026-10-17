//! Drive a car or a truck through a small town and reach the garage.
//!
//! Up = gas, Down = brake and reverse, Left / Right = steer, Space = handbrake, H = horn,
//! E = engine on / off, Tab = switch between car and truck, L = load / unload the truck,
//! F = speed, gear and fuel, G = where is the garage. Escape quits.
//!
//! All sounds are generated, no files needed.

use audio_code::prelude::*;

const MAP: &str = "
    ####################
    #P.......#........G#
    #........#.........#
    #...######...####..#
    #..................#
    #..T...........#...#
    ####################
";

struct Town {
    car: BodyId,
    truck: BodyId,
    driving: BodyId,
    garage: BodyId,
}

impl Town {
    fn new(ctx: &mut Context) -> Self {
        // Engine: a low buzz that loops without clicks (whole cycles, no fades).
        let engine = Synth::saw(55.0).length(1.0).attack(0.0).release(0.0).volume(0.25)
            .with(Synth::square(110.0).length(1.0).attack(0.0).release(0.0).volume(0.08));
        ctx.audio.register("engine", engine);
        ctx.audio.register("horn", Synth::square(415.0).length(0.5).volume(0.25).with(Synth::square(523.0).length(0.5).volume(0.2)));
        ctx.audio.register("shift", Synth::noise().length(0.04).decay(90.0).volume(0.3));
        ctx.audio.register("crash", Synth::noise().length(0.4).decay(10.0).volume(0.8).with(Synth::sine(80.0).sweep_to(30.0).length(0.3)));
        ctx.audio.register("start", Synth::saw(40.0).sweep_to(55.0).length(0.6).volume(0.3));
        ctx.audio.register("garage", Synth::sine(880.0).length(0.15).decay(10.0));

        let level = ctx.physics.build_level(MAP, 6.0);
        let sounds = |v: Vehicle| {
            v.with_engine_sound("engine")
                .with_horn_sound("horn")
                .with_shift_sound("shift")
                .with_crash_sound("crash")
                .with_start_sound("start")
        };
        let car_start = level.point('P').unwrap_or(Vec3::ZERO);
        let car = ctx.physics.add(Body::car(car_start).with_facing(90.0).with_vehicle(sounds(Vehicle::car().with_fuel(5.0, 8.0))));
        let truck_start = level.point('T').unwrap_or(Vec3::ZERO);
        let truck = ctx.physics.add(
            Body::truck(truck_start).with_facing(90.0).with_vehicle(sounds(Vehicle::truck()).engine_off()),
        );
        let garage_at = level.point('G').unwrap_or(Vec3::ZERO);
        let garage = ctx.physics.add(Body::sensor(garage_at).with_box(6.0, 6.0, 3.0).with_tag("garage"));
        ctx.loop_on(garage, "garage");
        ctx.follow(car);
        Self { car, truck, driving: car, garage }
    }

    fn name(&self) -> &'static str {
        if self.driving == self.car { "car" } else { "truck" }
    }
}

impl Scene for Town {
    fn start(&mut self, ctx: &mut Context) {
        ctx.speak("You sit in the car. Up drives, down brakes. Drive to the garage in the north east. F1 for help.");
    }

    fn update(&mut self, ctx: &mut Context) {
        let id = self.driving;
        ctx.drive(id);

        if ctx.pressed(Key::H) {
            ctx.horn(id);
        }
        if ctx.pressed(Key::E) {
            let running = ctx.physics.vehicle(id).is_some_and(|v| v.is_engine_on());
            if running {
                ctx.stop_engine(id);
                ctx.speak("Engine off");
            } else if !ctx.start_engine(id) {
                ctx.speak("The tank is empty");
            }
        }
        if ctx.pressed(Key::Tab) {
            if let Some(v) = ctx.physics.vehicle_mut(id) {
                v.throttle = 0.0;
                v.brake = 0.0;
                v.handbrake = true;
            }
            self.driving = if id == self.car { self.truck } else { self.car };
            ctx.follow(self.driving);
            let text = format!("You switch to the {}. E starts the engine.", self.name());
            ctx.speak(text);
        }
        if ctx.pressed(Key::L) && self.driving == self.truck {
            let speed = ctx.physics.vehicle(self.truck).map_or(0.0, |v| v.speed().abs());
            if speed > 0.5 {
                ctx.speak("Stop first");
            } else if let Some(v) = ctx.physics.vehicle_mut(self.truck) {
                let load = if v.load > 0.0 { 0.0 } else { v.max_load };
                v.set_load(load);
                let text = format!("Load {} tons, total weight {} tons", load / 1000.0, ctx.physics.weight(self.truck) / 1000.0);
                ctx.speak(text);
            }
        }
        if ctx.pressed(Key::F)
            && let Some(v) = ctx.physics.vehicle(id)
        {
            let mut text = format!("{}, gear {}", v.describe_speed(), v.gear_name());
            if let Some(fuel) = v.fuel() {
                text.push_str(&format!(", {fuel:.1} liters left"));
            }
            ctx.speak(text);
        }
        if ctx.pressed(Key::G) {
            let text = ctx.physics.describe(id, self.garage);
            ctx.speak(text);
        }
        if ctx.pressed(Key::F1) {
            ctx.speak("Up gas, down brake and reverse, left and right steer, space handbrake, H horn, E engine, Tab switch vehicle, L load the truck, F speed, G garage.");
        }

        if let Some(gear) = ctx.physics.gear_changed(id)
            && gear == -1
        {
            ctx.speak("Reverse");
        }
        if let Some((_, speed)) = ctx.physics.crashed(id)
            && speed > 5.0
        {
            let text = format!("Crash at {:.0} kilometers per hour", speed * 3.6);
            ctx.speak(text);
        }
        if ctx.physics.entered_tag(id, "garage").is_some() {
            let text = format!("You parked the {} in the garage. Well done!", self.name());
            ctx.speak(text);
            ctx.quit();
        }
        if ctx.pressed(Key::Escape) {
            ctx.quit();
        }
    }
}

fn main() -> audio_code::Result<()> {
    Engine::new("Driving").run_with(Town::new)
}
