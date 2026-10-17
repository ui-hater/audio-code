# Physics

Simple, predictable 3D physics: `ctx.physics` (type `World`). Two shapes (axis aligned
boxes and spheres), no rotation, deterministic order. The engine steps the world
automatically at `Engine::physics_rate` (60 Hz by default); you add bodies, move them and
react to events.

```rust
use audio_code::prelude::*;   // World, Body, BodyId, BodyKind, Shape, PhysicsEvent, RayHit, MoveResult, EARTH_GRAVITY
```

Axes: `+x` east, `+y` north, `+z` up. Positions are body centers in meters.

## Constants

### `EARTH_GRAVITY: Vec3`

`vec3(0.0, 0.0, -9.81)`: earth gravity, pointing down.

```rust
ctx.physics.set_gravity(EARTH_GRAVITY);
```

## Body kinds

| Kind | Constructor | Moved by | Gravity | Blocks others |
|------|-------------|----------|---------|---------------|
| `Dynamic` | `Body::dynamic` | velocity, gravity, forces, collisions | yes | yes |
| `Fixed` | `Body::fixed` | nothing (only `set_position`) | no | yes |
| `Kinematic` | `Body::kinematic` | your code (`move_by`, ...) or its velocity, which ignores walls | no | yes, and pushes dynamic bodies |
| `Sensor` | `Body::sensor` | nothing (only `set_position`) | no | no, only reports `Entered` / `Left` |

Typical use: a top down player is kinematic and moved with `move_by` / `move_forward`. A
platformer player is dynamic, moved with velocity and `jump`. Walls and floors are fixed.
Lifts are kinematic with a velocity. Coins, doors and room triggers are sensors.

## Body

A `Body` is built with a constructor plus chained `with_*` methods, then added with
`ctx.physics.add(body)`, which returns its `BodyId`. All fields are public, so after
adding you can also change them through `ctx.physics.get_mut(id)`.

Defaults: shape 1 m cube (`Shape::Box(vec3(0.5, 0.5, 0.5))`), velocity zero, gravity
scale 1.0, bounciness 0.0, friction 8.0, damping 0.0, mass 1.0, no speed limit, facing 0,
pitch 0, empty tag, user data 0.

### `Body::dynamic(position: Vec3) -> Body`

A body moved by velocity, gravity, forces and collisions: a ball, a crate, a platformer
player. Only dynamic bodies have `on_ground`, can `jump` and create collision and landing
events.

```rust
let ball = ctx.physics.add(Body::dynamic(vec3(0.0, 0.0, 5.0)).with_sphere(0.2).with_bounciness(0.7));
```

### `Body::fixed(position: Vec3) -> Body`

An immovable body: walls, floors, obstacles.

```rust
ctx.physics.add(Body::fixed(vec3(0.0, 0.0, -0.5)).with_box(100.0, 100.0, 1.0).with_tag("floor"));
```

### `Body::kinematic(position: Vec3) -> Body`

A body moved only by your code. Its velocity is applied without collision checks (right for
lifts and platforms); move a kinematic player with `move_by`, which stops at walls.

```rust
let player = ctx.physics.add(Body::kinematic(vec3(2.0, 2.0, 0.0)).with_sphere(0.4).with_tag("player"));
let lift = ctx.physics.add(
    Body::kinematic(vec3(10.0, 0.0, 0.0)).with_box(2.0, 2.0, 0.2).with_velocity(vec3(0.0, 0.0, 1.0)),
);
```

### `Body::sensor(position: Vec3) -> Body`

An invisible trigger area that blocks nothing and only reports when solid bodies enter or
leave it.

```rust
let exit = ctx.physics.add(Body::sensor(vec3(20.0, 8.0, 0.0)).with_box(2.0, 2.0, 3.0).with_tag("exit"));
```

### `body.with_box(width: f32, depth: f32, height: f32) -> Body`

Box shape with full width (x, east-west), depth (y, north-south) and height (z) in meters.
`with_box(2.0, 2.0, 2.0)` reaches 1 m from the center in every direction.

```rust
let crate_box = Body::dynamic(vec3(3.0, 0.0, 0.5)).with_box(1.0, 1.0, 1.0);
```

### `body.with_sphere(radius: f32) -> Body`

Sphere shape with a radius in meters. 0.3 to 0.5 is a good player size.

```rust
let player = Body::kinematic(Vec3::ZERO).with_sphere(0.4);
```

### `body.with_tag(tag: impl Into<String>) -> Body`

Free text to recognize the body in events and queries, such as `"wall"` or `"coin"`.

```rust
let coin = Body::sensor(vec3(4.0, 4.0, 0.0)).with_tag("coin");
```

### `body.with_velocity(velocity: Vec3) -> Body`

Initial velocity in m/s.

```rust
let arrow = Body::dynamic(vec3(0.0, 0.0, 1.5)).with_sphere(0.05).with_velocity(vec3(0.0, 30.0, 2.0));
```

### `body.with_bounciness(bounciness: f32) -> Body`

How much speed is kept when bouncing, 0.0 (stops dead, default) to 1.0 (full speed back).
A ball is about 0.6 to 0.9, a crate 0.1 to 0.3. When two bodies touch, the larger value
counts.

```rust
let ball = Body::dynamic(vec3(0.0, 0.0, 3.0)).with_sphere(0.15).with_bounciness(0.8);
```

### `body.with_friction(friction: f32) -> Body`

Fraction of the sideways speed removed per second while on the ground. Default 8.0 (stops
within about 0.3 s), 2.0 slides about a second, 0.0 is ice. Never slows jumps or falls.

```rust
let skater = Body::dynamic(Vec3::ZERO).with_friction(0.5);
```

### `body.with_damping(damping: f32) -> Body`

Fraction of the speed removed per second, always and in every direction (air or water
resistance). Default 0.0; 0.1 to 0.5 light air drag, 2 to 5 water.

```rust
let diver = Body::dynamic(Vec3::ZERO).with_damping(3.0).with_gravity_scale(0.2);
```

### `body.with_gravity_scale(scale: f32) -> Body`

Multiplier for world gravity: 1.0 normal, 0.5 floats, 0.0 no gravity, negative rises.

```rust
let balloon = Body::dynamic(Vec3::ZERO).with_gravity_scale(-0.2);
```

### `body.with_mass(mass: f32) -> Body`

Mass in kilograms, default 1.0 (minimum 0.001). Heavier bodies push lighter ones further,
lose less speed in collisions and are moved less by forces.

```rust
let boulder = Body::dynamic(vec3(0.0, 5.0, 1.0)).with_sphere(1.0).with_mass(500.0);
```

### `body.with_max_speed(max_speed: f32) -> Body`

Speed limit in m/s, for example a terminal velocity. Default unlimited.

```rust
let player = Body::dynamic(Vec3::ZERO).with_max_speed(20.0);
```

### `body.with_facing(degrees: f32) -> Body`

Compass direction the body faces (0 north, 90 east). The shape does not rotate. Used by
`move_forward`, `strafe`, `turn`, `describe` and the listener when the body is followed.

```rust
let player = Body::kinematic(Vec3::ZERO).with_facing(90.0);   // facing east
```

### `body.with_pitch(degrees: f32) -> Body`

How far the body looks up (positive) or down (negative), clamped to -90..=90. Used by
`move_forward` and the listener.

```rust
let plane = Body::kinematic(vec3(0.0, 0.0, 100.0)).with_pitch(10.0);
```

### `body.with_user_data(data: u64) -> Body`

A free number for your own data, such as an index into your enemy list.

```rust
let enemy = Body::kinematic(vec3(5.0, 5.0, 0.0)).with_tag("enemy").with_user_data(3);
```

### `body.contains(point: Vec3) -> bool`

Whether the body's shape contains the point.

```rust
let room = Body::sensor(Vec3::ZERO).with_box(4.0, 4.0, 3.0);
assert!(room.contains(vec3(1.0, 1.0, 0.0)));
```

### Body fields

| Field | Type | Meaning |
|-------|------|---------|
| `position` | `Vec3` | center in meters |
| `velocity` | `Vec3` | m/s (ignored for fixed bodies and sensors) |
| `shape` | `Shape` | collision shape |
| `kind` | `BodyKind` | how it is simulated |
| `gravity_scale` | `f32` | gravity multiplier (dynamic only) |
| `bounciness` | `f32` | 0.0..=1.0 |
| `friction` | `f32` | ground friction per second (dynamic only) |
| `damping` | `f32` | air or water drag per second (dynamic only) |
| `tag` | `String` | free text |
| `user_data` | `u64` | free number |
| `facing` | `f32` | compass degrees |
| `pitch` | `f32` | degrees up |
| `mass` | `f32` | kilograms |
| `max_speed` | `f32` | m/s, `f32::INFINITY` = none |
| `on_ground` | `bool` | touched ground in the last step (dynamic only, read only in practice) |
| `ground` | `Option<BodyId>` | what it stands on; moving kinematic grounds carry it |

```rust
if let Some(body) = ctx.physics.get_mut(player) {
    body.friction = 0.0;          // walked onto ice
    body.tag = "frozen".into();
}
```

## Shape, BodyKind, BodyId

```rust
pub enum Shape {
    Box(Vec3),     // HALF size along x, y and z
    Sphere(f32),   // radius
}

pub enum BodyKind { Dynamic, Fixed, Kinematic, Sensor }
```

`BodyId` is a small `Copy` handle (`Eq`, `Hash`, `Ord`) returned by `add`. It stays valid
until the body is removed.

```rust
let body = ctx.physics.get(player);
if let Some(b) = body {
    if let Shape::Sphere(radius) = b.shape {
        println!("radius {radius}");
    }
    let is_player_kinematic = b.kind == BodyKind::Kinematic;
}
```

## World: bodies

### `World::new() -> World`

An empty world without gravity. The engine creates one for you as `ctx.physics`; you need
this only for tests or tools.

```rust
let mut world = World::new();
let id = world.add(Body::fixed(Vec3::ZERO));
```

### `ctx.physics.add(body: Body) -> BodyId`

Adds a body and returns its id.

```rust
let wall = ctx.physics.add(Body::fixed(vec3(5.0, 0.0, 1.5)).with_box(1.0, 10.0, 3.0).with_tag("wall"));
```

### `ctx.physics.remove(id: BodyId) -> Option<Body>`

Removes a body and returns it if it existed. Sounds attached with `ctx.loop_on` /
`ctx.attach` stop by themselves.

```rust
if let Some(coin) = ctx.physics.entered_tag(player, "coin") {
    ctx.physics.remove(coin);
    ctx.play("pickup");
}
```

### `ctx.physics.clear()`

Removes every body.

```rust
ctx.physics.clear();
```

### `ctx.physics.get(id: BodyId) -> Option<&Body>`

Reads a body.

```rust
if let Some(b) = ctx.physics.get(player) {
    let height = b.position.z;
}
```

### `ctx.physics.get_mut(id: BodyId) -> Option<&mut Body>`

Changes a body.

```rust
if let Some(b) = ctx.physics.get_mut(enemy) {
    b.max_speed = 2.0;
}
```

### `ctx.physics.bodies() -> impl Iterator<Item = (BodyId, &Body)>`

All bodies in creation order.

```rust
let dynamic_count = ctx.physics.bodies().filter(|(_, b)| b.kind == BodyKind::Dynamic).count();
```

### `ctx.physics.with_tag(tag: &str) -> impl Iterator<Item = BodyId>`

All bodies with a tag.

```rust
let coins_left = ctx.physics.with_tag("coin").count();
```

### `ctx.physics.tag(id: BodyId) -> &str`

The tag of a body, `""` if it does not exist.

```rust
if let Some((other, _)) = ctx.physics.collided(player) {
    if ctx.physics.tag(other) == "wall" {
        ctx.play("bump");
    }
}
```

### `ctx.physics.len() -> usize`

Number of bodies.

```rust
let count = ctx.physics.len();
```

### `ctx.physics.is_empty() -> bool`

Whether the world has no bodies.

```rust
if ctx.physics.is_empty() {
    ctx.speak("Empty world");
}
```

## World: position, velocity and forces

### `ctx.physics.position(id: BodyId) -> Vec3`

Position of a body (`Vec3::ZERO` if it does not exist).

```rust
let at = ctx.physics.position(enemy);
ctx.play_at("growl", at);
```

### `ctx.physics.set_position(id: BodyId, position: Vec3)`

Teleports a body without collision checks.

```rust
ctx.physics.set_position(player, vec3(2.0, 2.0, 0.0));   // respawn
```

### `ctx.physics.velocity(id: BodyId) -> Vec3`

Velocity of a body (`Vec3::ZERO` if it does not exist).

```rust
let falling_fast = ctx.physics.velocity(player).z < -8.0;
```

### `ctx.physics.set_velocity(id: BodyId, velocity: Vec3)`

Sets the velocity. For kinematic bodies (lifts) it moves them without collisions; for
dynamic bodies it is integrated with gravity and collisions.

```rust
ctx.physics.set_velocity(door, vec3(0.0, 0.0, 0.5));   // door slides up
```

### `ctx.physics.apply_impulse(id: BodyId, impulse: Vec3)`

Adds `impulse` (m/s) to the velocity at once, regardless of mass: kicks, knockback.

```rust
ctx.physics.apply_impulse(enemy, vec3(0.0, 8.0, 3.0));
```

### `ctx.physics.apply_force(id: BodyId, force: Vec3)`

Pushes a body with a force in newtons during the next physics step: each second adds
`force / mass` m/s. Call it every frame (or in `fixed_update`) for a continuous force such
as wind or a conveyor belt.

```rust
ctx.physics.apply_force(player, vec3(5.0, 0.0, 0.0));   // wind from the west
```

### `ctx.physics.jump(id: BodyId, height: f32) -> bool`

Makes a dynamic body standing on the ground jump so it reaches `height` meters above where
it stands, taking gravity into account. Returns `false` and does nothing while in the air,
without gravity, or for other kinds.

```rust
if ctx.pressed(Key::Space) && ctx.physics.jump(player, 1.5) {
    ctx.play("jump");
}
```

### `ctx.physics.on_ground(id: BodyId) -> bool`

Whether a dynamic body stands on something (always `false` for other kinds).

```rust
if !ctx.physics.on_ground(player) {
    ctx.speak("In the air");
}
```

### `ctx.physics.set_gravity(gravity: Vec3)`

Sets gravity in m/s². Default none.

```rust
ctx.physics.set_gravity(EARTH_GRAVITY);
```

### `ctx.physics.gravity() -> Vec3`

Current gravity.

```rust
let g = ctx.physics.gravity().length();
```

### `ctx.physics.up() -> Vec3`

Unit vector pointing away from gravity (`+z` when there is no gravity).

```rust
let up = ctx.physics.up();
```

### `ctx.physics.step(dt: f32)`

Advances the simulation by `dt` seconds. The engine calls this; use it only with a
`World` you own (tests, tools).

```rust
let mut world = World::new();
world.set_gravity(EARTH_GRAVITY);
let rock = world.add(Body::dynamic(vec3(0.0, 0.0, 10.0)));
world.step(1.0 / 60.0);
```

## World: moving like a character

These move a body right now, slide along walls and stop at solid bodies, push dynamic
bodies out of the way, and return a `MoveResult`. They update sensor events at once but do
not create `CollisionStarted` (use `MoveResult::hits`).

### `ctx.physics.move_by(id: BodyId, delta: Vec3) -> MoveResult`

Moves a body by `delta` meters.

```rust
let step = ctx.input.movement() * 3.0 * ctx.time.delta();
let result = ctx.physics.move_by(player, vec3(step.x, step.y, 0.0));
if result.blocked && ctx.once("wall_hint") {
    ctx.speak("A wall blocks your way");
}
```

### `ctx.physics.move_forward(id: BodyId, distance: f32) -> MoveResult`

Moves `distance` meters in the direction the body faces (negative = backwards). The pitch
counts too: a body looking 30° up also rises.

```rust
if ctx.input.repeating(Key::Up, 0.3, 0.3) {
    if ctx.physics.move_forward(player, 1.0).blocked {
        ctx.play("bump");
    } else {
        ctx.play("step");
    }
}
```

### `ctx.physics.strafe(id: BodyId, distance: f32) -> MoveResult`

Moves sideways, `distance` meters to the body's right (negative = left). Level, ignores
pitch.

```rust
if ctx.held(Key::D) {
    ctx.physics.strafe(player, 2.0 * ctx.time.delta());
}
```

### `ctx.physics.move_up(id: BodyId, distance: f32) -> MoveResult`

Moves straight up (negative = down): flying, swimming, ladders. Up is `+z`, or against
gravity if gravity points elsewhere.

```rust
if ctx.held(Key::PageUp) {
    ctx.physics.move_up(player, 2.0 * ctx.time.delta());
}
```

### `MoveResult`

```rust
pub struct MoveResult {
    pub moved: Vec3,          // how far the body actually moved
    pub blocked: bool,        // something stopped part of the movement (also when sliding)
    pub hits: Vec<BodyId>,    // solid bodies touched on the way, in order
}
```

```rust
let result = ctx.physics.move_forward(player, 1.0);
for hit in &result.hits {
    if ctx.physics.tag(*hit) == "door" {
        ctx.speak("A door");
    }
}
```

## World: facing and pitch

### `ctx.physics.facing(id: BodyId) -> f32`

Compass direction the body faces (0 if it does not exist).

```rust
let heading = ctx.physics.facing(player);
ctx.speak(math::direction_name(heading));
```

### `ctx.physics.set_facing(id: BodyId, degrees: f32)`

Sets the facing (wrapped into 0..360).

```rust
ctx.physics.set_facing(player, 180.0);   // face south
```

### `ctx.physics.turn(id: BodyId, degrees: f32) -> f32`

Turns by `degrees` (positive right, negative left) and returns the new facing in 0..360.

```rust
if ctx.pressed(Key::Right) {
    let now = ctx.physics.turn(player, 45.0);
    ctx.speak(math::direction_name(now));
}
```

### `ctx.physics.pitch(id: BodyId) -> f32`

How far the body looks up (positive) or down (negative), in degrees.

```rust
let looking_down = ctx.physics.pitch(player) < 0.0;
```

### `ctx.physics.set_pitch(id: BodyId, degrees: f32)`

Sets the pitch, clamped to -90..=90.

```rust
ctx.physics.set_pitch(player, 0.0);   // look level again
```

### `ctx.physics.tilt(id: BodyId, degrees: f32) -> f32`

Tilts the view up (positive) or down (negative) and returns the new pitch.

```rust
if ctx.held(Key::Up) {
    ctx.physics.tilt(player, 30.0 * ctx.time.delta());
}
```

### `ctx.physics.look_at(id: BodyId, point: Vec3)`

Turns the body to face `point`: sets facing and pitch.

```rust
let target = ctx.physics.position(enemy);
ctx.physics.look_at(player, target);
```

## World: queries

### `ctx.physics.distance(a: BodyId, b: BodyId) -> f32`

Distance between two bodies' centers (infinity if one does not exist).

```rust
if ctx.physics.distance(player, enemy) < 2.0 {
    ctx.speak("The enemy is right next to you");
}
```

### `ctx.physics.describe(from: BodyId, target: BodyId) -> String`

Where `target` is from `from`'s point of view, ready to speak, using `from`'s facing:
`"3 o'clock, 12 meters"`, `"12 o'clock, 11 meters, 4 meters above"`,
`"directly below, 5 meters"`.

```rust
if ctx.pressed(Key::F) {
    let text = ctx.physics.describe(player, enemy);
    ctx.speak(text);
}
```

### `ctx.physics.raycast(origin: Vec3, direction: Vec3, max_distance: f32, ignore: Option<BodyId>) -> Option<RayHit>`

Sends a straight line from `origin` in `direction` (any length) and returns the first solid
body hit within `max_distance` meters. Pass the caster as `ignore`, otherwise the ray hits
it first. Sensors are ignored.

```rust
let from = ctx.physics.position(player);
let ahead = math::forward(ctx.physics.facing(player));
match ctx.physics.raycast(from, ahead, 20.0, Some(player)) {
    Some(hit) => {
        let text = format!("{} in {:.0} meters", ctx.physics.tag(hit.body), hit.distance);
        ctx.speak(text);
    }
    None => ctx.speak("Nothing ahead"),
}
```

### `RayHit`

```rust
pub struct RayHit {
    pub body: BodyId,     // the body hit
    pub point: Vec3,      // where the ray hit
    pub distance: f32,    // meters from the origin
    pub normal: Vec3,     // direction the hit surface faces, length 1
}
```

### `ctx.physics.at_point(point: Vec3) -> Vec<BodyId>`

Bodies whose shape contains `point` (including sensors).

```rust
let here = ctx.physics.position(player);
let inside_water = ctx.physics.at_point(here).iter().any(|&id| ctx.physics.tag(id) == "water");
```

### `ctx.physics.in_radius(center: Vec3, radius: f32) -> Vec<BodyId>`

Bodies whose center lies within `radius` meters, nearest first.

```rust
let center = ctx.physics.position(player);
for id in ctx.physics.in_radius(center, 5.0) {
    if ctx.physics.tag(id) == "enemy" {
        ctx.physics.apply_impulse(id, vec3(0.0, 0.0, 5.0));   // shock wave
    }
}
```

### `ctx.physics.nearest_with_tag(center: Vec3, tag: &str) -> Option<BodyId>`

The nearest body with the given tag.

```rust
let here = ctx.physics.position(player);
if let Some(coin) = ctx.physics.nearest_with_tag(here, "coin") {
    let text = ctx.physics.describe(player, coin);
    ctx.speak(text);
}
```

### `ctx.physics.overlaps(a: BodyId, b: BodyId) -> bool`

Whether two bodies overlap right now.

```rust
if ctx.physics.overlaps(player, sensor) {
    ctx.speak("Inside the zone");
}
```

### `ctx.physics.sensors_of(body: BodyId) -> Vec<BodyId>`

Sensors the body is inside right now.

```rust
let rooms: Vec<String> = ctx.physics.sensors_of(player).iter().map(|&s| ctx.physics.tag(s).to_string()).collect();
```

## World: events

Events from the physics steps of the current frame. They are cleared at the start of each
frame's physics, so read them in `update` or `fixed_update` of the same frame.

### `ctx.physics.events() -> &[PhysicsEvent]`

All events of this frame.

```rust
let events = ctx.physics.events().to_vec();
for event in events {
    if let PhysicsEvent::CollisionStarted { speed, .. } = event {
        ctx.audio.sound("thud").volume((speed / 10.0).min(1.0)).play();
    }
}
```

### `ctx.physics.landed(body: BodyId) -> Option<f32>`

If `body` landed on something this frame: the falling speed in m/s. With earth gravity a
1 m fall lands at about 4.4 m/s, a 3 m fall at 7.7 m/s.

```rust
if let Some(speed) = ctx.physics.landed(player) {
    ctx.play("land");
    if speed > 10.0 {
        ctx.speak("Ouch");
    }
}
```

### `ctx.physics.collided(body: BodyId) -> Option<(BodyId, f32)>`

If `body` started touching another solid body this frame: that body and the impact speed.
Only from physics steps of dynamic bodies, not from `move_by`.

```rust
if let Some((other, speed)) = ctx.physics.collided(player) {
    let at = ctx.physics.position(other);
    ctx.audio.sound("bump").at(at).volume((speed / 8.0).min(1.0)).play();
}
```

### `ctx.physics.entered(body: BodyId) -> Option<BodyId>`

If `body` walked into a sensor this frame: the sensor.

```rust
if let Some(zone) = ctx.physics.entered(player) {
    let text = format!("Entering {}", ctx.physics.tag(zone));
    ctx.speak(text);
}
```

### `ctx.physics.entered_tag(body: BodyId, tag: &str) -> Option<BodyId>`

If `body` walked into a sensor with this tag this frame: the sensor.

```rust
if let Some(coin) = ctx.physics.entered_tag(player, "coin") {
    ctx.physics.remove(coin);
    ctx.play("pickup");
}
```

### `ctx.physics.left(body: BodyId) -> Option<BodyId>`

If `body` left a sensor this frame: the sensor.

```rust
if ctx.physics.left(player).is_some() {
    ctx.play("door_close");
}
```

### `ctx.physics.triggered(sensor: BodyId) -> Option<BodyId>`

If any body entered `sensor` this frame: that body.

```rust
if let Some(who) = ctx.physics.triggered(sensor) {
    if ctx.physics.tag(who) == "enemy" {
        ctx.play("alarm");
    }
}
```

### `PhysicsEvent`

```rust
pub enum PhysicsEvent {
    CollisionStarted { a: BodyId, b: BodyId, speed: f32 },  // dynamic `a` hit `b`; impact speed m/s
    CollisionEnded { a: BodyId, b: BodyId },                // stopped touching
    Landed { body: BodyId, on: BodyId, speed: f32 },        // dynamic body touched ground after air
    Entered { sensor: BodyId, body: BodyId },               // solid body entered a sensor
    Left { sensor: BodyId, body: BodyId },                  // body left a sensor
}
```

### `event.involves(id: BodyId) -> bool`

Whether the event involves this body.

```rust
let player_events = ctx.physics.events().iter().filter(|e| e.involves(player)).count();
```

### `event.other(id: BodyId) -> Option<BodyId>`

For an event involving `id`, the other body.

```rust
for event in ctx.physics.events() {
    if let Some(other) = event.other(player) {
        println!("{event:?} with {other:?}");
    }
}
```

Building walls from a text map (`ctx.physics.build_level`) is on [level.md](level.md).
