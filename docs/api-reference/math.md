# Math

Vectors, angles and direction helpers made for audio games, plus decibels and random
numbers. Everything lives in `audio_code::math`. The prelude imports the module itself as
`math`, plus `Vec2`, `Vec3`, `vec2`, `vec3` and `random`.

```rust
use audio_code::prelude::*;     // math, Vec2, Vec3, vec2, vec3, random
use audio_code::math::*;        // or every helper without the `math::` prefix
```

## Conventions

- `+x` east, `+y` north, `+z` up.
- Yaw / facing / bearing: compass degrees, 0 = north, 90 = east, 180 = south, 270 = west,
  positive turns right (clockwise).
- Pitch: degrees above the horizon, 0 level, 90 straight up, -90 straight down.

## Vectors

`Vec2`, `Vec3`, `vec2` and `vec3` are re-exported from the [glam](https://docs.rs/glam)
crate. A `Vec3` is a position, a direction or a velocity, in meters (or m/s).

### `vec3(x: f32, y: f32, z: f32) -> Vec3`

Creates a 3D vector. `vec3(3.0, -2.0, 1.5)` is 3 m east, 2 m south, 1.5 m up.

```rust
let player = vec3(3.0, -2.0, 1.5);
let above = player + vec3(0.0, 0.0, 2.0);
let distance = player.distance(above);    // 2.0
let east = player.x;                      // 3.0
```

### `vec2(x: f32, y: f32) -> Vec2`

Creates a 2D vector. `ctx.input.movement()` returns one (`x` right, `y` forward).
`.extend(z)` turns it into a `Vec3`.

```rust
let flat = vec2(1.0, 0.0);
let world = flat.extend(0.0);   // vec3(1.0, 0.0, 0.0)
```

Commonly used glam methods: `length()`, `distance(other)`, `normalize_or_zero()`,
`dot(other)`, `lerp(other, t)`, `+ - * /`, constants `Vec3::ZERO`, `Vec3::X`, `Vec3::Y`,
`Vec3::Z`.

```rust
let dir = (vec3(10.0, 0.0, 0.0) - Vec3::ZERO).normalize_or_zero();   // Vec3::X
let halfway = Vec3::ZERO.lerp(vec3(10.0, 0.0, 0.0), 0.5);           // vec3(5.0, 0.0, 0.0)
```

## Directions

### `math::forward(yaw: f32) -> Vec3`

Unit vector pointing in compass direction `yaw` on the ground (z = 0).

```rust
let east = math::forward(90.0);   // vec3(1.0, 0.0, 0.0)
let ahead = ctx.physics.position(player) + math::forward(ctx.physics.facing(player)) * 5.0;
```

### `math::right(yaw: f32) -> Vec3`

Unit vector pointing to the right of compass direction `yaw`.

```rust
let right_of_north = math::right(0.0);   // vec3(1.0, 0.0, 0.0), east
```

### `math::forward_3d(yaw: f32, pitch: f32) -> Vec3`

Unit vector in compass direction `yaw`, tilted up by `pitch` degrees. With pitch 0 it
equals `forward`.

```rust
let up_north = math::forward_3d(0.0, 45.0);   // half north, half up
```

### `math::up_3d(yaw: f32, pitch: f32) -> Vec3`

Unit vector out of the top of the head of someone facing `yaw` and looking up by `pitch`.
With pitch 0 it is `+z`.

```rust
let head = math::up_3d(0.0, 90.0);   // looking straight up: the head points south
```

### `math::bearing(from: Vec3, to: Vec3) -> f32`

Compass bearing in degrees from `from` to `to`, ignoring height. 0 if both are at the same
spot.

```rust
let b = math::bearing(Vec3::ZERO, vec3(0.0, -5.0, 0.0));   // 180.0, south
```

### `math::elevation(from: Vec3, to: Vec3) -> f32`

Angle from `from` up to `to`: 90 straight above, 0 same height, -90 straight below.

```rust
let e = math::elevation(Vec3::ZERO, vec3(3.0, 0.0, 3.0));   // 45.0
```

### `math::look_at(from: Vec3, to: Vec3) -> (f32, f32)`

Yaw and pitch that point from `from` to `to`. For a body, `ctx.physics.look_at` does this
in one call.

```rust
let (yaw, pitch) = math::look_at(Vec3::ZERO, vec3(0.0, -4.0, -4.0));   // (180.0, -45.0)
```

### `math::move_towards(position: Vec3, yaw: f32, distance: f32) -> Vec3`

`position` moved `distance` meters in compass direction `yaw` on the ground plane. No
collisions (use `ctx.physics.move_forward` for bodies).

```rust
let spawn = math::move_towards(vec3(5.0, 5.0, 0.0), 270.0, 3.0);   // 3 m west: vec3(2.0, 5.0, 0.0)
```

## Angles

### `math::normalize_angle(degrees: f32) -> f32`

Wraps an angle into 0.0..360.0.

```rust
assert_eq!(math::normalize_angle(-90.0), 270.0);
assert_eq!(math::normalize_angle(370.0), 10.0);
```

### `math::angle_difference(from: f32, to: f32) -> f32`

Shortest signed difference `to - from` in -180..=180. Positive means turning right.

```rust
assert_eq!(math::angle_difference(350.0, 10.0), 20.0);   // 20° to the right
assert_eq!(math::angle_difference(10.0, 350.0), -20.0);  // 20° to the left
```

## Speaking positions

### `math::direction_name(degrees: f32) -> &'static str`

A compass angle as one of 8 names: `"north"`, `"north east"`, `"east"`, `"south east"`,
`"south"`, `"south west"`, `"west"`, `"north west"`.

```rust
let facing = ctx.physics.facing(player);
ctx.speak(format!("Facing {}", math::direction_name(facing)));
```

### `math::clock_position(relative_degrees: f32) -> u32`

An angle relative to the player (0 = straight ahead) as a clock hour 1..=12: 90° is 3
o'clock, 180° is 6 o'clock.

```rust
assert_eq!(math::clock_position(90.0), 3);
assert_eq!(math::clock_position(0.0), 12);
```

### `math::describe_position(from: Vec3, yaw: f32, target: Vec3) -> String`

Where `target` is for someone at `from` facing `yaw`, ready to speak. The distance is
straight-line 3D, rounded; a height difference of a meter or more is added.

- `"3 o'clock, 12 meters"`
- `"12 o'clock, 11 meters, 4 meters above"`
- `"directly below, 5 meters"`

```rust
let text = math::describe_position(Vec3::ZERO, 0.0, vec3(5.0, 0.0, 0.0));
assert_eq!(text, "3 o'clock, 5 meters");
```

## Numbers and volume

### `math::lerp(a: f32, b: f32, t: f32) -> f32`

Linear interpolation: `a` at `t = 0`, `b` at `t = 1`.

```rust
let volume = math::lerp(0.2, 1.0, 0.5);   // 0.6
```

### `math::db_to_linear(db: f32) -> f32`

Decibels to a linear volume factor: 0 dB = 1.0, -6 dB ≈ 0.5, -20 dB = 0.1.

```rust
let s = ctx.audio.play_looped("rain");
s.set_volume(math::db_to_linear(-12.0));
```

### `math::linear_to_db(linear: f32) -> f32`

Linear volume factor to decibels (0.0 is treated as 1e-6, about -120 dB).

```rust
let db = math::linear_to_db(0.5);   // about -6.02
```

## Random (`math::random`, also `random` in the prelude)

### `random::range(min: f32, max: f32) -> f32`

Random float in `min..max`.

```rust
let delay = random::range(1.0, 3.0);
ctx.time.after("growl", delay);
```

### `random::int(min: i64, max: i64) -> i64`

Random integer in `min..=max` (both included).

```rust
let damage = random::int(5, 10);
```

### `random::chance(probability: f32) -> bool`

`true` with the given probability (0.0 never, 1.0 always).

```rust
if random::chance(0.1) {
    ctx.play("critical_hit");
}
```

### `random::pick<T>(items: &[T]) -> Option<&T>`

A random element, `None` for an empty slice.

```rust
let line = random::pick(&["Hello!", "Hi there!", "Welcome back!"]).copied().unwrap_or("Hi");
ctx.speak(line);
```

### `random::seed(seed: u64)`

Seeds the random generator for reproducible results (for tests or daily challenges).

```rust
random::seed(42);
let a = random::int(1, 100);
```
