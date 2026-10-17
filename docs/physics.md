# Physics

`ctx.physics` is a small 3D physics world built for audio games. You can bump into walls,
stand on floors, fall, trigger zones and cast rays to check what is ahead.

There are only boxes and spheres, and bodies never rotate. This keeps behavior predictable
and easy to describe with sound.

This page first explains the ideas (units, body kinds, what happens in a step), then each
part of the API. If you only want to get going, read [Units and directions](#units-and-directions),
[Body kinds](#body-kinds) and [Which setup for my game?](#which-setup-for-my-game).

## Units and directions

| Quantity  | Unit                     | Example                                 |
|-----------|--------------------------|-----------------------------------------|
| position  | meters                   | `vec3(3.0, 10.0, 0.0)`: 3 m east, 10 m north |
| size      | meters                   | `with_box(1.0, 1.0, 1.8)`: a person      |
| velocity  | meters per second        | `vec3(0.0, 2.0, 0.0)`: 2 m/s north (a walk) |
| gravity   | meters per second²       | `EARTH_GRAVITY` = `vec3(0.0, 0.0, -9.81)` |
| mass      | kilograms                | `with_mass(80.0)`                        |
| force     | newtons (kg · m/s²)      | `apply_force(id, vec3(20.0, 0.0, 0.0))`  |
| angle     | compass degrees          | 0 = north, 90 = east, 180 = south, 270 = west |
| time      | seconds                  | the physics step is 1/60 s by default    |

Axes:

- `+x` = east / right
- `+y` = north / forward
- `+z` = up

Positions are written `vec3(x, y, z)`: `vec3(3.0, 10.0, 2.0)` is 3 m east, 10 m north and
2 m up from the origin. Negative numbers go the other way (west, south, down).
[Positions, directions and 3D](3d.md) explains `vec3` in detail, with examples.

A top down game uses x and y and no gravity. A side scroller uses x (left/right) and z
(up/down) with gravity pulling towards `-z`. A full 3D game (flying, swimming) uses all
three, see [Moving in 3D](#moving-in-3d). A position is always the **center** of the
body: a 1.8 m tall box standing on a floor at `z = 0` has its center at `z = 0.9`.

Some handy speeds for orientation: walking 1.5 m/s, running 5 m/s, a car in town 14 m/s,
a fall from 2 m hits the ground at about 6 m/s.

## Body kinds

Every body has one of four kinds. The kind decides what moves the body and what it reacts to.

| Kind | Moved by | Gravity | Blocks others | Is pushed out of walls | Typical use |
|------|----------|---------|---------------|------------------------|-------------|
| `Body::fixed` | nothing, only `set_position` | no | yes | – | walls, floors, furniture |
| `Body::dynamic` | velocity, gravity, forces, collisions | yes | yes | yes, every step | falling things, balls, crates, the player in a platformer |
| `Body::kinematic` | your code: `move_by`, or its velocity | no | yes | only during `move_by` | the player in a top down game, lifts, doors, moving platforms |
| `Body::sensor` | nothing, only `set_position` | no | no | – | trigger zones, pickups, rooms |

```rust
// Walls and floors: never move.
ctx.physics.add(Body::fixed(vec3(0.0, 10.0, 0.0)).with_box(20.0, 1.0, 3.0).with_tag("wall"));

// Moved by velocity and gravity, pushed out of walls.
let crate_id = ctx.physics.add(Body::dynamic(vec3(2.0, 0.0, 5.0)).with_box(1.0, 1.0, 1.0).with_bounciness(0.3));

// Moved only by your code. Ideal for the player in a top down game.
let player = ctx.physics.add(Body::kinematic(Vec3::ZERO).with_sphere(0.4).with_tag("player"));

// Invisible trigger zone.
let door = ctx.physics.add(Body::sensor(vec3(0.0, 9.0, 0.0)).with_box(2.0, 1.0, 3.0).with_tag("door"));
```

Things that surprise people:

- A **kinematic** body that moves by its velocity (`set_velocity`) goes **through** walls.
  That is what you want for a lift, but not for a player. Move a kinematic player with
  `move_by`, `move_forward` or `strafe`, which stop at walls.
- `on_ground`, `jump`, `Landed` and `CollisionStarted` only work for **dynamic** bodies,
  because only they are simulated with gravity and collisions.
- Sensors only notice bodies that are solid (dynamic, kinematic or fixed), not other sensors.
- Sensors and fixed bodies ignore their velocity.

## Which setup for my game?

**Top down game** (walk around a level, turn with the arrow keys, no falling):

- no gravity (the default)
- player: `Body::kinematic(..).with_sphere(0.4)`
- move with `move_forward` / `strafe` / `move_by` and react to `result.blocked` and
  `result.hits`
- walls: `Body::fixed` or a [text map](#levels-from-text-maps)

```rust
let player = ctx.physics.add(Body::kinematic(start).with_sphere(0.4));
ctx.follow(player);
// in update:
if ctx.input.repeating(Key::Up, 0.4, 0.4) && ctx.physics.move_forward(player, 1.0).blocked {
    ctx.play("bump");
}
```

**Side scroller / platformer** (run left and right, jump, fall):

- gravity on: `Engine::new(..).gravity(EARTH_GRAVITY)`
- player: `Body::dynamic(..).with_box(0.8, 0.8, 1.6)`, facing north (the default) so that
  x maps to left and right in the ears
- run by setting the x part of the velocity, jump with `jump`, use `landed` and
  `collided` for sounds

```rust
let player = ctx.physics.add(Body::dynamic(vec3(0.0, 0.0, 1.0)).with_box(0.8, 0.8, 1.6));
// in update:
let mut velocity = ctx.physics.velocity(player);
velocity.x = ctx.input.axis(Key::Left, Key::Right) * 6.0;   // 6 m/s
ctx.physics.set_velocity(player, velocity);
if ctx.pressed(Key::Space) && ctx.physics.jump(player, 1.5) {
    ctx.play("jump");
}
```

`examples/beacon.rs` is a complete top down game, `examples/platformer.rs` a complete
platformer.

## Shapes

```rust
Body::fixed(pos).with_box(20.0, 1.0, 3.0)   // width (x), depth (y), height (z), full sizes
Body::kinematic(pos).with_sphere(0.4)        // radius
```

`with_box` takes **full** sizes, so `with_box(2.0, 2.0, 2.0)` reaches 1 m from the center
in every direction. A body without a shape is a 1 m cube. Boxes always stay aligned with
the axes. A sphere is a good player shape in top down games, because it slides smoothly
around corners.

## Body settings

All settings are optional. They are set with `with_*` when creating a body, or later
through `ctx.physics.get_mut(id)` (the fields have the same names).

| Setting | Default | Meaning | Typical values |
|---------|---------|---------|----------------|
| `with_tag("coin")` | `""` | Free text to recognize the body in events and queries. | `"wall"`, `"enemy"`, `"door"` |
| `with_user_data(n)` | `0` | Free number for your own data. | an index into your enemy list |
| `with_velocity(v)` | zero | Start velocity in m/s. | |
| `with_facing(deg)` | `0.0` (north) | Compass direction the body faces. The shape does **not** rotate. Used by `move_forward`, `strafe`, `describe` and `ctx.follow`. | `0.0`–`360.0` |
| `with_pitch(deg)` | `0.0` (level) | How far the body looks up (positive) or down (negative). Used by `move_forward` and `ctx.follow`. | `-90.0`–`90.0` |
| `with_mass(kg)` | `1.0` | Weight in kilograms. Only matters between two dynamic bodies and for forces. | crate 5–20, person 70–80 |
| `with_bounciness(b)` | `0.0` | How much speed is kept when bouncing off something, from `0.0` (stops dead) to `1.0` (bounces back at full speed). When two bodies touch, the larger value of both counts. | ball 0.6–0.9, crate 0.1–0.3 |
| `with_friction(f)` | `8.0` | How fast the body slows down sideways while it stands on the ground. See the table below. `0.0` is ice. | 0–20 |
| `with_damping(d)` | `0.0` | Like friction, but always and in every direction, also in the air (air or water resistance). Also slows down falling. | air 0.0–0.5, water 2–5 |
| `with_gravity_scale(s)` | `1.0` | Multiplies the world gravity for this body. `0.5` floats, `0.0` ignores gravity, negative values rise. | 0.0–2.0 |
| `with_max_speed(v)` | no limit | Speed limit in m/s, in any direction. Works as a terminal velocity when falling. | 20–50 |

Mass, bounciness, friction, damping, gravity scale and max speed only affect **dynamic**
bodies. Bounciness of a wall still counts when a dynamic body hits it.

### Friction and damping in numbers

Friction and damping are "fraction of speed removed per second". Each physics step the
speed is multiplied by `1 - value * step_time`. At the default 60 steps per second:

| Value | Speed halved after | Down to 10 % after | Feels like |
|-------|--------------------|--------------------|------------|
| 0     | never              | never              | ice, space |
| 1     | 0.7 s              | 2.3 s              | rolling ball |
| 2     | 0.34 s             | 1.1 s              | sliding on a smooth floor |
| 4     | 0.17 s             | 0.56 s             | |
| 8 (default friction) | 0.08 s | 0.27 s         | a person stopping |
| 20    | 0.03 s             | 0.1 s              | stops almost at once |

Friction only works while the body stands on something (`on_ground`) and only on the
sideways part of the velocity, so it never slows down a jump or a fall. If you set the
running speed yourself every frame (like the platformer above), friction still takes a bit
off in each step: with friction 8 a body you set to 6 m/s moves at about 5.2 m/s. Use
`with_friction(0.0)` if you need the exact speed, and set the velocity to zero yourself to
stop.

## Levels from text maps

```rust
const MAP: &str = "
    ##########
    #P...#..C#
    #....#...#
    #..C.....#
    ##########
";

let level = ctx.physics.build_level(MAP, 2.0);   // every character is a 2 x 2 meter tile
let player = ctx.physics.add(Body::kinematic(level.point('P').unwrap()).with_sphere(0.4));
for spot in level.points('C') {
    let coin = ctx.physics.add(Body::sensor(spot).with_sphere(0.5).with_tag("coin"));
    ctx.loop_on(coin, "coin");
}
```

- The second argument is the tile size in meters: each character becomes a square of that
  size.
- `#` becomes a wall (fixed body, tag `"wall"`, 3 m high).
- `.` and spaces are empty floor. No body is created for them; a top down game needs no
  floor because there is no gravity.
- Every other character is a point you can look up with `level.point(c)` (the first one)
  or `level.points(c)` (all of them, from north-west to south-east). The point is the
  center of the tile at `z = 0`.
- The first line is north, the left column is west. The south-west corner of the map is at
  `x = 0, y = 0`, and `level.size()` returns the size in meters. The indentation of the map
  does not matter.

## Facing, turning and walking

Shapes never rotate, but every body has a `facing` (compass degrees), which is ideal for
the player:

```rust
ctx.physics.turn(player, 45.0);                          // 45° to the right, returns the new facing
ctx.physics.turn(player, -90.0);                         // 90° to the left
let result = ctx.physics.move_forward(player, 1.0);      // 1 m ahead, -1.0 walks backwards
ctx.physics.strafe(player, 1.0);                         // 1 m sideways to the right, -1.0 left
ctx.physics.facing(player);                              // 0.0 to 360.0
ctx.speak(ctx.physics.describe(player, beacon));         // "2 o'clock, 8 meters"
ctx.follow(player);                                      // listener uses position and facing
```

`describe` gives the direction as a clock position relative to the facing: 12 o'clock is
straight ahead, 3 o'clock to the right, 6 o'clock behind. If the target is a meter or more
higher or lower, the height is added: `"12 o'clock, 11 meters, 4 meters above"`.

### Moving in 3D

Besides the facing, every body has a `pitch`: how far it looks up or down in degrees
(0 = level, 90 = straight up, -90 = straight down). `move_forward` follows the pitch, so a
drone, a bird or a diver can move in any direction:

```rust
ctx.physics.tilt(drone, 30.0);              // look 30° up (negative = down), returns the new pitch
ctx.physics.move_forward(drone, 1.0);       // 1 m forward and up at the same time
ctx.physics.move_up(drone, 1.0);            // 1 m straight up, -1.0 down (ladders, lifts, flying)
ctx.physics.look_at(drone, bird_pos);       // face a point: sets facing and pitch
ctx.physics.pitch(drone);
```

With the default pitch of 0, `move_forward` stays level, so walking games work as before.
`strafe` is always level. See [Positions, directions and 3D](3d.md) for a complete
overview and `examples/flight.rs` for a game.

## Moving bodies

There are five ways to move a body. Pick by what you want to happen:

| Call | What it does | When it happens | Collisions |
|------|--------------|-----------------|------------|
| `move_by(id, delta)` | moves by `delta` meters | right now | stops at and slides along walls, pushes dynamic bodies, returns a `MoveResult` |
| `set_velocity(id, v)` | sets the speed in m/s | in every following step | dynamic: yes; kinematic: **no**, it passes through walls |
| `apply_impulse(id, v)` | adds `v` m/s to the velocity once (a kick) | from the next step | like `set_velocity` |
| `apply_force(id, f)` | pushes with `f` newtons during the next step only | next step | like `set_velocity` |
| `set_position(id, pos)` | teleports | right now | none, the body may end up inside a wall |

### `move_by`: the character controller

`move_by` moves a body immediately. It slides along walls and tells you what happened:

```rust
let result = ctx.physics.move_by(player, math::forward(yaw) * 1.0);
if result.blocked {
    ctx.audio.play("bump");
} else {
    ctx.audio.play("step");
}
for hit in &result.hits {
    if ctx.physics.tag(*hit) == "door" { /* ... */ }
}
```

The `MoveResult` contains:

- `moved`: how far the body really moved. Shorter than `delta` when a wall was in the way.
- `blocked`: `true` if something stopped part of the movement. Walking diagonally into a
  wall still slides along it, but `blocked` is `true`.
- `hits`: every solid body touched on the way, in order.

`move_by` does not create `CollisionStarted` events; use `hits` instead. It does update
sensors, so `entered` and `left` work right away. Big moves are split into small pieces,
so even a 10 m jump does not pass through a thin wall.

### Velocity, impulses and forces

```rust
ctx.physics.set_velocity(player, vec3(5.0, 0.0, 0.0));   // 5 m/s east from now on
ctx.physics.apply_impulse(ball, vec3(0.0, 10.0, 0.0));   // instant kick: +10 m/s north
ctx.physics.apply_force(boat, vec3(20.0, 0.0, 0.0));     // 20 N east, for one step
```

- An **impulse** changes the velocity directly, no matter how heavy the body is. Use it for
  kicks, explosions and knockback.
- A **force** respects mass: each second it adds `force / mass` m/s. 20 N on a 1 kg body
  adds 20 m/s per second, on a 10 kg body only 2 m/s per second. A force only lasts for one
  step, so call `apply_force` every frame for wind, currents or engines.

## Gravity

There is no gravity by default, which suits top down games. Turn it on when the engine
starts or at any time later:

```rust
Engine::new("My game").gravity(EARTH_GRAVITY).run(...)   // 9.81 m/s² down
ctx.physics.set_gravity(EARTH_GRAVITY * 2.0);           // snappier platformer
ctx.physics.set_gravity(vec3(0.0, 0.0, -1.6));          // moon
```

Gravity is an acceleration: every second a falling body gets 9.81 m/s faster. Real gravity
feels slow and floaty in games, so many platformers use 2 to 3 times `EARTH_GRAVITY`.

Gravity only pulls **dynamic** bodies. `with_gravity_scale` changes it per body and
`with_max_speed` sets a terminal velocity.

## Jumping, falling and landing

```rust
// Jump exactly 1.5 meters high. Returns false while in the air (no double jumps).
if ctx.input.pressed(Key::Space) && ctx.physics.jump(player, 1.5) {
    ctx.audio.play("jump");
}

// Landing sounds and fall damage.
if let Some(speed) = ctx.physics.landed(player) {
    ctx.audio.sound("land").volume((speed / 12.0).min(1.0)).play();
    if speed > 15.0 {
        ctx.speech.speak("Ouch!");
    }
}

ctx.physics.on_ground(player);
ctx.physics.get(player).unwrap().ground;   // Some(body you stand on)
```

`jump(id, height)` takes the height in **meters**, not a speed. It computes the needed
upward speed from the current gravity, so a jump stays 1.5 m high even if you change
gravity. It only works for a dynamic body that stands on something and when there is
gravity; otherwise it returns `false` and does nothing. Damping makes the jump a bit lower.

How long a jump lasts (up and down again):

| Height | `EARTH_GRAVITY` | `EARTH_GRAVITY * 2.0` |
|--------|-----------------|-----------------------|
| 0.5 m  | 0.64 s          | 0.45 s                |
| 1.0 m  | 0.90 s          | 0.64 s                |
| 1.5 m  | 1.11 s          | 0.78 s                |
| 2.5 m  | 1.43 s          | 1.01 s                |

The `speed` of a landing is the falling speed at impact in m/s. With `EARTH_GRAVITY`, a fall
from 1 m lands at about 4.4 m/s, from 3 m at 7.7 m/s and from 10 m at 14 m/s. Dividing by
roughly 12 gives a good volume for landing sounds.

`on_ground` is `true` when the body touched something below it in the last step.

## Mass, pushing and bouncing

- Dynamic bodies hitting each other exchange momentum like billiard balls. A heavy body
  hitting a light one keeps most of its speed and sends the light one flying; a light body
  bounces off a heavy one.
- When two dynamic bodies overlap, they are pushed apart in proportion to their masses.
- A kinematic player moved with `move_by` **pushes dynamic bodies** (crates, balls),
  regardless of mass. If the crate is stuck against a wall, the player is blocked
  (`result.blocked`).
- Dynamic bodies standing on a moving **kinematic** body (a lift, a moving platform, a
  conveyor) are carried along:

```rust
let lift = ctx.physics.add(Body::kinematic(vec3(10.0, 0.0, 0.0)).with_box(3.0, 3.0, 0.5));
ctx.physics.set_velocity(lift, vec3(0.0, 0.0, 2.0));   // goes up at 2 m/s, and so does whoever stands on it
```

## What happens in one step

The world steps automatically at a fixed rate (`Engine::physics_rate`, 60 steps per second
by default). If a frame takes longer, several steps (at most 8) run in that frame, so the
simulation speed does not depend on the frame rate. A higher rate is more precise and costs
more CPU; 60 is plenty for most games. Your scene's `fixed_update` runs right after each
step, `update` once per frame after all steps.

In each step:

1. Kinematic bodies move by their velocity (without collisions).
2. Every dynamic body, one after the other:
   1. is carried along by the kinematic body it stands on,
   2. gets faster by gravity and forces,
   3. is slowed down by damping, and by friction if it stands on the ground,
   4. is limited to `max_speed`,
   5. moves by its velocity, in small pieces so it does not pass through thin walls, and is
      pushed out of everything it overlaps. Hitting something removes the speed going into
      it (or reverses part of it, by bounciness) and creates the events below.
3. Sensors compare who is inside now with the step before and report `Entered` and `Left`.

## Events

Events tell you what happened during the physics steps of the **current frame**. They are
cleared before the next frame's steps, so read them in `update` every frame.

| Event | When | Fields |
|-------|------|--------|
| `CollisionStarted { a, b, speed }` | the dynamic body `a` hit `b` and they were not touching before | `speed`: how fast they came together in m/s |
| `CollisionEnded { a, b }` | they stopped touching | |
| `Landed { body, on, speed }` | the dynamic `body` touched ground after being in the air | `speed`: falling speed in m/s |
| `Entered { sensor, body }` | a solid `body` came into `sensor` | |
| `Left { sensor, body }` | it went out again | |
| `Crashed { vehicle, other, speed }` | a vehicle drove into `other` | `speed`: speed lost in m/s |
| `GearChanged { vehicle, gear }` | a vehicle changed gear | `gear`: -1 reverse, 0 neutral, 1 and up |

`speed` in `CollisionStarted` only counts the part of the movement that goes into the other
body: running along a wall gives a low speed, running straight into it a high one. It is
the right value for the volume of impact sounds.

Usually you only care about one body. There are shortcuts that return the first matching
event of this frame:

```rust
if let Some(speed) = ctx.physics.landed(player) { /* ... */ }
if let Some((other, speed)) = ctx.physics.collided(player) { /* ... */ }
if let Some(coin) = ctx.physics.entered_tag(player, "coin") { /* ... */ }
if let Some(zone) = ctx.physics.entered(player) { /* any sensor */ }
if let Some(zone) = ctx.physics.left(player) { /* ... */ }
if let Some(who) = ctx.physics.triggered(trap) { /* something entered the trap */ }
```

To see all events, loop over them:

```rust
for event in ctx.physics.events().to_vec() {
    match event {
        PhysicsEvent::CollisionStarted { a, b, speed } => {
            ctx.audio.sound("thud").volume((speed / 10.0).min(1.0)).at(ctx.physics.position(a)).play();
        }
        PhysicsEvent::Entered { sensor, body } => { /* body walked into a sensor */ }
        _ => {}
    }
}
```

`.to_vec()` copies the list, so you can change `ctx` (play sounds, remove bodies) inside the
loop. `event.involves(id)` and `event.other(id)` help filter events.

## Queries

```rust
// What is in front of me? Great for "sonar" or auto describing walls.
if let Some(hit) = ctx.physics.raycast(pos, math::forward(yaw), 20.0, Some(player)) {
    let what = ctx.physics.tag(hit.body);
    ctx.speech.speak(format!("{what} in {:.0} meters", hit.distance));
}

ctx.physics.in_radius(pos, 5.0);              // bodies whose center is within 5 m, nearest first
ctx.physics.nearest_with_tag(pos, "enemy");
ctx.physics.at_point(pos);                    // bodies whose shape contains the point
ctx.physics.with_tag("coin").count();
ctx.physics.overlaps(a, b);
ctx.physics.sensors_of(player);               // which zones am I in?
ctx.physics.distance(a, b);                   // meters between the centers
```

`raycast(origin, direction, max_distance, ignore)` sends a straight line from `origin` in
`direction` and returns the first solid body within `max_distance` meters. Pass the caster
as `ignore`, otherwise the ray hits the player itself. Sensors are never hit. The
`RayHit` has the `body`, the `point` where the ray hit, the `distance` in meters and the
`normal`, the direction the hit surface faces (for example `-y` for the south side of a
wall).

## Accessing bodies

```rust
ctx.physics.position(id);
ctx.physics.set_position(id, pos);   // teleport
ctx.physics.velocity(id);
ctx.physics.get(id);                 // Option<&Body>
ctx.physics.get_mut(id);             // change any setting, e.g. .unwrap().friction = 0.0
ctx.physics.remove(id);              // attached sounds stop by themselves
ctx.physics.clear();
```

Calls with an id that no longer exists do nothing and return a neutral value (zero
position, empty tag, `false`).

## Cars and trucks

A body can optionally be a vehicle with weight, speed, gears, brakes and fuel:
`ctx.physics.add(Body::car(pos))`. See [Cars and trucks](vehicles.md).

## Limits

- Collisions are resolved one body at a time. Big piles of stacked dynamic bodies may jitter.
- Fast bodies are moved in small pieces so they do not tunnel through thin walls, up to 64
  pieces per step. Something faster than about 64 times its own radius per step (about
  1900 m/s for a 0.5 m body) can still pass through.
- If you need rotation, slopes, joints or many thousands of bodies, use a full physics
  engine such as rapier together with audio-code.

How it works inside is described in [Architecture](architecture.md#physics-physicsrs).
