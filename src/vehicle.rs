//! Cars and trucks: weight, speed, engine, gears, steering, brakes, fuel and sounds.
//!
//! A vehicle is an optional part of a physics [`Body`](crate::physics::Body). Add one with
//! [`Body::car`](crate::physics::Body::car), [`Body::truck`](crate::physics::Body::truck) or
//! [`Body::with_vehicle`](crate::physics::Body::with_vehicle); every other body is not
//! affected. The physics world drives it at the fixed physics rate: the pedals and the
//! steering wheel are plain fields you set every frame (or let
//! [`Context::drive`](crate::Context::drive) set them from the arrow keys).
//!
//! ```no_run
//! # use audio_code::prelude::*;
//! # fn f(ctx: &mut Context) {
//! let car = ctx.physics.add(Body::car(vec3(0.0, 0.0, 0.0)).with_vehicle(
//!     Vehicle::car().with_engine_sound("engine").with_horn_sound("horn"),
//! ));
//! ctx.follow(car);
//! // every frame:
//! ctx.drive(car);   // Up = gas, Down = brake / reverse, Left / Right = steer, Space = handbrake
//! if let Some(v) = ctx.physics.vehicle(car) {
//!     let kmh = v.speed_kmh();
//! }
//! # }
//! ```
//!
//! ## The model
//!
//! * The **weight** is the body's [`mass`](crate::physics::Body::mass) plus the vehicle's
//!   [`load`](Vehicle::load), in kilograms. The engine force and the brake force are fixed,
//!   so a heavier vehicle speeds up more slowly and needs a longer way to stop.
//! * The **speed** is a signed number along the facing of the body (negative when
//!   reversing), limited by [`top_speed`](Vehicle::top_speed) and
//!   [`reverse_speed`](Vehicle::reverse_speed). Engine force fades out towards the top speed.
//! * **Steering** turns the body's facing. The turn rate grows with speed (a parked car
//!   cannot turn) up to [`max_turn_rate`](Vehicle::max_turn_rate); the tightest circle has
//!   the radius [`turn_radius`](Vehicle::turn_radius).
//! * The vehicle moves with [`World::move_by`](crate::physics::World::move_by), so it stops
//!   at walls, slides along them, pushes loose objects and triggers sensors (checkpoints).
//!   Running into something loses the speed going into it and reports
//!   [`PhysicsEvent::Crashed`](crate::physics::PhysicsEvent::Crashed).
//! * Vehicles drive level (they keep their height) and are not pulled by gravity.
//! * An automatic gearbox picks gears from the speed; the engine speed (rpm) follows.
//!   [`Vehicle::engine_pitch`] turns it into a pitch for a looping engine sound.

use glam::Vec3;

/// Drives at more than this speed (m/s) count as moving forward or backward. Slower than
/// that, the brake pedal starts reversing.
const STOPPED: f32 = 0.3;

/// Air drag at top speed as a share of the engine force.
const AIR_DRAG: f32 = 0.3;

/// The vehicle part of a body: settings, pedals and the driving state.
///
/// Settings are public fields with `with_*` builder methods. Speeds are stored in meters per
/// second; the builders that take km/h say so in their name.
#[derive(Debug, Clone, PartialEq)]
pub struct Vehicle {
    /// Highest forward speed in m/s. The engine gets weaker towards it, so full throttle
    /// approaches it slowly.
    pub top_speed: f32,
    /// Highest reverse speed in m/s.
    pub reverse_speed: f32,
    /// Driving force of the engine at full throttle in newtons. Acceleration is this divided
    /// by the weight.
    pub engine_force: f32,
    /// Braking force at full brake pedal in newtons. Deceleration is this divided by the
    /// weight.
    pub brake_force: f32,
    /// Most acceleration or deceleration the tyres can transfer, in m/s². Caps engine,
    /// brakes and handbrake. 9 is a car on dry asphalt, 3 is a wet road, 1 is ice.
    pub grip: f32,
    /// Speed lost per second while rolling, in m/s² (tyres, air). Always active.
    pub rolling_resistance: f32,
    /// Extra speed lost per second when no pedal is pressed and the engine runs, in m/s².
    pub engine_braking: f32,
    /// Radius of the tightest circle the vehicle can drive, in meters.
    pub turn_radius: f32,
    /// Fastest the vehicle can turn, in degrees per second.
    pub max_turn_rate: f32,
    /// Number of forward gears (at least 1).
    pub gears: u32,
    /// Engine speed when idling, in revolutions per minute.
    pub idle_rpm: f32,
    /// Highest engine speed, in revolutions per minute.
    pub max_rpm: f32,
    /// Cargo in kilograms, added to the body's mass. Change it with [`Vehicle::set_load`] to
    /// respect [`Vehicle::max_load`].
    pub load: f32,
    /// Most cargo in kilograms. Default unlimited.
    pub max_load: f32,
    /// Tank size in liters, `None` (the default) for unlimited fuel.
    pub fuel_capacity: Option<f32>,
    /// Fuel use in liters per 100 km at full throttle with an empty vehicle. Light throttle
    /// uses less, cargo uses more.
    pub consumption: f32,
    /// Looping engine sound, played at the vehicle while the engine runs. Its pitch follows
    /// the engine speed and its volume the throttle.
    pub engine_sound: Option<String>,
    /// Sound for [`Context::horn`](crate::Context::horn).
    pub horn_sound: Option<String>,
    /// Sound for [`Context::start_engine`](crate::Context::start_engine).
    pub start_sound: Option<String>,
    /// Sound played on every gear change.
    pub shift_sound: Option<String>,
    /// Sound played when the vehicle crashes into something, louder for harder crashes.
    pub crash_sound: Option<String>,

    /// Gas pedal, 0.0 to 1.0. Set it every frame. While reversing it brakes.
    pub throttle: f32,
    /// Brake pedal, 0.0 to 1.0. Set it every frame. Pressed while the vehicle stands still,
    /// it drives backwards (after braking to a stop, release it and press it again).
    pub brake: f32,
    /// Steering wheel, -1.0 (full left) to 1.0 (full right).
    pub steer: f32,
    /// Handbrake: stops the vehicle as hard as the grip allows.
    pub handbrake: bool,

    speed: f32,
    gear: i32,
    rpm: f32,
    engine_on: bool,
    fuel: f32,
    distance: f32,
    /// The pedal that braked the vehicle to a stop must be released before it drives
    /// the other way: (brake pedal, gas pedal).
    hold: (bool, bool),
}

impl Default for Vehicle {
    fn default() -> Self {
        Self::car()
    }
}

/// What happened during one step, reported by the physics world as events.
#[derive(Debug, Default)]
pub(crate) struct StepReport {
    pub gear_changed: Option<i32>,
}

impl Vehicle {
    /// A car: top speed 180 km/h, 0 to 100 km/h in about 10 seconds with
    /// [`Body::car`](crate::physics::Body::car)'s 1300 kg, 5 gears, turns in a 5 m circle.
    pub fn car() -> Self {
        Self {
            top_speed: 180.0 / 3.6,
            reverse_speed: 25.0 / 3.6,
            engine_force: 4500.0,
            brake_force: 11_000.0,
            grip: 9.0,
            rolling_resistance: 0.15,
            engine_braking: 0.8,
            turn_radius: 5.0,
            max_turn_rate: 90.0,
            gears: 5,
            idle_rpm: 800.0,
            max_rpm: 6500.0,
            load: 0.0,
            max_load: f32::INFINITY,
            fuel_capacity: None,
            consumption: 12.0,
            engine_sound: None,
            horn_sound: None,
            start_sound: None,
            shift_sound: None,
            crash_sound: None,
            throttle: 0.0,
            brake: 0.0,
            steer: 0.0,
            handbrake: false,
            speed: 0.0,
            gear: 1,
            rpm: 800.0,
            engine_on: true,
            fuel: 0.0,
            distance: 0.0,
            hold: (false, false),
        }
    }

    /// A truck: top speed 90 km/h, 12 gears, turns in an 11 m circle, carries up to 25 t.
    /// With [`Body::truck`](crate::physics::Body::truck)'s 12 t it reaches 60 km/h in about
    /// 13 seconds empty and in about 50 seconds fully loaded.
    pub fn truck() -> Self {
        Self {
            top_speed: 90.0 / 3.6,
            reverse_speed: 10.0 / 3.6,
            engine_force: 20_000.0,
            brake_force: 110_000.0,
            grip: 7.0,
            rolling_resistance: 0.1,
            engine_braking: 0.5,
            turn_radius: 11.0,
            max_turn_rate: 35.0,
            gears: 12,
            idle_rpm: 600.0,
            max_rpm: 2200.0,
            max_load: 25_000.0,
            consumption: 35.0,
            gear: 1,
            rpm: 600.0,
            ..Self::car()
        }
    }

    /// Sets the top speed in km/h.
    pub fn with_top_speed_kmh(mut self, kmh: f32) -> Self {
        self.top_speed = (kmh / 3.6).max(0.1);
        self
    }

    /// Sets the top reverse speed in km/h.
    pub fn with_reverse_speed_kmh(mut self, kmh: f32) -> Self {
        self.reverse_speed = (kmh / 3.6).max(0.0);
        self
    }

    /// Sets the engine force in newtons (car 4500, truck 20000). More force, faster
    /// acceleration.
    pub fn with_engine_force(mut self, newtons: f32) -> Self {
        self.engine_force = newtons.max(0.0);
        self
    }

    /// Sets the brake force in newtons (car 11000, truck 110000).
    pub fn with_brake_force(mut self, newtons: f32) -> Self {
        self.brake_force = newtons.max(0.0);
        self
    }

    /// Sets the tyre grip in m/s² (9 dry, 3 wet, 1 ice).
    pub fn with_grip(mut self, grip: f32) -> Self {
        self.grip = grip.max(0.01);
        self
    }

    /// Sets the rolling resistance in m/s².
    pub fn with_rolling_resistance(mut self, resistance: f32) -> Self {
        self.rolling_resistance = resistance.max(0.0);
        self
    }

    /// Sets the engine braking in m/s² (how fast the vehicle slows down off the gas).
    pub fn with_engine_braking(mut self, braking: f32) -> Self {
        self.engine_braking = braking.max(0.0);
        self
    }

    /// Sets the radius of the tightest circle in meters.
    pub fn with_turn_radius(mut self, meters: f32) -> Self {
        self.turn_radius = meters.max(0.1);
        self
    }

    /// Sets the fastest turn rate in degrees per second.
    pub fn with_max_turn_rate(mut self, degrees_per_second: f32) -> Self {
        self.max_turn_rate = degrees_per_second.max(0.0);
        self
    }

    /// Sets the gearbox: number of forward gears, idle and highest engine speed in rpm.
    pub fn with_gears(mut self, gears: u32, idle_rpm: f32, max_rpm: f32) -> Self {
        self.gears = gears.max(1);
        self.idle_rpm = idle_rpm.max(0.0);
        self.max_rpm = max_rpm.max(self.idle_rpm + 1.0);
        self.rpm = self.idle_rpm;
        self
    }

    /// Sets the cargo in kilograms (limited by [`Vehicle::max_load`]).
    pub fn with_load(mut self, kg: f32) -> Self {
        self.set_load(kg);
        self
    }

    /// Sets the most cargo in kilograms.
    pub fn with_max_load(mut self, kg: f32) -> Self {
        self.max_load = kg.max(0.0);
        self.load = self.load.min(self.max_load);
        self
    }

    /// Gives the vehicle a tank of `capacity` liters, starting full, that uses
    /// `liters_per_100km` at full throttle. Without this, fuel is unlimited.
    pub fn with_fuel(mut self, capacity: f32, liters_per_100km: f32) -> Self {
        self.fuel_capacity = Some(capacity.max(0.0));
        self.fuel = capacity.max(0.0);
        self.consumption = liters_per_100km.max(0.0);
        self
    }

    /// Sets the looping engine sound.
    pub fn with_engine_sound(mut self, name: impl Into<String>) -> Self {
        self.engine_sound = Some(name.into());
        self
    }

    /// Sets the horn sound.
    pub fn with_horn_sound(mut self, name: impl Into<String>) -> Self {
        self.horn_sound = Some(name.into());
        self
    }

    /// Sets the engine start sound.
    pub fn with_start_sound(mut self, name: impl Into<String>) -> Self {
        self.start_sound = Some(name.into());
        self
    }

    /// Sets the gear change sound.
    pub fn with_shift_sound(mut self, name: impl Into<String>) -> Self {
        self.shift_sound = Some(name.into());
        self
    }

    /// Sets the crash sound.
    pub fn with_crash_sound(mut self, name: impl Into<String>) -> Self {
        self.crash_sound = Some(name.into());
        self
    }

    /// Starts with the engine off. Start it with [`Vehicle::start_engine`] or
    /// [`Context::start_engine`](crate::Context::start_engine).
    pub fn engine_off(mut self) -> Self {
        self.engine_on = false;
        self.rpm = 0.0;
        self
    }

    // ----------------------------------------------------------------- state

    /// Speed along the facing in m/s, negative when reversing.
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// Speed in km/h, negative when reversing.
    pub fn speed_kmh(&self) -> f32 {
        self.speed * 3.6
    }

    /// Sets the speed in m/s, e.g. to start a race already rolling. Limited to the top
    /// speeds.
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(-self.reverse_speed, self.top_speed);
    }

    /// The speed ready to speak, e.g. `"54 kilometers per hour"` (reversing included).
    pub fn describe_speed(&self) -> String {
        let kmh = self.speed_kmh().abs().round() as i64;
        let unit = if kmh == 1 { "kilometer" } else { "kilometers" };
        if self.is_reversing() {
            format!("reversing, {kmh} {unit} per hour")
        } else {
            format!("{kmh} {unit} per hour")
        }
    }

    /// Whether the vehicle is rolling backwards.
    pub fn is_reversing(&self) -> bool {
        self.speed < -0.05
    }

    /// Current gear: -1 reverse, 0 neutral (engine off), 1 and up forward.
    pub fn gear(&self) -> i32 {
        self.gear
    }

    /// The gear ready to speak: `"reverse"`, `"neutral"` or the number.
    pub fn gear_name(&self) -> String {
        match self.gear {
            -1 => "reverse".to_string(),
            0 => "neutral".to_string(),
            g => g.to_string(),
        }
    }

    /// Engine speed in revolutions per minute (0 when the engine is off).
    pub fn rpm(&self) -> f32 {
        self.rpm
    }

    /// Engine speed from 0.0 (idle) to 1.0 (highest rpm).
    pub fn rpm_fraction(&self) -> f32 {
        ((self.rpm - self.idle_rpm) / (self.max_rpm - self.idle_rpm)).clamp(0.0, 1.0)
    }

    /// A playback pitch for a looping engine sound recorded at idle: 0.8 at idle, 2.0 at
    /// the highest rpm. The engine sound set with [`Vehicle::with_engine_sound`] uses it
    /// automatically.
    pub fn engine_pitch(&self) -> f32 {
        0.8 + 1.2 * self.rpm_fraction()
    }

    /// Whether the engine runs.
    pub fn is_engine_on(&self) -> bool {
        self.engine_on
    }

    /// Starts the engine (silently; [`Context::start_engine`](crate::Context::start_engine)
    /// also plays the start sound). Returns `false` if the tank is empty.
    pub fn start_engine(&mut self) -> bool {
        if self.is_out_of_fuel() {
            return false;
        }
        self.engine_on = true;
        if self.gear == 0 {
            self.gear = 1;
        }
        self.rpm = self.rpm.max(self.idle_rpm);
        true
    }

    /// Stops the engine. The vehicle rolls out.
    pub fn stop_engine(&mut self) {
        self.engine_on = false;
        self.gear = 0;
        self.rpm = 0.0;
    }

    /// Liters left in the tank, `None` for unlimited fuel.
    pub fn fuel(&self) -> Option<f32> {
        self.fuel_capacity.map(|_| self.fuel)
    }

    /// Tank level from 0.0 (empty) to 1.0 (full); always 1.0 for unlimited fuel.
    pub fn fuel_fraction(&self) -> f32 {
        match self.fuel_capacity {
            Some(capacity) if capacity > 0.0 => self.fuel / capacity,
            Some(_) => 0.0,
            None => 1.0,
        }
    }

    /// Whether the tank is empty (never with unlimited fuel).
    pub fn is_out_of_fuel(&self) -> bool {
        self.fuel_capacity.is_some() && self.fuel <= 0.0
    }

    /// Adds fuel in liters, up to the tank size. Returns how much was added.
    pub fn refuel(&mut self, liters: f32) -> f32 {
        let Some(capacity) = self.fuel_capacity else {
            return 0.0;
        };
        let added = liters.max(0.0).min(capacity - self.fuel);
        self.fuel += added;
        added
    }

    /// Sets the cargo in kilograms, limited to 0..=[`Vehicle::max_load`]. Returns the new
    /// load.
    pub fn set_load(&mut self, kg: f32) -> f32 {
        self.load = kg.clamp(0.0, self.max_load);
        self.load
    }

    /// Meters driven so far (forward and backward), like an odometer.
    pub fn distance(&self) -> f32 {
        self.distance
    }

    // ----------------------------------------------------------------- simulation

    /// Updates speed, gear, rpm and fuel for one step of `dt` seconds. `mass` is the
    /// body's mass. Returns the turn in degrees and what changed.
    pub(crate) fn simulate(&mut self, mass: f32, dt: f32) -> (f32, StepReport) {
        let weight = (mass + self.load).max(1.0);
        let throttle = self.throttle.clamp(0.0, 1.0);
        let brake = self.brake.clamp(0.0, 1.0);
        let v = self.speed;
        if brake == 0.0 {
            self.hold.0 = false;
        }
        if throttle == 0.0 {
            self.hold.1 = false;
        }

        // Arcade pedals: moving backwards, gas brakes; standing still, the brake reverses.
        let (drive, pedal_brake) = if v > STOPPED {
            self.hold.0 |= brake > 0.0;
            (throttle, brake)
        } else if v < -STOPPED {
            self.hold.1 |= throttle > 0.0;
            (-brake, throttle)
        } else {
            let (brake_held, gas_held) = self.hold;
            let gas = if gas_held { 0.0 } else { throttle };
            let reverse = if brake_held { 0.0 } else { brake };
            let holding = if brake_held { brake } else { 0.0 }.max(if gas_held { throttle } else { 0.0 });
            (gas - reverse, holding)
        };
        let drive = if self.engine_on && !self.handbrake { drive } else { 0.0 };

        let mut accel = 0.0;
        if drive > 0.0 {
            let falloff = 1.0 - (v.max(0.0) / self.top_speed).powi(2);
            accel = self.engine_force * drive * falloff.max(0.0) / weight;
        } else if drive < 0.0 {
            let falloff = 1.0 - ((-v).max(0.0) / self.reverse_speed.max(0.01)).powi(2);
            accel = -self.engine_force * -drive * falloff.max(0.0) / weight;
        }
        accel = accel.clamp(-self.grip, self.grip);

        let mut slow = self.brake_force * pedal_brake / weight;
        if self.handbrake {
            slow = self.grip;
        }
        slow = slow.min(self.grip) + self.rolling_resistance;
        if drive == 0.0 && pedal_brake == 0.0 && self.engine_on {
            slow += self.engine_braking;
        }
        // Air drag grows with the square of the speed. Under power, the engine falloff
        // above already includes it.
        if drive == 0.0 {
            let top = if v >= 0.0 { self.top_speed } else { self.reverse_speed.max(0.01) };
            slow += AIR_DRAG * self.engine_force / weight * (v / top).powi(2);
        }

        let mut speed = v + accel * dt;
        let drop = slow * dt;
        if speed > 0.0 {
            speed = (speed - drop).max(0.0);
        } else if speed < 0.0 {
            speed = (speed + drop).min(0.0);
        }
        self.speed = speed.clamp(-self.reverse_speed, self.top_speed);

        // Steering: turning needs speed, up to the turn rate limit.
        let rate = (self.speed.abs() / self.turn_radius).to_degrees().min(self.max_turn_rate);
        let turn = rate * self.steer.clamp(-1.0, 1.0) * self.speed.signum() * dt;

        // Fuel.
        if self.fuel_capacity.is_some() && self.engine_on {
            let load_factor = weight / mass.max(1.0);
            let per_meter = self.consumption / 100_000.0 * (0.3 + 0.7 * throttle.max(brake)) * load_factor;
            self.fuel = (self.fuel - per_meter * self.speed.abs() * dt).max(0.0);
            if self.fuel <= 0.0 {
                self.stop_engine();
            }
        }

        let report = StepReport { gear_changed: self.update_gearbox(drive) };
        (turn, report)
    }

    /// Picks the gear from the speed and computes the rpm. Returns the new gear if it changed.
    fn update_gearbox(&mut self, drive: f32) -> Option<i32> {
        let before = self.gear;
        if !self.engine_on {
            self.gear = 0;
            self.rpm = 0.0;
            return (before != 0).then_some(0);
        }
        let gears = self.gears.max(1) as i32;
        let band = |g: i32| self.top_speed * g as f32 / gears as f32;
        let fraction = if self.speed < -0.05 || (self.speed.abs() <= 0.05 && drive < 0.0) {
            self.gear = -1;
            -self.speed / self.reverse_speed.max(0.01)
        } else {
            if self.gear < 1 {
                self.gear = 1;
            }
            while self.gear < gears && self.speed > band(self.gear) * 0.95 {
                self.gear += 1;
            }
            while self.gear > 1 && self.speed < band(self.gear - 1) * 0.7 {
                self.gear -= 1;
            }
            self.speed / band(self.gear)
        };
        let revving = drive.abs() * 0.3;
        let fraction = fraction.clamp(0.0, 1.0).max(revving);
        self.rpm = self.idle_rpm + (self.max_rpm - self.idle_rpm) * fraction;
        (self.gear != before).then_some(self.gear)
    }

    /// Removes the speed going into an obstacle whose surface faces `normal`, keeping the
    /// part that slides along it. Returns the impact speed.
    pub(crate) fn blocked(&mut self, facing: f32, normal: Vec3) -> f32 {
        let velocity = velocity(facing, self.speed);
        let into = velocity.dot(normal);
        if into >= 0.0 {
            return 0.0;
        }
        let slide = velocity - normal * into;
        self.speed = slide.dot(crate::math::forward(facing)).clamp(-self.reverse_speed, self.top_speed);
        -into
    }

    pub(crate) fn add_distance(&mut self, meters: f32) {
        self.distance += meters.abs();
    }
}

/// Velocity of a vehicle body facing `facing` with signed `speed`.
pub(crate) fn velocity(facing: f32, speed: f32) -> Vec3 {
    crate::math::forward(facing) * speed
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    const DT: f32 = 1.0 / 60.0;

    fn run(world: &mut World, seconds: f32) {
        for _ in 0..(seconds / DT).round() as usize {
            world.step(DT);
        }
    }

    /// Seconds a vehicle with full throttle needs to reach `kmh`.
    fn time_to(world: &mut World, id: BodyId, kmh: f32) -> f32 {
        world.vehicle_mut(id).unwrap().throttle = 1.0;
        let mut t = 0.0;
        while world.vehicle(id).unwrap().speed_kmh() < kmh && t < 300.0 {
            world.step(DT);
            t += DT;
        }
        t
    }

    #[test]
    fn car_accelerates_and_stops() {
        let mut world = World::new();
        let car = world.add(Body::car(Vec3::ZERO));
        let t = time_to(&mut world, car, 100.0);
        assert!((7.0..14.0).contains(&t), "0-100 km/h took {t} s");
        assert!(world.position(car).y > 50.0, "drives north, the default facing");
        assert!(world.vehicle(car).unwrap().gear() > 1, "shifted up");
        assert!((world.speed(car) * 3.6 - 100.0).abs() < 2.0);

        let v = world.vehicle_mut(car).unwrap();
        v.throttle = 0.0;
        v.brake = 1.0;
        run(&mut world, 5.0);
        assert!(world.vehicle(car).unwrap().speed().abs() < 0.01, "stopped, not reversing yet");
        world.vehicle_mut(car).unwrap().brake = 0.0;
        world.step(DT);
        world.vehicle_mut(car).unwrap().brake = 1.0;
        run(&mut world, 1.0);
        assert!(world.vehicle(car).unwrap().is_reversing(), "pressed again: reverses");
    }

    #[test]
    fn top_speed_is_respected() {
        let mut world = World::new();
        let car = world.add(Body::car(Vec3::ZERO).with_vehicle(Vehicle::car().with_top_speed_kmh(50.0)));
        world.vehicle_mut(car).unwrap().throttle = 1.0;
        run(&mut world, 30.0);
        assert!(world.vehicle(car).unwrap().speed_kmh() <= 50.0 + 1e-3);
    }

    #[test]
    fn load_makes_a_truck_slower_and_brake_longer() {
        let mut world = World::new();
        let empty = world.add(Body::truck(Vec3::ZERO));
        let full = world.add(Body::truck(vec3(100.0, 0.0, 0.0)));
        world.vehicle_mut(full).unwrap().set_load(25_000.0);
        assert_eq!(world.weight(full), 37_000.0);
        let t_empty = time_to(&mut world, empty, 60.0);
        world.vehicle_mut(empty).unwrap().throttle = 0.0;
        let t_full = time_to(&mut world, full, 60.0);
        assert!(t_full > t_empty * 2.0, "empty {t_empty} s, full {t_full} s");

        for id in [empty, full] {
            let v = world.vehicle_mut(id).unwrap();
            v.set_speed(60.0 / 3.6);
            v.throttle = 0.0;
            v.brake = 1.0;
        }
        let (y_empty, y_full) = (world.position(empty).y, world.position(full).y);
        run(&mut world, 10.0);
        let stop_empty = world.position(empty).y - y_empty;
        let stop_full = world.position(full).y - y_full;
        assert!(stop_full > stop_empty * 2.0, "braking: empty {stop_empty} m, full {stop_full} m");
    }

    #[test]
    fn brake_reverses_when_stopped() {
        let mut world = World::new();
        let car = world.add(Body::car(Vec3::ZERO));
        world.vehicle_mut(car).unwrap().brake = 1.0;
        run(&mut world, 3.0);
        let v = world.vehicle(car).unwrap();
        assert!(v.is_reversing());
        assert_eq!(v.gear(), -1);
        assert!(v.speed_kmh() >= -25.0 - 1e-3);
        assert!(world.position(car).y < 0.0);
        assert!(v.describe_speed().starts_with("reversing"));
    }

    #[test]
    fn steering_needs_speed() {
        let mut world = World::new();
        let car = world.add(Body::car(Vec3::ZERO));
        world.vehicle_mut(car).unwrap().steer = 1.0;
        run(&mut world, 1.0);
        assert_eq!(world.facing(car), 0.0, "a parked car does not turn");
        world.vehicle_mut(car).unwrap().set_speed(10.0);
        run(&mut world, 0.5);
        let facing = world.facing(car);
        assert!(facing > 20.0 && facing < 90.0, "turned right to {facing}");
    }

    #[test]
    fn crash_into_wall() {
        let mut world = World::new();
        let wall = world.add(Body::fixed(vec3(0.0, 20.0, 0.0)).with_box(10.0, 1.0, 3.0));
        let car = world.add(Body::car(Vec3::ZERO));
        world.vehicle_mut(car).unwrap().set_speed(15.0);
        let mut crash = None;
        for _ in 0..200 {
            world.clear_events();
            world.step(DT);
            if let Some(c) = world.crashed(car) {
                crash = Some(c);
                break;
            }
        }
        let (other, speed) = crash.expect("crashed");
        assert_eq!(other, wall);
        assert!(speed > 10.0);
        assert!(world.vehicle(car).unwrap().speed().abs() < 1.0, "lost its speed");
        assert!(world.position(car).y < 19.0, "did not pass the wall");
    }

    #[test]
    fn fuel_runs_out() {
        let mut world = World::new();
        let car = world.add(Body::car(Vec3::ZERO).with_vehicle(Vehicle::car().with_fuel(0.05, 10.0)));
        world.vehicle_mut(car).unwrap().throttle = 1.0;
        let mut t = 0.0;
        while world.vehicle(car).unwrap().is_engine_on() && t < 120.0 {
            world.step(DT);
            t += DT;
        }
        let v = world.vehicle(car).unwrap();
        assert!(v.is_out_of_fuel());
        assert!(!v.is_engine_on());
        assert_eq!(v.gear(), 0);
        let driven = v.distance();
        assert!((400.0..600.0).contains(&driven), "0.05 l at 10 l/100 km is 500 m, drove {driven}");
        let v = world.vehicle_mut(car).unwrap();
        assert!(v.refuel(10.0) > 0.0);
        assert!(v.start_engine());
    }

    #[test]
    fn context_shortcuts() {
        let mut runner = Engine::new("t").headless().build_fn(|_| {});
        runner.frame();
        let ctx = &mut runner.ctx;
        let car = ctx.physics.add(Body::car(Vec3::ZERO).with_vehicle(Vehicle::car().engine_off()));
        assert!(!ctx.physics.vehicle(car).unwrap().is_engine_on());
        assert!(ctx.start_engine(car));
        assert!(!ctx.start_engine(car), "already running");
        runner.key_down(Key::Up, false);
        for _ in 0..30 {
            runner.ctx.drive(car);
            std::thread::sleep(std::time::Duration::from_millis(5));
            runner.frame();
        }
        assert!(runner.ctx.physics.vehicle(car).unwrap().speed() > 0.0);
        runner.ctx.stop_engine(car);
        assert_eq!(runner.ctx.physics.vehicle(car).unwrap().gear(), 0);
    }
}
