# Physics

`ctx.physics` is a small 3D physics world built for audio games. You can bump into walls,
stand on floors, fall, trigger zones and cast rays to check what is ahead.

There are only boxes and spheres, and bodies never rotate. This keeps behavior predictable
and easy to describe with sound.

## Coordinate system

- `+x` = east / right, `+y` = north / forward, `+z` = up
- Angles are compass degrees: 0 = north, 90 = east

A top down game uses x and y and no gravity. A side scroller uses x and z with gravity
`vec3(0.0, 0.0, -9.81)`.

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

## Velocity and gravity

```rust
ctx.physics.set_gravity(vec3(0.0, 0.0, -20.0));
ctx.physics.set_velocity(player, vec3(5.0, 0.0, 0.0));
if ctx.physics.on_ground(player) && ctx.input.pressed(Key::Space) {
    ctx.physics.apply_impulse(player, vec3(0.0, 0.0, 8.0));   // jump
}
```

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

- Dynamic bodies push themselves out of other bodies. They do not push each other around
  like real rigid bodies.
- Fast bodies are moved in sub steps so they do not tunnel through thin walls, up to a
  limit.
- If you need rotation, joints or many thousands of bodies, use a full physics engine such
  as rapier together with audio-code.
