# Cars and trucks

Vehicles are an optional part of a physics body. A normal body (player, wall, coin) has no
vehicle and is not affected by anything on this page. A body with a vehicle gets a
**weight**, a **speed**, an engine with gears, brakes, a handbrake, steering, an optional
fuel tank, cargo and sounds. The physics world drives it for you at the fixed physics rate.

Try it: `cargo run --example driving`.

## The shortest car game

```rust
use audio_code::prelude::*;

fn main() -> audio_code::Result<()> {
    let mut car = None;
    audio_code::run("Drive", move |ctx| {
        let id = *car.get_or_insert_with(|| {
            let id = ctx.physics.add(Body::car(Vec3::ZERO).with_vehicle(
                Vehicle::car().with_engine_sound("engine").with_horn_sound("horn"),
            ));
            ctx.follow(id);
            id
        });
        ctx.drive(id); // arrows drive, Space is the handbrake
        if ctx.pressed(Key::H) {
            ctx.horn(id);
        }
        if ctx.pressed(Key::F) {
            let text = ctx.physics.vehicle(id).map(|v| v.describe_speed()).unwrap_or_default();
            ctx.speak(text); // "54 kilometers per hour"
        }
        if ctx.pressed(Key::Escape) {
            ctx.quit();
        }
    })
}
```

- `Body::car(position)` is a kinematic body with a 1 m sphere, 1300 kg and `Vehicle::car()`.
  `Body::truck(position)` has a 1.5 m sphere, 12 t and `Vehicle::truck()`.
- `ctx.follow(id)` puts the player's ears into the car, so the world turns around you
  when you steer.
- `ctx.drive(id)` reads the keyboard every frame.

## Controls

`ctx.drive(id)` maps the keys to the pedals:

| Key | Pedal |
|-----|-------|
| Up or W | gas (`throttle = 1.0`) |
| Down or S | brake; standing still, press again to drive backwards |
| Left / Right or A / D | steering wheel (`steer = -1.0 / 1.0`) |
| Space | handbrake |

You can also set the pedals yourself, for example for computer drivers, analogue values or
other keys. They are plain fields:

```rust
if let Some(v) = ctx.physics.vehicle_mut(enemy) {
    v.throttle = 0.6;   // 0.0 to 1.0
    v.brake = 0.0;      // 0.0 to 1.0
    v.steer = -0.3;     // -1.0 left to 1.0 right
    v.handbrake = false;
}
```

The pedals behave like in most driving games:

- Moving forward, the brake pedal brakes. Moving backward, the gas pedal brakes.
- After braking to a stop, the vehicle stays put while the pedal is held. Release it and
  press it again to drive the other way. From a standstill, the brake pedal drives
  backwards right away.
- Without any pedal, the vehicle slows down by itself (engine braking, rolling resistance
  and air drag).

## Weight

The weight is the body's mass plus the cargo, in kilograms:

```rust
let truck = ctx.physics.add(Body::truck(Vec3::ZERO));          // 12 000 kg empty
ctx.physics.vehicle_mut(truck).map(|v| v.set_load(20_000.0));  // plus 20 t of cargo
let total = ctx.physics.weight(truck);                         // 32 000.0
```

The engine and brake forces stay the same, so weight changes how the vehicle feels:

| | Empty truck (12 t) | Full truck (37 t) |
|---|---|---|
| 0 to 60 km/h | about 13 s | about 50 s |
| Stopping from 60 km/h | about 19 m | about 45 m |

A car (1.3 t) goes from 0 to 100 km/h in about 10 s and stops from 100 km/h in about 44 m.

A different empty weight is `.with_mass(kg)` on the body. `Vehicle::max_load` limits the
cargo (25 t for trucks, unlimited for cars); `set_load` clamps to it.

```rust
let van = ctx.physics.add(
    Body::car(Vec3::ZERO)
        .with_mass(2200.0)
        .with_tag("van")
        .with_vehicle(Vehicle::car().with_top_speed_kmh(150.0).with_max_load(1000.0)),
);
```

## Speed

The speed is a signed number along the direction the vehicle faces: positive forward,
negative backward.

```rust
if let Some(v) = ctx.physics.vehicle(car) {
    let mps = v.speed();          // meters per second
    let kmh = v.speed_kmh();      // kilometers per hour
    let text = v.describe_speed(); // "54 kilometers per hour" / "reversing, 8 kilometers per hour"
}
let any_body = ctx.physics.speed(car); // velocity length, works for every body
```

| Setting | Car | Truck |
|---------|-----|-------|
| `with_top_speed_kmh` | 180 | 90 |
| `with_reverse_speed_kmh` | 25 | 10 |
| `with_engine_force` (N) | 4500 | 14 000 |
| `with_brake_force` (N) | 11 000 | 110 000 |
| `with_grip` (m/s²) | 9 | 7 |
| `with_rolling_resistance` (m/s²) | 0.15 | 0.1 |
| `with_engine_braking` (m/s²) | 0.8 | 0.5 |

- **Acceleration** is engine force divided by weight. It fades out towards the top speed,
  which full throttle approaches slowly (a car is at 170 km/h after about a minute).
- Off the gas, a car at full speed rolls out in about 40 s and 850 m, a truck in about
  30 s and 350 m.
- **Grip** caps how hard the tyres can speed up or brake. Lower it for rain (3) or ice (1).
- To start a race already moving: `v.set_speed(20.0)` (m/s).

## Steering

Steering turns the body's `facing`. A parked vehicle cannot turn. The faster it goes, the
faster it turns, up to a limit:

| Setting | Car | Truck |
|---------|-----|-------|
| `with_turn_radius` (m), the tightest circle | 5 | 11 |
| `with_max_turn_rate` (degrees per second) | 90 | 35 |

Shapes never rotate in this engine, which is why cars and trucks are spheres: a sphere
fits a vehicle facing any direction and slides smoothly along walls.

## Engine and gears

```rust
let car = ctx.physics.add(Body::car(Vec3::ZERO).with_vehicle(Vehicle::car().engine_off()));
ctx.start_engine(car);   // plays the start sound; false if the tank is empty
ctx.stop_engine(car);    // the car rolls out
```

An automatic gearbox picks the gear from the speed. `gear()` is -1 for reverse, 0 for
neutral (engine off) and 1 and up for the forward gears. `gear_name()` returns
`"reverse"`, `"neutral"` or the number. The engine speed follows: `rpm()`,
`rpm_fraction()` (0 idle to 1 maximum) and `engine_pitch()` (0.8 to 2.0, for a looping
engine sound).

```rust
let truck = Vehicle::truck().with_gears(16, 600.0, 2400.0);   // 16 gears, idle and max rpm
if let Some(gear) = ctx.physics.gear_changed(car) {
    if gear == -1 {
        ctx.speak("Reverse");
    }
}
```

## Fuel

Fuel is unlimited unless you give the vehicle a tank:

```rust
let car = ctx.physics.add(Body::car(Vec3::ZERO).with_vehicle(Vehicle::car().with_fuel(50.0, 8.0)));
// 50 liter tank, starts full, 8 liters per 100 km at full throttle
if let Some(v) = ctx.physics.vehicle_mut(car) {
    let left = v.fuel();            // Some(liters)
    let share = v.fuel_fraction();  // 0.0 to 1.0
    if v.is_out_of_fuel() {
        v.refuel(20.0);
    }
}
```

Gentle throttle uses less, cargo uses more. When the tank runs dry, the engine stops.
`distance()` is the odometer in meters.

## Sounds

All sounds are optional names (files or registered sounds, as everywhere in the engine):

| Builder | When |
|---------|------|
| `with_engine_sound` | loops at the vehicle while the engine runs; pitch follows the rpm, volume the throttle |
| `with_horn_sound` | `ctx.horn(id)` |
| `with_start_sound` | `ctx.start_engine(id)` |
| `with_shift_sound` | every gear change |
| `with_crash_sound` | every crash, louder for harder crashes |

Record the engine loop at idle; the engine plays it at 0.8× speed when idling and up to 2×
at full rpm.

## Walls, crashes and checkpoints

Vehicles move like a character with `move_by`: walls stop them, they slide along walls
they touch at an angle, they push loose dynamic bodies away and they trigger sensors.

```rust
if let Some((other, speed)) = ctx.physics.crashed(car) {
    // speed lost, in m/s (10 m/s = 36 km/h)
    if speed > 8.0 {
        ctx.speak("Ouch!");
    }
}
if let Some(checkpoint) = ctx.physics.entered_tag(car, "checkpoint") {
    ctx.physics.remove(checkpoint);
    ctx.play("checkpoint");
}
```

Hitting a wall head-on loses all the speed, scraping along it loses only the part going
into the wall. The events are `PhysicsEvent::Crashed { vehicle, other, speed }` and
`PhysicsEvent::GearChanged { vehicle, gear }`.

## Limits

- Vehicles drive level: they keep their height and gravity does not pull them. Roads with
  hills are not simulated.
- No skidding or drifting: the vehicle always moves where it faces.
- Every vehicle is kinematic, so two vehicles block each other like walls.

All functions with signatures and examples: [API reference: vehicle](api-reference/vehicle.md).
