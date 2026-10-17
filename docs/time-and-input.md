# Time and input

## Frame time: `ctx.time`

```rust
ctx.time.delta();          // seconds since last frame (game time)
ctx.time.real_delta();     // same, ignoring the time scale
ctx.time.elapsed();        // seconds since start (game time, f64)
ctx.time.real_elapsed();   // Duration since start (wall clock)
ctx.time.frame();          // frame counter
ctx.time.fps();            // smoothed frames per second
```

Multiply speeds by `delta()` so movement does not depend on the frame rate:

```rust
self.x += 3.0 * ctx.time.delta();   // 3 meters per second
```

### Slow motion and pause

```rust
ctx.time.set_scale(0.5);   // half speed
ctx.time.pause();          // scale 0: physics, scheduler and delta stop
ctx.time.resume();
ctx.time.is_paused();
```

Menus keep working while the game is paused, because the UI does not depend on game time.

### The scheduler

Named events that fire in game time:

```rust
ctx.time.after("explode", 5.0);   // once, in 5 seconds
ctx.time.every("spawn", 2.0);     // every 2 seconds
ctx.time.cancel("spawn");
ctx.time.is_scheduled("explode");
ctx.time.remaining("explode");    // Some(seconds)

if ctx.time.fired("spawn") {
    // ...
}
```

Scheduling a name again replaces the old entry. Because the scheduler follows game time,
pausing the game also pauses all scheduled events.

## Real-time helpers

These are plain structs that you keep in your scene. They measure wall clock time and need
no context.

```rust
use audio_code::prelude::*;

struct Enemy {
    attack: Cooldown,   // limits how often something can happen
    growl: Interval,    // fires at a steady rate
    alive_for: Timer,   // stopwatch
}

let mut enemy = Enemy {
    attack: Cooldown::new(1.5),
    growl: Interval::new(4.0),
    alive_for: Timer::new(),
};

if enemy.attack.try_use() { /* attack at most every 1.5 s */ }
if enemy.growl.tick() { /* every 4 s */ }
if enemy.alive_for.elapsed_secs() > 60.0 { /* ... */ }
```

`Timer` also has `pause`, `resume`, `restart`, `set`, `elapsed_ms` and `has_elapsed`.
`Cooldown` has `ready`, `trigger`, `reset` and `remaining`. `Interval` has `reset` and
`set_interval`.

## Keyboard: `ctx.input`

```rust
ctx.input.pressed(Key::Space);    // went down this frame (OS repeat ignored)
ctx.input.held(Key::Up);          // is down
ctx.input.released(Key::Space);   // went up this frame
ctx.input.pressed_or_repeated(Key::Down);  // like pressed, plus OS key repeat
ctx.input.held_for(Key::Space);   // Some(seconds) while held
ctx.input.any_pressed();
ctx.input.keys_pressed();         // all keys this frame, in order
ctx.input.text();                 // typed text this frame, layout aware
ctx.input.modifiers().shift;
```

A key that goes down and up within one frame still counts as `pressed`.

### Your own key repeat

`repeating(key, delay, every)` returns true when the key goes down, then after `delay`
seconds, then every `every` seconds while the key stays held. It does not depend on the
operating system's repeat settings, which makes it right for grid movement:

```rust
if ctx.input.repeating(Key::Up, 0.4, 0.25) {
    self.step_forward(ctx);
}
```

### Keys

`Key` covers letters (`Key::A`), digits (`Key::Digit1`), `F1`–`F12`, arrows, `Enter`,
`Escape`, `Space`, `Tab`, `Backspace`, `Delete`, `Insert`, `Home`, `End`, `PageUp`,
`PageDown`, `Shift`, `Control`, `Alt`, the numpad and common punctuation.

Letters follow the keyboard layout. `Key::Z` is the key labelled Z on both QWERTY and QWERTZ
keyboards. `Key::name()` gives a readable name such as "Page Up", for help texts.

### UI blocking

While a menu or dialog is open, all the methods above return nothing. To read the keys
anyway, use `ctx.input.raw()`, for example for a global "repeat last message" key:

```rust
if ctx.input.raw().pressed(Key::F1) {
    ctx.speech.repeat_last();
}
```

When the window loses focus, all held keys are released automatically.
