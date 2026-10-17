# Input

Keyboard state for the current frame: `ctx.input` (type `Input`). `ctx.pressed(key)` and
`ctx.held(key)` are shortcuts.

- Events are collected between frames, so a key pressed and released within one frame is
  still reported by `pressed` (and `released`).
- While a UI widget is open (`ctx.ui.is_open()`), every query here returns `false`, `None`
  or empty. Use `ctx.input.raw()` to read keys anyway.

```rust
use audio_code::prelude::*;               // Input, Key, Modifiers
use audio_code::input::RawInput;          // only if you name the type
```

## Key states

### `ctx.input.pressed(key: Key) -> bool`

`true` in the frame the key went down. Key repeat from the operating system is ignored.

```rust
if ctx.input.pressed(Key::Enter) {
    ctx.speak("Confirmed");
}
```

### `ctx.input.pressed_or_repeated(key: Key) -> bool`

`true` when the key went down and every time the operating system sends a key repeat while
it is held. Menus and text fields use this. For your own game timing use `repeating`.

```rust
if ctx.input.pressed_or_repeated(Key::Down) {
    ctx.play("scroll");
}
```

### `ctx.input.held(key: Key) -> bool`

`true` as long as the key is down.

```rust
if ctx.input.held(Key::Shift) {
    ctx.speak("Running");
}
```

### `ctx.input.released(key: Key) -> bool`

`true` in the frame the key was let go.

```rust
if ctx.input.released(Key::Space) {
    ctx.play("bow_release");
}
```

### `ctx.input.held_for(key: Key) -> Option<f32>`

How long the key has been held in seconds, `None` when it is up.

```rust
if ctx.input.held_for(Key::Space).is_some_and(|t| t > 1.0) {
    ctx.speak("Fully charged");
}
```

### `ctx.input.held_duration(key: Key) -> Option<Duration>`

Same as `held_for`, as a `std::time::Duration`.

```rust
let held = ctx.input.held_duration(Key::Space);
```

### `ctx.input.repeating(key: Key, delay: f32, every: f32) -> bool`

Key repeat with your own timing, independent of operating system settings. `true` when the
key goes down, again after `delay` seconds, then every `every` seconds while held. Ideal
for walking on a grid or scrolling.

```rust
if ctx.input.repeating(Key::Up, 0.3, 0.3) {
    // one step every 0.3 seconds while Up is held
    ctx.physics.move_forward(player, 1.0);
    ctx.play("step");
}
```

### `ctx.input.any_pressed() -> bool`

`true` if any key went down this frame.

```rust
if ctx.input.any_pressed() {
    ctx.speak("Let's go");
}
```

### `ctx.input.keys_pressed() -> &[Key]`

Keys that went down this frame, in order, including OS repeats.

```rust
for key in ctx.input.keys_pressed().to_vec() {
    ctx.speak(key.name());
}
```

### `ctx.input.modifiers() -> Modifiers`

Current state of Shift, Control and Alt.

```rust
let m = ctx.input.modifiers();
if m.control && ctx.pressed(Key::S) {
    let _ = ctx.storage.save();
}
```

### `ctx.input.text() -> &str`

Text typed this frame, respecting keyboard layout and Shift. Control characters
are removed.

```rust
let typed = ctx.input.text().to_string();
if !typed.is_empty() {
    ctx.speak(typed);
}
```

### `ctx.input.is_blocked() -> bool`

Whether game input is blocked because a UI widget is open.

```rust
if !ctx.input.is_blocked() && ctx.pressed(Key::Escape) {
    ctx.quit();
}
```

## Directions

### `ctx.input.axis(negative: Key, positive: Key) -> f32`

-1.0, 0.0 or 1.0 depending on which of the two keys is held (0.0 for both or none).

```rust
let run = ctx.input.axis(Key::Left, Key::Right);   // -1 left, 1 right
ctx.physics.set_velocity(player, vec3(run * 4.0, 0.0, ctx.physics.velocity(player).z));
```

### `ctx.input.movement() -> Vec2`

Direction from the arrow keys or WASD: `x` is right, `y` is forward. Diagonals are
normalized (length 1), so diagonal movement is not faster.

```rust
let dir = ctx.input.movement();
let step = dir * 3.0 * ctx.time.delta();
ctx.physics.move_by(player, vec3(step.x, step.y, 0.0));
```

### `ctx.input.movement_3d() -> Vec3`

Like `movement`, plus `z`: up with Page Up or E, down with Page Down or Q. Normalized.

```rust
let dir = ctx.input.movement_3d();
ctx.physics.move_by(player, dir * 5.0 * ctx.time.delta());
```

## Raw input

### `ctx.input.raw() -> RawInput<'_>`

A view of the input that ignores UI blocking. `RawInput` has `pressed`,
`pressed_or_repeated`, `held`, `released`, `text`, `keys_pressed` and `modifiers` with the
same meaning as on `Input`.

```rust
// A global "repeat last message" key that also works inside menus.
if ctx.input.raw().pressed(Key::F2) {
    ctx.speech.repeat_last();
}
```

## Key

Letters and digits follow the label on the key in the active keyboard layout, so `Key::Z`
is the key labelled "Z" on QWERTY and QWERTZ keyboards alike.

| Group | Variants |
|-------|----------|
| Letters | `A` .. `Z` |
| Digits | `Digit0` .. `Digit9` |
| Function keys | `F1` .. `F12` |
| Arrows | `Up`, `Down`, `Left`, `Right` |
| Editing | `Enter`, `Escape`, `Space`, `Tab`, `Backspace`, `Delete`, `Insert` |
| Navigation | `Home`, `End`, `PageUp`, `PageDown` |
| Modifiers | `Shift`, `Control`, `Alt` (left and right are the same) |
| Numpad | `Numpad0` .. `Numpad9`, `NumpadAdd`, `NumpadSubtract`, `NumpadMultiply`, `NumpadDivide`, `NumpadDecimal`, `NumpadEnter` |
| Punctuation | `Comma`, `Period`, `Minus`, `Plus`, `Slash`, `Backslash`, `Semicolon`, `Quote`, `LeftBracket`, `RightBracket`, `Backquote` |

`Key` is `Copy`, `Eq`, `Hash` and `Ord`, so it works as a map key.

### `Key::from_char(c: char) -> Option<Key>`

Maps a letter or digit to its key, case insensitive. Other characters give `None`.

```rust
assert_eq!(Key::from_char('q'), Some(Key::Q));
assert_eq!(Key::from_char('7'), Some(Key::Digit7));
```

### `key.name() -> String`

A short readable name for messages: `"Page Up"`, `"5"`, `"F1"`, `"Space"`.

```rust
let jump = Key::Space;
ctx.speak(format!("Press {} to jump", jump.name()));
```

## Modifiers

```rust
pub struct Modifiers {
    pub shift: bool,     // either Shift key held
    pub control: bool,   // either Control key held
    pub alt: bool,       // either Alt key held
}
```

```rust
if ctx.input.modifiers().shift {
    ctx.speak("Shift is down");
}
```
