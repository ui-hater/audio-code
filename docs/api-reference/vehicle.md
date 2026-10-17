# Vehicles

Cars and trucks: weight, speed, engine, gears, steering, brakes, fuel, cargo and sounds. A
vehicle is an optional part of a physics body (`Body::vehicle: Option<Vehicle>`); bodies
without one are not affected. The physics world drives every vehicle at the fixed physics
rate from its pedals and steering. Guide: [../vehicles.md](../vehicles.md).

```rust
use audio_code::prelude::*;   // Vehicle, Body, World, PhysicsEvent
```

Units: speeds are stored in m/s (builders ending in `_kmh` take km/h), forces in newtons,
weights in kilograms, accelerations in m/s², fuel in liters.

## Creating vehicles

### `Body::car(position: Vec3) -> Body`

A car: kinematic body, sphere of 1 m radius, mass 1300 kg, tag `"car"`, `Vehicle::car()`.

```rust
let car = ctx.physics.add(Body::car(vec3(0.0, 0.0, 0.0)).with_facing(90.0));
ctx.follow(car);
```

### `Body::truck(position: Vec3) -> Body`

A truck: kinematic body, sphere of 1.5 m radius, empty mass 12 000 kg, tag `"truck"`,
`Vehicle::truck()`.

```rust
let truck = ctx.physics.add(Body::truck(vec3(10.0, 0.0, 0.0)));
```

### `body.with_vehicle(vehicle: Vehicle) -> Body`

Makes any body a vehicle with these settings and sets its kind to kinematic. The body's
`mass` is the empty weight. Call it after `Body::car` / `Body::truck` to change their
vehicle.

```rust
let bus = ctx.physics.add(
    Body::kinematic(Vec3::ZERO)
        .with_sphere(1.5)
        .with_mass(11_000.0)
        .with_tag("bus")
        .with_vehicle(Vehicle::truck().with_top_speed_kmh(80.0).with_max_load(6000.0)),
);
```

### `Vehicle::car() -> Vehicle`

Car settings: top 180 km/h, reverse 25 km/h, engine 4500 N, brakes 11 000 N, grip 9 m/s²,
turn radius 5 m, at most 90°/s, 5 gears from 800 to 6500 rpm, unlimited load and fuel.
`Vehicle::default()` is the same.

```rust
let sports = Vehicle::car().with_top_speed_kmh(260.0).with_engine_force(9000.0);
```

### `Vehicle::truck() -> Vehicle`

Truck settings: top 90 km/h, reverse 10 km/h, engine 20 000 N, brakes 110 000 N, grip
7 m/s², turn radius 11 m, at most 35°/s, 12 gears from 600 to 2200 rpm, up to 25 000 kg
cargo.

```rust
let tipper = Vehicle::truck().with_max_load(15_000.0);
```

## Settings (builders)

Each builder returns the vehicle. All settings are also public fields with the same name
(without `with_`), for changing them while driving.

### `vehicle.with_top_speed_kmh(kmh: f32) -> Vehicle`

Highest forward speed. Full throttle approaches it slowly.

```rust
let slow = Vehicle::car().with_top_speed_kmh(60.0);
```

### `vehicle.with_reverse_speed_kmh(kmh: f32) -> Vehicle`

Highest reverse speed.

```rust
let v = Vehicle::car().with_reverse_speed_kmh(15.0);
```

### `vehicle.with_engine_force(newtons: f32) -> Vehicle`

Driving force at full throttle. Acceleration = force / weight.

```rust
let strong = Vehicle::truck().with_engine_force(30_000.0);
```

### `vehicle.with_brake_force(newtons: f32) -> Vehicle`

Braking force at full brake pedal. Deceleration = force / weight, at most the grip.

```rust
let weak_brakes = Vehicle::car().with_brake_force(5000.0);
```

### `vehicle.with_grip(grip: f32) -> Vehicle`

Most acceleration or deceleration the tyres transfer, in m/s²: 9 dry, 3 wet, 1 ice.

```rust
let on_ice = Vehicle::car().with_grip(1.0);
```

### `vehicle.with_rolling_resistance(resistance: f32) -> Vehicle`

Speed lost per second while rolling, in m/s². Always active.

```rust
let v = Vehicle::car().with_rolling_resistance(0.5);   // sand
```

### `vehicle.with_engine_braking(braking: f32) -> Vehicle`

Extra speed lost per second with no pedal pressed and the engine running, in m/s².

```rust
let v = Vehicle::car().with_engine_braking(0.2);
```

### `vehicle.with_turn_radius(meters: f32) -> Vehicle`

Radius of the tightest circle. Smaller turns tighter.

```rust
let taxi = Vehicle::car().with_turn_radius(4.0);
```

### `vehicle.with_max_turn_rate(degrees_per_second: f32) -> Vehicle`

Fastest turn, reached at higher speeds.

```rust
let nimble = Vehicle::car().with_max_turn_rate(140.0);
```

### `vehicle.with_gears(gears: u32, idle_rpm: f32, max_rpm: f32) -> Vehicle`

Number of forward gears (at least 1), idle and highest engine speed.

```rust
let v = Vehicle::truck().with_gears(16, 600.0, 2400.0);
```

### `vehicle.with_load(kg: f32) -> Vehicle`

Starts with cargo, limited to `max_load`.

```rust
let loaded = Vehicle::truck().with_load(18_000.0);
```

### `vehicle.with_max_load(kg: f32) -> Vehicle`

Most cargo. Default unlimited for cars, 25 000 kg for trucks.

```rust
let pickup = Vehicle::car().with_max_load(800.0);
```

### `vehicle.with_fuel(capacity: f32, liters_per_100km: f32) -> Vehicle`

Gives the vehicle a tank (starts full) that uses `liters_per_100km` at full throttle.
Without it, fuel is unlimited. Gentle throttle uses down to 30 %, cargo uses more.

```rust
let v = Vehicle::car().with_fuel(50.0, 8.0);
```

### `vehicle.engine_off() -> Vehicle`

Starts with the engine off (gear 0). Start it with `ctx.start_engine`.

```rust
let parked = ctx.physics.add(Body::car(Vec3::ZERO).with_vehicle(Vehicle::car().engine_off()));
```

### Sounds

`with_engine_sound(name)`, `with_horn_sound(name)`, `with_start_sound(name)`,
`with_shift_sound(name)`, `with_crash_sound(name)`, each taking `impl Into<String>`. All are
optional sound names (files or registered sounds).

| Sound | Played |
|-------|--------|
| engine | looped at the vehicle while the engine runs, pitch = `engine_pitch()`, volume 0.6 to 1.0 with the throttle |
| horn | by `ctx.horn(id)` |
| start | by `ctx.start_engine(id)` |
| shift | on every gear change |
| crash | on every crash, volume = crash speed / 15 m/s |

```rust
let car = Vehicle::car()
    .with_engine_sound("car/engine")
    .with_horn_sound("car/horn")
    .with_start_sound("car/start")
    .with_shift_sound("car/shift")
    .with_crash_sound("car/crash");
```

## Pedals and steering (fields)

Set these every frame, or let `ctx.drive(id)` set them from the keyboard.

| Field | Type | Meaning |
|-------|------|---------|
| `throttle` | `f32` | gas pedal, 0.0 to 1.0; brakes while reversing |
| `brake` | `f32` | brake pedal, 0.0 to 1.0; from a standstill it reverses |
| `steer` | `f32` | -1.0 full left to 1.0 full right |
| `handbrake` | `bool` | stops as hard as the grip allows; no driving force |

After braking to a stop the vehicle stays put while that pedal is held; release and press
it again to drive the other way.

```rust
// A computer driver that steers towards a target.
let target = vec3(50.0, 50.0, 0.0);
let from = ctx.physics.position(enemy);
let turn = math::angle_difference(ctx.physics.facing(enemy), math::bearing(from, target));
if let Some(v) = ctx.physics.vehicle_mut(enemy) {
    v.throttle = 0.7;
    v.steer = (turn / 45.0).clamp(-1.0, 1.0);
}
```

## Context shortcuts

### `ctx.drive(vehicle: BodyId)`

Sets the pedals from the keyboard: Up / W gas, Down / S brake and reverse, Left / Right or
A / D steer, Space handbrake. Call it every frame. While a menu is open all pedals are
released. Does nothing for bodies without a vehicle.

```rust
ctx.drive(player);
```

### `ctx.horn(vehicle: BodyId) -> Sound`

Plays the horn sound at the vehicle (it moves with it). Silent handle without a horn sound.

```rust
if ctx.pressed(Key::H) {
    ctx.horn(player);
}
```

### `ctx.start_engine(vehicle: BodyId) -> bool`

Starts the engine and plays the start sound. `false` if the body is no vehicle, the engine
already runs or the tank is empty.

```rust
if ctx.pressed(Key::E) && !ctx.start_engine(player) {
    ctx.speak("It does not start");
}
```

### `ctx.stop_engine(vehicle: BodyId)`

Stops the engine (gear 0). The engine sound fades out and the vehicle rolls out.

```rust
ctx.stop_engine(player);
```

## World functions

### `ctx.physics.vehicle(id: BodyId) -> Option<&Vehicle>`

The vehicle part of a body, `None` for other bodies.

```rust
if let Some(v) = ctx.physics.vehicle(player) {
    let text = v.describe_speed();
    ctx.speak(text);
}
```

### `ctx.physics.vehicle_mut(id: BodyId) -> Option<&mut Vehicle>`

Mutable access: pedals, load, fuel, settings.

```rust
if let Some(v) = ctx.physics.vehicle_mut(player) {
    v.grip = 3.0;   // it starts raining
}
```

### `ctx.physics.weight(id: BodyId) -> f32`

Total weight in kilograms: body mass plus vehicle load. Works for every body (without a
vehicle it is the mass).

```rust
let tons = ctx.physics.weight(player) / 1000.0;
ctx.speak(format!("{tons:.1} tons"));
```

### `ctx.physics.speed(id: BodyId) -> f32`

How fast a body moves in m/s (length of its velocity). For vehicles it is the driving
speed without sign.

```rust
let kmh = ctx.physics.speed(player) * 3.6;
```

### `ctx.physics.crashed(vehicle: BodyId) -> Option<(BodyId, f32)>`

If the vehicle crashed into a solid body this frame: what it hit and the speed lost in m/s.
Scraping along a wall loses little, hitting it head-on loses everything.

```rust
if let Some((other, speed)) = ctx.physics.crashed(player) {
    let text = format!("Crash into {} at {:.0} kilometers per hour", ctx.physics.tag(other), speed * 3.6);
    ctx.speak(text);
}
```

### `ctx.physics.gear_changed(vehicle: BodyId) -> Option<i32>`

If the vehicle changed gear this frame: the new gear.

```rust
if ctx.physics.gear_changed(player) == Some(-1) {
    ctx.speak("Reverse");
}
```

Vehicles also trigger sensors like any moving body: `ctx.physics.entered_tag(car, "checkpoint")`.

## Events

```rust
pub enum PhysicsEvent {
    // ...
    Crashed { vehicle: BodyId, other: BodyId, speed: f32 },   // speed lost in m/s
    GearChanged { vehicle: BodyId, gear: i32 },              // -1 reverse, 0 neutral, 1..
}
```

## State (methods)

### `vehicle.speed() -> f32`

Speed along the facing in m/s, negative when reversing.

```rust
let v = Vehicle::car();
assert_eq!(v.speed(), 0.0);
```

### `vehicle.speed_kmh() -> f32`

Speed in km/h, negative when reversing.

```rust
let kmh = ctx.physics.vehicle(player).map_or(0.0, |v| v.speed_kmh());
```

### `vehicle.set_speed(speed: f32)`

Sets the speed in m/s, limited to the top speeds. For starting already in motion.

```rust
if let Some(v) = ctx.physics.vehicle_mut(player) {
    v.set_speed(100.0 / 3.6);
}
```

### `vehicle.describe_speed() -> String`

`"54 kilometers per hour"`, `"1 kilometer per hour"`, `"reversing, 8 kilometers per hour"`.

```rust
let mut v = Vehicle::car();
v.set_speed(15.0);
assert_eq!(v.describe_speed(), "54 kilometers per hour");
```

### `vehicle.is_reversing() -> bool`

Whether it rolls backwards.

```rust
let backwards = ctx.physics.vehicle(player).is_some_and(|v| v.is_reversing());
if backwards && ctx.once("reverse_beep") {
    ctx.audio.play_tone(1000.0, 0.2);
}
```

### `vehicle.gear() -> i32`

-1 reverse, 0 neutral (engine off), 1 and up forward.

```rust
let gear = ctx.physics.vehicle(player).map_or(0, |v| v.gear());
```

### `vehicle.gear_name() -> String`

`"reverse"`, `"neutral"` or the number.

```rust
if let Some(v) = ctx.physics.vehicle(player) {
    let text = format!("Gear {}", v.gear_name());
    ctx.speak(text);
}
```

### `vehicle.rpm() -> f32`

Engine speed in revolutions per minute, 0 with the engine off.

```rust
let rpm = ctx.physics.vehicle(player).map_or(0.0, |v| v.rpm());
```

### `vehicle.rpm_fraction() -> f32`

Engine speed from 0.0 (idle) to 1.0 (highest rpm).

```rust
let redline = ctx.physics.vehicle(player).is_some_and(|v| v.rpm_fraction() > 0.95);
```

### `vehicle.engine_pitch() -> f32`

Playback pitch for an engine loop recorded at idle: 0.8 at idle up to 2.0. The automatic
engine sound uses it; use it yourself for your own engine sound.

```rust
let hum = ctx.audio.play_looped("my_engine");
if let Some(v) = ctx.physics.vehicle(player) {
    hum.set_pitch(v.engine_pitch());
}
```

### `vehicle.is_engine_on() -> bool`

Whether the engine runs.

```rust
let running = ctx.physics.vehicle(player).is_some_and(|v| v.is_engine_on());
```

### `vehicle.start_engine() -> bool`

Starts the engine without the start sound. `false` if the tank is empty.

```rust
let mut v = Vehicle::car().engine_off();
assert!(v.start_engine());
```

### `vehicle.stop_engine()`

Stops the engine (gear 0, rpm 0).

```rust
let mut v = Vehicle::car();
v.stop_engine();
assert_eq!(v.gear(), 0);
```

### `vehicle.fuel() -> Option<f32>`

Liters left, `None` for unlimited fuel.

```rust
if let Some(liters) = ctx.physics.vehicle(player).and_then(|v| v.fuel()) {
    ctx.speak(format!("{liters:.0} liters left"));
}
```

### `vehicle.fuel_fraction() -> f32`

Tank level 0.0 to 1.0; always 1.0 for unlimited fuel.

```rust
let low = ctx.physics.vehicle(player).is_some_and(|v| v.fuel_fraction() < 0.1);
```

### `vehicle.is_out_of_fuel() -> bool`

Whether the tank is empty (never with unlimited fuel). The engine stops when it runs dry.

```rust
let empty = ctx.physics.vehicle(player).is_some_and(|v| v.is_out_of_fuel());
```

### `vehicle.refuel(liters: f32) -> f32`

Adds fuel up to the tank size and returns how much was added (0 for unlimited fuel).

```rust
if let Some(v) = ctx.physics.vehicle_mut(player) {
    let added = v.refuel(100.0);
}
```

### `vehicle.set_load(kg: f32) -> f32`

Sets the cargo, limited to `0..=max_load`, and returns the new load. Heavier means slower
acceleration and longer braking.

```rust
if let Some(v) = ctx.physics.vehicle_mut(player) {
    v.set_load(12_000.0);
}
```

### `vehicle.distance() -> f32`

Meters driven so far (odometer).

```rust
let km = ctx.physics.vehicle(player).map_or(0.0, |v| v.distance() / 1000.0);
```

## Fields

Settings: `top_speed`, `reverse_speed` (m/s), `engine_force`, `brake_force` (N), `grip`,
`rolling_resistance`, `engine_braking` (m/s²), `turn_radius` (m), `max_turn_rate` (°/s),
`gears`, `idle_rpm`, `max_rpm`, `load`, `max_load` (kg), `fuel_capacity: Option<f32>` (l),
`consumption` (l/100 km), `engine_sound`, `horn_sound`, `start_sound`, `shift_sound`,
`crash_sound: Option<String>`. Controls: `throttle`, `brake`, `steer`, `handbrake`.
