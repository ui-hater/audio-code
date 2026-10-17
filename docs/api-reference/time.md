# Time

Two kinds of time:

- **Game time**: `ctx.time` (type `Time`). Advances once per frame, can be slowed down or
  paused, and drives physics and the named scheduler.
- **Real time**: `Timer`, `Cooldown`, `Interval`. Standalone values you keep in your own
  structs. They measure wall clock time, ignore the time scale and need no context.

```rust
use audio_code::prelude::*;   // Time, Timer, Cooldown, Interval
```

## Time (`ctx.time`)

### `ctx.time.delta() -> f32`

Seconds of game time since the previous frame, multiplied by the time scale. Huge hitches
are capped at 0.25 s. Multiply speeds by it for frame rate independent movement.

```rust
ctx.physics.move_forward(player, 2.0 * ctx.time.delta());   // 2 m per second
```

### `ctx.time.real_delta() -> f32`

Like `delta`, but ignores the time scale. For menus and effects that run while the game
is paused.

```rust
let fade_step = 0.5 * ctx.time.real_delta();
```

### `ctx.time.elapsed() -> f64`

Seconds of game time since the game started (scaled, stops while paused).

```rust
let minutes = (ctx.time.elapsed() / 60.0).floor();
```

### `ctx.time.real_elapsed() -> Duration`

Wall clock time since the engine started, ignoring the scale.

```rust
let uptime = ctx.time.real_elapsed().as_secs();
```

### `ctx.time.frame() -> u64`

Number of frames since the start.

```rust
if ctx.time.frame() % 60 == 0 {
    // about once per second at 60 fps
}
```

### `ctx.time.fps() -> f32`

Current frames per second, smoothed.

```rust
if ctx.pressed(Key::F12) {
    let text = format!("{:.0} frames per second", ctx.time.fps());
    ctx.speak(text);
}
```

### `ctx.time.scale() -> f32`

Current time scale. 1.0 is normal speed.

```rust
let slow = ctx.time.scale() < 1.0;
```

### `ctx.time.set_scale(scale: f32)`

Speeds up (> 1.0), slows down (< 1.0) or pauses (0.0) game time. Physics, the scheduler and
`delta` follow it. Negative values become 0.

```rust
ctx.time.set_scale(0.5);   // slow motion
```

### `ctx.time.pause()`

Pauses game time. Same as `set_scale(0.0)`.

```rust
ctx.time.pause();
```

### `ctx.time.resume()`

Resumes game time at normal speed (scale 1.0).

```rust
ctx.time.resume();
```

### `ctx.time.is_paused() -> bool`

Whether the scale is 0.

```rust
if ctx.pressed(Key::P) {
    if ctx.time.is_paused() { ctx.time.resume() } else { ctx.time.pause() }
}
```

## Scheduler (`ctx.time`)

Named events that fire after some game time. Names are free strings. Scheduling a name that
is already pending replaces it. An event is reported by `fired` during exactly one frame.

### `ctx.time.after(name: impl Into<String>, seconds: f32)`

Schedules a one-shot event `seconds` of game time from now.

```rust
ctx.time.after("bomb", 3.0);
// in update():
if ctx.time.fired("bomb") {
    ctx.play("explosion");
}
```

### `ctx.time.every(name: impl Into<String>, seconds: f32)`

Schedules a repeating event that fires every `seconds` (minimum 1 ms). The first one fires
after `seconds`.

```rust
ctx.time.every("spawn", 2.5);
if ctx.time.fired("spawn") {
    ctx.speak("A new enemy appears");
}
```

### `ctx.time.fired(name: &str) -> bool`

`true` during the frame in which the named event fired.

```rust
if ctx.time.fired("tick") {
    ctx.play("clock");
}
```

### `ctx.time.fired_events() -> impl Iterator<Item = &str>`

All events that fired this frame.

```rust
let fired: Vec<String> = ctx.time.fired_events().map(String::from).collect();
for name in fired {
    println!("{name}");
}
```

### `ctx.time.cancel(name: &str)`

Cancels a scheduled event. Does nothing if it is not scheduled.

```rust
ctx.time.cancel("bomb");
```

### `ctx.time.is_scheduled(name: &str) -> bool`

Whether an event with this name is waiting to fire. One-shot events stop being scheduled
once they fired.

```rust
if !ctx.time.is_scheduled("reload") {
    ctx.time.after("reload", 1.5);
}
```

### `ctx.time.remaining(name: &str) -> Option<f32>`

Seconds until the event fires, `None` if not scheduled.

```rust
if let Some(left) = ctx.time.remaining("bomb") {
    let text = format!("{left:.0} seconds");
    ctx.speak(text);
}
```

## Timer

A stopwatch in real time. Can be paused, resumed and restarted. `Timer::default()` equals
`Timer::new()`.

### `Timer::new() -> Timer`

A running timer starting at zero.

```rust
use audio_code::time::Timer;
let lap = Timer::new();
```

### `timer.elapsed() -> Duration`

Time measured so far.

```rust
let lap = Timer::new();
let d = lap.elapsed();
```

### `timer.elapsed_secs() -> f32`

Time measured so far in seconds.

```rust
let lap = Timer::new();
let text = format!("{:.1} seconds", lap.elapsed_secs());
```

### `timer.elapsed_ms() -> u64`

Time measured so far in milliseconds.

```rust
let reaction = Timer::new();
let ms = reaction.elapsed_ms();
```

### `timer.has_elapsed(seconds: f32) -> bool`

Whether at least `seconds` have passed.

```rust
let idle = Timer::new();
if idle.has_elapsed(30.0) {
    ctx.speak("Are you still there?");
}
```

### `timer.restart()`

Back to zero, running (also unpauses).

```rust
let mut idle = Timer::new();
if ctx.input.any_pressed() {
    idle.restart();
}
```

### `timer.set(seconds: f32)`

Sets the measured time to `seconds`, keeping the paused state.

```rust
let mut t = Timer::new();
t.set(10.0);
```

### `timer.pause()`

Stops counting until `resume`.

```rust
let mut t = Timer::new();
t.pause();
```

### `timer.resume()`

Continues counting after a pause.

```rust
let mut t = Timer::new();
t.pause();
t.resume();
```

### `timer.is_paused() -> bool`

Whether the timer is paused.

```rust
let t = Timer::new();
assert!(!t.is_paused());
```

## Cooldown

Limits how often something can happen, in real time. A new cooldown is ready at once.

### `Cooldown::new(seconds: f32) -> Cooldown`

A cooldown of `seconds` that is ready immediately.

```rust
struct Gun {
    cooldown: Cooldown,
}
let gun = Gun { cooldown: Cooldown::new(0.5) };
```

### `cooldown.try_use() -> bool`

If ready: starts the cooldown and returns `true`. Otherwise `false`.

```rust
let mut gun = Cooldown::new(0.5);
if ctx.pressed(Key::Space) && gun.try_use() {
    ctx.play("shot");
}
```

### `cooldown.ready() -> bool`

Whether the cooldown has run out (without using it).

```rust
let gun = Cooldown::new(0.5);
if gun.ready() {
    ctx.speak("Ready to fire");
}
```

### `cooldown.trigger()`

Starts the cooldown whether or not it was ready.

```rust
let mut dash = Cooldown::new(2.0);
dash.trigger();
```

### `cooldown.reset()`

Makes the cooldown ready immediately.

```rust
let mut dash = Cooldown::new(2.0);
dash.reset();
```

### `cooldown.remaining() -> f32`

Seconds until ready, 0.0 if ready.

```rust
let dash = Cooldown::new(2.0);
let wait = dash.remaining();
```

### `cooldown.set_length(seconds: f32)`

Changes the cooldown length.

```rust
let mut gun = Cooldown::new(0.5);
gun.set_length(0.25);   // power up: fire twice as often
```

## Interval

Fires at a steady real-time rhythm: footsteps, radar beeps, clocks.

### `Interval::new(seconds: f32) -> Interval`

An interval that fires every `seconds` (minimum 1 ms). The first tick comes after
`seconds`.

```rust
let radar = Interval::new(1.0);
```

### `interval.tick() -> bool`

Call every frame. Returns `true` once each time the interval elapses. If several were
missed, only one tick is reported, and the rhythm stays steady.

```rust
let mut steps = Interval::new(0.4);
if ctx.input.movement() != Vec2::ZERO && steps.tick() {
    ctx.play("step");
}
```

### `interval.reset()`

Restarts counting from zero.

```rust
let mut steps = Interval::new(0.4);
steps.reset();
```

### `interval.set_interval(seconds: f32)`

Changes how often it fires, for example faster beeps when closer.

```rust
let mut radar = Interval::new(1.0);
let distance = ctx.physics.distance(player, enemy);
radar.set_interval((distance / 10.0).clamp(0.1, 1.0));
```

### `interval.interval() -> f32`

Current interval length in seconds.

```rust
let radar = Interval::new(1.0);
assert_eq!(radar.interval(), 1.0);
```
