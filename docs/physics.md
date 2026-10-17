# Physics

`ctx.physics` is a small 3D physics world built for audio games. You can bump into walls,
stand on floors, fall, trigger zones and cast rays to check what is ahead.

There are only boxes and spheres, and bodies never rotate. This keeps behavior predictable
and easy to describe with sound.

## Coordinate system

- `+x` = east / right, `+y` = north / forward, `+z` = up
- Angles are compass degrees: 0 = north, 90 = east

A top down game uses x and y and no gravity. A side scroller uses x and z with gravity
`EARTH_GRAVITY` (`vec3(0.0, 0.0, -9.81)`).

## Bodies

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

`with_box(width, depth, height)` takes full sizes. Other settings are `with_velocity`,
`with_friction` (ground damping), `with_damping` (air), `with_gravity_scale`, and
`with_user_data` (your own number, e.g. an index into your enemy list).

## Moving the player: `move_by`

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

## Gravity

There is no gravity by default, which suits top down games. Turn it on when the engine
starts or at any time later:

```rust
Engine::new("My game").gravity(EARTH_GRAVITY).run(...)   // 9.81 m/s² down
ctx.physics.set_gravity(EARTH_GRAVITY * 2.0);           // snappier platformer
ctx.physics.set_gravity(vec3(0.0, 0.0, -1.6));          // moon
```

Gravity only pulls **dynamic** bodies. `with_gravity_scale(0.5)` makes a body float,
`with_gravity_scale(0.0)` turns gravity off for it, and a negative value makes it rise.
`with_max_speed(30.0)` sets a terminal velocity.

## Jumping, falling and landing

```rust
// Jump exactly 1.5 meters high. Returns false while in the air (no double jumps).
if ctx.input.pressed(Key::Space) && ctx.physics.jump(player, 1.5) {
    ctx.audio.play("jump");
}

// Landing sounds and fall damage.
for event in ctx.physics.events().to_vec() {
    if let PhysicsEvent::Landed { body, on, speed } = event {
        ctx.audio.sound("land").volume((speed / 12.0).min(1.0)).play();
        if speed > 15.0 {
            ctx.speech.speak("Ouch!");
        }
    }
}

ctx.physics.on_ground(player);
ctx.physics.get(player).unwrap().ground;   // Some(body you stand on)
```

## Velocity, impulses and forces

```rust
ctx.physics.set_velocity(player, vec3(5.0, 0.0, 0.0));
ctx.physics.apply_impulse(ball, vec3(0.0, 10.0, 0.0));   // instant kick
ctx.physics.apply_force(boat, vec3(20.0, 0.0, 0.0));     // newtons, for one step: call every frame for wind or engines
```

Forces respect mass: the same force moves a body with `with_mass(10.0)` ten times slower
than a 1 kg body.

## Mass, pushing and bouncing

- Dynamic bodies hitting each other exchange momentum like billiard balls. Heavy bodies push
  light ones further. `with_bounciness(1.0)` makes collisions fully elastic.
- A kinematic player moved with `move_by` **pushes dynamic bodies** (crates, balls). If the
  crate is stuck against a wall, the player is blocked (`result.blocked`).
- Dynamic bodies standing on a moving **kinematic** body (a lift, a moving platform, a
  conveyor) are carried along:

```rust
let lift = ctx.physics.add(Body::kinematic(vec3(10.0, 0.0, 0.0)).with_box(3.0, 3.0, 0.5));
ctx.physics.set_velocity(lift, vec3(0.0, 0.0, 2.0));   // goes up, and so does whoever stands on it
```

## Stepping

The world steps automatically at a fixed rate (`Engine::physics_rate`, 60 Hz by default).
Your scene's `fixed_update` runs right after each step.

## Events

```rust
for event in ctx.physics.events() {
    match event {
        PhysicsEvent::CollisionStarted { a, b, speed } => {
            // `a` moved into `b` with `speed` m/s. Good for impact sounds:
            ctx.audio.sound("thud").volume((speed / 10.0).min(1.0)).at(ctx.physics.position(*a)).play();
        }
        PhysicsEvent::CollisionEnded { .. } => {}
        PhysicsEvent::Landed { body, on, speed } => { /* body touched ground after falling */ }
        PhysicsEvent::Entered { sensor, body } => { /* body walked into a sensor */ }
        PhysicsEvent::Left { sensor, body } => {}
    }
}
```

`event.involves(id)` and `event.other(id)` help filter events. If you need to change
`ctx` (play sounds, remove bodies) while looping, iterate over a copy:
`for e in ctx.physics.events().to_vec()`.

## Queries

```rust
// What is in front of me? Great for "sonar" or auto describing walls.
if let Some(hit) = ctx.physics.raycast(pos, math::forward(yaw), 20.0, Some(player)) {
    let what = ctx.physics.tag(hit.body);
    ctx.speech.speak(format!("{what} in {:.0} meters", hit.distance));
}

ctx.physics.in_radius(pos, 5.0);              // nearby bodies, nearest first
ctx.physics.nearest_with_tag(pos, "enemy");
ctx.physics.at_point(pos);
ctx.physics.with_tag("coin").count();
ctx.physics.overlaps(a, b);
ctx.physics.sensors_of(player);               // which zones am I in?
```

## Accessing bodies

```rust
ctx.physics.position(id);
ctx.physics.set_position(id, pos);   // teleport
ctx.physics.get(id);                 // Option<&Body>
ctx.physics.get_mut(id);             // change anything
ctx.physics.remove(id);
ctx.physics.clear();
```

## Limits

- Collisions are resolved one body at a time. Big piles of stacked dynamic bodies may jitter.
- Fast bodies are moved in sub steps so they do not tunnel through thin walls, up to a
  limit.
- If you need rotation, joints or many thousands of bodies, use a full physics engine such
  as rapier together with audio-code.
