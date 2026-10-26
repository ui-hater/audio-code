# Context

`Context` is passed to every scene method as `ctx: &mut Context`. It holds all engine
systems as public fields, controls quitting and scene changes, stores data shared by all
scenes, and offers shortcuts for the most common calls.

## Fields

| Field | Type | Page |
|-------|------|------|
| `ctx.time` | `Time` | [time.md](time.md) |
| `ctx.input` | `Input` | [input.md](input.md) |
| `ctx.audio` | `Audio` | [audio.md](audio.md) |
| `ctx.speech` | `Speech` | [speech.md](speech.md) |
| `ctx.ui` | `Ui` | [ui.md](ui.md) |
| `ctx.physics` | `World` | [physics.md](physics.md) |
| `ctx.storage` | `Storage` | [storage.md](storage.md) |
| `ctx.lang` | `Lang` | [lang.md](lang.md) |
| `ctx.net` | `Net` | [net.md](net.md) |

The fields are separate, so you can borrow two of them at once:

```rust
let here = ctx.physics.position(player);
ctx.audio.play_at("step", here);
```

## Quitting

### `ctx.quit()`

Ends the game after this frame. All scenes get `end`, storage is saved, the window closes.

```rust
if ctx.pressed(Key::Escape) {
    ctx.quit();
}
```

### `ctx.is_quitting() -> bool`

Whether `quit` was called this frame.

```rust
if ctx.is_quitting() {
    ctx.speak("Goodbye");
}
```

### `ctx.title() -> &str`

The title passed to `Engine::new`.

```rust
let text = format!("Welcome to {}", ctx.title());
ctx.speak(text);
```

## Scene changes

All three take effect after the current frame. If several are called in one frame, the
last one wins. A scene's `start` may itself request another change.

### `ctx.switch_scene(scene: impl Scene)`

Replaces the current scene: the old one gets `end`, the new one gets `start`.

```rust
struct GameOver;
impl Scene for GameOver {
    fn update(&mut self, ctx: &mut Context) {}
}

if ctx.physics.landed(player).is_some_and(|speed| speed > 12.0) {
    ctx.switch_scene(GameOver);
}
```

### `ctx.push_scene(scene: impl Scene)`

Puts a scene on top of the current one. The current scene gets `pause` and stops
receiving `update` until the pushed one is popped. Typical for pause screens and inventory.

```rust
struct Pause;
impl Scene for Pause {
    fn update(&mut self, ctx: &mut Context) {
        if ctx.pressed(Key::Escape) {
            ctx.pop_scene();
        }
    }
}

if ctx.pressed(Key::P) {
    ctx.push_scene(Pause);
}
```

### `ctx.pop_scene()`

Removes the current scene (`end`), the scene below gets `resume`. Popping the last scene
quits the game.

```rust
if ctx.pressed(Key::Escape) {
    ctx.pop_scene();
}
```

## Shared data

One value per Rust type, shared by all scenes (settings, score, the save game). Define
your own struct and store it.

### `ctx.insert<T: 'static>(value: T)`

Stores a value. Storing another value of the same type replaces the old one.

```rust
struct Score(u32);
ctx.insert(Score(0));
```

### `ctx.get<T: 'static>() -> Option<&T>`

Reads a value stored with `insert`.

```rust
struct Score(u32);
if let Some(score) = ctx.get::<Score>() {
    let text = format!("Score {}", score.0);
    ctx.speak(text);
}
```

### `ctx.get_mut<T: 'static>() -> Option<&mut T>`

Mutable access to a stored value.

```rust
struct Score(u32);
if let Some(score) = ctx.get_mut::<Score>() {
    score.0 += 10;
}
```

### `ctx.get_or_default<T: Default + 'static>() -> &mut T`

Mutable access, inserting `T::default()` first if nothing is stored yet.

```rust
#[derive(Default)]
struct Stats {
    deaths: u32,
}
ctx.get_or_default::<Stats>().deaths += 1;
```

### `ctx.remove<T: 'static>() -> Option<T>`

Removes a stored value and returns it.

```rust
struct Score(u32);
let last = ctx.remove::<Score>();
```

## Shortcuts

Short forms of the calls almost every game makes.

### `ctx.speak(text: impl AsRef<str>)`

Speaks text through the screen reader, interrupting current speech. Same as
`ctx.speech.speak(text)`.

```rust
ctx.speak("Door opened");
ctx.speak(format!("{} coins left", 3));
```

### `ctx.play(name: &str) -> Sound`

Plays a sound once, not positioned. Same as `ctx.audio.play(name)`. `"door"` finds
`sounds/door.ogg` (see [audio.md](audio.md#how-sound-names-are-found)).

```rust
ctx.play("door");
```

### `ctx.play_at(name: &str, position: Vec3) -> Sound`

Plays a sound once at a fixed position in the world. Same as `ctx.audio.play_at`.

```rust
ctx.play_at("explosion", vec3(10.0, 5.0, 0.0));
```

### `ctx.pressed(key: Key) -> bool`

Whether the key went down this frame (OS key repeat ignored). Same as
`ctx.input.pressed(key)`.

```rust
if ctx.pressed(Key::Space) {
    ctx.play("jump");
}
```

### `ctx.held(key: Key) -> bool`

Whether the key is held down. Same as `ctx.input.held(key)`.

```rust
if ctx.held(Key::Up) {
    ctx.physics.move_forward(player, 3.0 * ctx.time.delta());
}
```

### `ctx.once(name: &str) -> bool`

`true` the first time it is called with `name`, `false` every time after. For one-time
setup in closure games and "say this only once" hints. Not saved between runs (use
`ctx.storage` for that).

```rust
if ctx.once("intro") {
    ctx.speak("Welcome! Press F1 for help.");
}
```

### `ctx.follow(body: BodyId)`

The listener (the player's ears) follows the body every frame: position, `facing` and
`pitch`. After this you never call `ctx.audio.set_listener` yourself. Takes effect at once.
If the body is removed, following stops.

```rust
let player = ctx.physics.add(Body::kinematic(vec3(2.0, 2.0, 0.0)).with_sphere(0.4));
ctx.follow(player);
```

### `ctx.unfollow()`

Stops following. The listener stays where it is.

```rust
ctx.unfollow();
ctx.audio.set_listener(Vec3::ZERO, 0.0);
```

### `ctx.followed() -> Option<BodyId>`

The body the listener follows, if any.

```rust
if ctx.followed().is_none() {
    ctx.follow(player);
}
```

### `ctx.play_on(body: BodyId, name: &str) -> Sound`

Plays a sound once at a body. The sound moves with the body while it plays.

```rust
ctx.play_on(enemy, "roar");
```

### `ctx.loop_on(body: BodyId, name: &str) -> Sound`

Plays a looping sound at a body. It moves with the body and stops by itself when the body
is removed. For enemies, engines, coins and beacons.

```rust
let coin = ctx.physics.add(Body::sensor(vec3(4.0, 8.0, 0.0)).with_tag("coin"));
ctx.loop_on(coin, "coin_hum");
// later: removing the coin also stops its sound
ctx.physics.remove(coin);
```

### `ctx.attach(body: BodyId, sound: Sound)`

Makes any positioned sound follow a body, for example one made with the sound builder.
The sound is moved to the body at once. Like `loop_on`, it stops when the body is removed.

```rust
let hum = ctx.audio.sound("hum").volume(0.3).looped().at(Vec3::ZERO).play();
ctx.attach(enemy, hum);
```

### Translation shortcuts

`ctx.tr(key)`, `ctx.tr_with(key, args)`, `ctx.set_language(code)` and
`ctx.describe(from, target)` are documented in [lang.md](lang.md).

```rust
ctx.speak(ctx.tr("intro"));
ctx.speak(ctx.describe(player, enemy));   // "3 o'clock, 12 meters" in the current language
```

### Vehicle shortcuts

`ctx.drive(vehicle)`, `ctx.horn(vehicle)`, `ctx.start_engine(vehicle)` and
`ctx.stop_engine(vehicle)` are described on [vehicle.md](vehicle.md#context-shortcuts).

```rust
let car = ctx.physics.add(Body::car(Vec3::ZERO));
ctx.follow(car);
ctx.drive(car);   // every frame
```

### `ctx.stop_sounds_on(body: BodyId)`

Stops every sound attached to a body with `play_on`, `loop_on` or `attach`.

```rust
ctx.stop_sounds_on(enemy);
```
