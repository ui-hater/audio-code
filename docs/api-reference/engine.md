# Engine, Scene and Runner

Starting a game, configuring the engine, writing scenes, and driving the engine yourself.
See also [context.md](context.md) for what a scene can do with `ctx`.

```rust
use audio_code::prelude::*;   // Engine, Scene, Context and everything else
```

## Starting a game

### `audio_code::run(title: &str, update: impl FnMut(&mut Context) + 'static) -> Result<()>`

The quickest start: the whole game is one closure that is called every frame. The closure
keeps its state in captured variables. Same as `Engine::new(title).run_fn(update)`. Opens
a window and returns when the game quits.

```rust
use audio_code::prelude::*;

fn main() -> audio_code::Result<()> {
    let mut score = 0;
    audio_code::run("Clicker", move |ctx| {
        if ctx.once("intro") {
            ctx.speak("Press space. Escape quits.");
        }
        if ctx.pressed(Key::Space) {
            score += 1;
            ctx.audio.play_tone(400.0 + score as f32 * 10.0, 0.05);
        }
        if ctx.pressed(Key::Escape) {
            ctx.speak(format!("Score: {score}"));
            ctx.quit();
        }
    })
}
```

## Engine

`Engine` is a builder that holds the configuration. Create it with `Engine::new`, chain the
settings you need, and finish with one of the `run*` or `build*` methods.

### `Engine::new(title: impl Into<String>) -> Engine`

Creates a configuration with defaults: 60 fps, 60 physics steps per second, sound folder
`"sounds"`, no gravity, window 640×480, save file `<title>.save`. The title is the window
title, which the screen reader announces when the window gets focus.

```rust
use audio_code::prelude::*;

struct MainMenu;

impl Scene for MainMenu {
    fn update(&mut self, ctx: &mut Context) {}
}

fn main() -> audio_code::Result<()> {
    Engine::new("My game").run(MainMenu)
}
```

### `engine.fps(fps: u32) -> Engine`

Target frames per second, clamped to 1..=1000. Default 60. `Scene::update` runs once per
frame.

```rust
let engine = Engine::new("Game").fps(120);
```

### `engine.physics_rate(rate: u32) -> Engine`

Physics steps (and `Scene::fixed_update` calls) per second, clamped to 1..=1000.
Default 60.

```rust
let engine = Engine::new("Game").physics_rate(120);
```

### `engine.gravity(gravity: Vec3) -> Engine`

Gravity of the physics world in m/s². Default `Vec3::ZERO` (no gravity, right for top down
games). Use `EARTH_GRAVITY` (`vec3(0.0, 0.0, -9.81)`) for platformers. Can be changed
later with `ctx.physics.set_gravity`.

```rust
let engine = Engine::new("Jumper").gravity(EARTH_GRAVITY);
let moon = Engine::new("Moon").gravity(vec3(0.0, 0.0, -1.62));
```

### `engine.save_file(path: impl Into<PathBuf>) -> Engine`

File used by `ctx.storage`. Default `<title>.save` in the working directory, with every
character that is not a letter, digit, `-` or `_` replaced by `_`.

```rust
let engine = Engine::new("Game").save_file("saves/profile1.save");
```

### `engine.encrypt_save(secret: impl Into<String>) -> Engine`

Encrypts the save file of `ctx.storage` with a key made from `secret` (optional, default
off). Players can no longer read or edit it; a changed or damaged file is detected, kept as
`<file>.broken`, and reported by `ctx.storage.load_error()`. An existing plain text save
still loads and is written encrypted on the next save. See
[storage.md](storage.md#encryption).

```rust
let engine = Engine::new("Game").encrypt_save("my secret word");
```

### `engine.sound_dir(dir: impl Into<PathBuf>) -> Engine`

Folder searched for sounds by name. Default `"sounds"`, so `ctx.play("step")` finds
`sounds/step.ogg`. See [audio.md](audio.md#how-sound-names-are-found).

```rust
let engine = Engine::new("Game").sound_dir("assets/audio");
```

### `engine.echo_speech(echo: bool) -> Engine`

Also prints everything spoken to the console. Default `false`. Handy while developing.

```rust
let engine = Engine::new("Game").echo_speech(true);
```

### `engine.window_size(width: u32, height: u32) -> Engine`

Window size in pixels. Default 640×480. The window draws nothing; it exists so the game
receives keyboard input and can be found and focused.

```rust
let engine = Engine::new("Game").window_size(800, 600);
```

### `engine.headless() -> Engine`

No audio device (silent `Audio`), speech printed to the console, storage kept in memory.
Meant for automated tests and tools; combine with `Engine::build`.

```rust
let mut runner = Engine::new("test").headless().build_fn(|ctx| {
    ctx.speak("hello");
});
runner.frame();
```

### `engine.run(scene: impl Scene) -> Result<()>`

Opens the window (winit backend) and runs the game until it quits. Returns an
`Error::Window` if no window can be created.

```rust
use audio_code::prelude::*;

struct Game;

impl Scene for Game {
    fn update(&mut self, ctx: &mut Context) {
        if ctx.pressed(Key::Escape) {
            ctx.quit();
        }
    }
}

fn main() -> audio_code::Result<()> {
    Engine::new("Game").run(Game)
}
```

### `engine.run_with<S: Scene>(build: impl FnOnce(&mut Context) -> S + 'static) -> Result<()>`

Like `run`, but builds the first scene with access to the `Context`. Use it when the
scene's constructor needs to add bodies, load sounds or read storage. `build` runs once,
before the scene's `start`.

```rust
use audio_code::prelude::*;

struct Level {
    player: BodyId,
}

impl Level {
    fn new(ctx: &mut Context) -> Self {
        let player = ctx.physics.add(Body::kinematic(Vec3::ZERO).with_sphere(0.4));
        ctx.follow(player);
        Self { player }
    }
}

impl Scene for Level {
    fn update(&mut self, ctx: &mut Context) {
        let step = ctx.input.movement() * 3.0 * ctx.time.delta();
        ctx.physics.move_by(self.player, step.extend(0.0));
    }
}

fn main() -> audio_code::Result<()> {
    Engine::new("Game").gravity(Vec3::ZERO).run_with(Level::new)
}
```

### `engine.run_fn(update: impl FnMut(&mut Context) + 'static) -> Result<()>`

Runs a game written as a single closure called every frame. `audio_code::run` is a
shortcut for this.

```rust
use audio_code::prelude::*;

fn main() -> audio_code::Result<()> {
    Engine::new("Tiny").echo_speech(true).run_fn(|ctx| {
        if ctx.pressed(Key::Escape) {
            ctx.quit();
        }
    })
}
```

### `engine.build(scene: impl Scene) -> Runner`

Creates the context and a `Runner` without opening a window. You call `runner.frame()`
yourself. Used by window backends, tests and custom loops.

```rust
use audio_code::prelude::*;

struct Game;

impl Scene for Game {
    fn update(&mut self, ctx: &mut Context) {
        if ctx.pressed(Key::Space) {
            ctx.speak("Space");
        }
    }
}

fn main() {
    let mut runner = Engine::new("test").headless().build(Game);
    runner.frame();                       // first frame: Scene::start
    runner.key_down(Key::Space, false);
    runner.frame();                       // Scene::update sees Space pressed
    assert_eq!(runner.ctx.speech.last(), Some("Space"));
}
```

### `engine.build_fn(update: impl FnMut(&mut Context) + 'static) -> Runner`

Like `build` for a closure game.

```rust
let mut runner = Engine::new("test").headless().build_fn(|ctx| {
    if ctx.pressed(Key::Enter) {
        ctx.quit();
    }
});
runner.frame();
runner.key_down(Key::Enter, false);
assert!(!runner.frame());   // false: the game quit
```

`engine.run_wx(..)` and `engine.run_wx_with(..)` (feature `wxdragon`) are described in
[wx.md](wx.md).

## Scene

```rust
pub trait Scene: 'static {
    fn start(&mut self, ctx: &mut Context) {}
    fn update(&mut self, ctx: &mut Context);           // required
    fn fixed_update(&mut self, ctx: &mut Context) {}
    fn on_ui(&mut self, event: &UiEvent, ctx: &mut Context) {}
    fn pause(&mut self, ctx: &mut Context) {}
    fn resume(&mut self, ctx: &mut Context) {}
    fn end(&mut self, ctx: &mut Context) {}
}
```

One screen or state of the game: main menu, level, pause screen, game over. Only `update`
is required. Scenes are changed with `ctx.switch_scene`, `ctx.push_scene` and
`ctx.pop_scene` (see [context.md](context.md#scene-changes)).

Order inside one frame: UI update → `on_ui` for each UI event → zero or more physics steps,
each followed by `fixed_update` → `update` → listener and attached sounds follow their
bodies → pending scene change is applied.

### `fn start(&mut self, ctx: &mut Context)`

Called once when the scene becomes active for the first time (game start, switch or push).
Speak the intro, open the menu, start music here.

```rust
use audio_code::prelude::*;

struct Intro;

impl Scene for Intro {
    fn start(&mut self, ctx: &mut Context) {
        ctx.speak("Welcome. Press Enter to begin.");
        ctx.audio.play_music("music/intro");
    }
    fn update(&mut self, ctx: &mut Context) {}
}
```

### `fn update(&mut self, ctx: &mut Context)`

Called every frame. Game logic goes here. Multiply speeds by `ctx.time.delta()` to stay
independent of the frame rate.

```rust
use audio_code::prelude::*;

struct Walk {
    x: f32,
}

impl Scene for Walk {
    fn update(&mut self, ctx: &mut Context) {
        self.x += ctx.input.axis(Key::Left, Key::Right) * 2.0 * ctx.time.delta();
        ctx.audio.set_listener(vec3(self.x, 0.0, 0.0), 0.0);
    }
}
```

### `fn fixed_update(&mut self, ctx: &mut Context)`

Called at a fixed rate (`Engine::physics_rate`, default 60 per second), right after each
physics step. Use it for logic that must not depend on the frame rate, such as applying
forces. It can run 0 to 8 times per frame.

```rust
use audio_code::prelude::*;

struct Windy {
    leaf: BodyId,
}

impl Scene for Windy {
    fn fixed_update(&mut self, ctx: &mut Context) {
        ctx.physics.apply_force(self.leaf, vec3(2.0, 0.0, 0.0));
    }
    fn update(&mut self, ctx: &mut Context) {}
}
```

### `fn on_ui(&mut self, event: &UiEvent, ctx: &mut Context)`

Called for every UI event of the frame, before `update`. An alternative to polling
`ctx.ui.clicked(..)` in `update`.

```rust
use audio_code::prelude::*;

struct MainMenu;

impl Scene for MainMenu {
    fn start(&mut self, ctx: &mut Context) {
        ctx.ui.open(Menu::new("main", "Main menu").button("play", "Play").button("quit", "Quit"));
    }
    fn on_ui(&mut self, event: &UiEvent, ctx: &mut Context) {
        if let UiEvent::Activated { item, .. } = event {
            match item.as_str() {
                "quit" => ctx.quit(),
                _ => ctx.speak("Not yet"),
            }
        }
    }
    fn update(&mut self, ctx: &mut Context) {}
}
```

### `fn pause(&mut self, ctx: &mut Context)`

Called when another scene is pushed on top of this one. Good place to pause loops.

```rust
use audio_code::prelude::*;

struct Level {
    engine_sound: Sound,
}

impl Scene for Level {
    fn pause(&mut self, ctx: &mut Context) {
        self.engine_sound.pause();
        ctx.time.pause();
    }
    fn resume(&mut self, ctx: &mut Context) {
        self.engine_sound.resume();
        ctx.time.resume();
    }
    fn update(&mut self, ctx: &mut Context) {}
}
```

### `fn resume(&mut self, ctx: &mut Context)`

Called when this scene is on top again after the scene above it was popped. See the
example under `pause`.

### `fn end(&mut self, ctx: &mut Context)`

Called when the scene is removed: popped, switched away from, or when the game quits.
Clean up bodies and sounds here.

```rust
use audio_code::prelude::*;

struct Level;

impl Scene for Level {
    fn end(&mut self, ctx: &mut Context) {
        ctx.physics.clear();
        ctx.audio.stop_all();
    }
    fn update(&mut self, ctx: &mut Context) {}
}
```

## Runner

`audio_code::Runner` drives scenes frame by frame. Window backends feed it key events and
call `frame()`. You only need it for tests, tools or your own backend. Create it with
`Engine::build` or `Engine::build_fn`.

### `runner.ctx: Context`

The public context. Tests read and change it directly.

```rust
let mut runner = Engine::new("t").headless().build_fn(|_| {});
let p = runner.ctx.physics.add(Body::kinematic(Vec3::ZERO));
runner.frame();
assert_eq!(runner.ctx.physics.position(p), Vec3::ZERO);
```

### `runner.frame() -> bool`

Runs one frame now. The very first call only runs `Scene::start`. Returns `false` once the
game has quit (and then storage has been saved and all scenes ended).

```rust
let mut runner = Engine::new("t").headless().build_fn(|ctx| ctx.quit());
runner.frame();              // start
assert!(!runner.frame());    // update calls quit
```

### `runner.key_down(key: Key, repeat: bool)`

Reports a key going down. `repeat` is `true` for key repeats sent by the operating system.
The key is visible in the next `frame()`.

```rust
let mut runner = Engine::new("t").headless().build_fn(|_| {});
runner.frame();
runner.key_down(Key::Up, false);
runner.frame();
assert!(runner.ctx.input.held(Key::Up));
```

### `runner.key_up(key: Key)`

Reports a key going up.

```rust
let mut runner = Engine::new("t").headless().build_fn(|_| {});
runner.key_down(Key::Up, false);
runner.frame();
runner.key_up(Key::Up);
runner.frame();
```

### `runner.text(text: &str)`

Reports typed text (control characters are dropped). Text fields read it through
`ctx.input.text()`.

```rust
let mut runner = Engine::new("t").headless().build_fn(|_| {});
runner.frame();
runner.text("hello");
runner.frame();
assert_eq!(runner.ctx.input.text(), "hello");
```

### `runner.focus_lost()`

Releases all held keys. Call it when the window loses focus, so no key stays stuck.

```rust
let mut runner = Engine::new("t").headless().build_fn(|_| {});
runner.focus_lost();
```

### `runner.next_frame_time() -> Instant`

When the next frame is due according to `Engine::fps`. Backends wait until then.

```rust
let mut runner = Engine::new("t").headless().build_fn(|_| {});
let due = runner.next_frame_time();
```

### `runner.should_quit() -> bool`

Whether the game wants to quit.

```rust
let mut runner = Engine::new("t").headless().build_fn(|_| {});
if runner.should_quit() {
    runner.shutdown();
}
```

### `runner.shutdown()`

Ends all scenes (`Scene::end`) and saves `ctx.storage`. Called automatically when the game
quits; call it yourself when you stop driving the runner early.

```rust
let mut runner = Engine::new("t").headless().build_fn(|_| {});
runner.frame();
runner.shutdown();
```

## Error and Result

### `audio_code::Result<T>`

Shorthand for `std::result::Result<T, audio_code::Error>`. `main` usually returns
`audio_code::Result<()>`.

### `audio_code::Error`

```rust
pub enum Error {
    Io(std::io::Error),   // a file could not be read
    Decode(String),       // a sound file could not be decoded
    Audio(String),        // the audio device could not be opened
    Window(String),       // the window or event loop could not be created
}
```

Implements `Display`, `std::error::Error` and `From<std::io::Error>`. Most of the engine
never returns errors (missing sounds only warn). You see them from `Engine::run`,
`Audio::preload`, `Audio::preload_dir`, `SoundData::load` and `Storage::save`.

```rust
match ctx.audio.preload("music/boss") {
    Ok(()) => {}
    Err(audio_code::Error::Io(e)) => eprintln!("missing: {e}"),
    Err(e) => eprintln!("{e}"),
}
```

## Prelude

`use audio_code::prelude::*;` imports:

- `Context`, `Engine`, `Scene`
- `Audio`, `Group`, `Sound`, `SoundData`, `Synth`, `Wave`
- `Input`, `Key`, `Modifiers`
- `Level`
- `math` (the module), `Vec2`, `Vec3`, `vec2`, `vec3`, `random`
- `Body`, `BodyId`, `BodyKind`, `EARTH_GRAVITY`, `MoveResult`, `PhysicsEvent`, `RayHit`,
  `Shape`, `World`
- `Speech`, `Storage`
- `Cooldown`, `Interval`, `Time`, `Timer`
- `Confirm`, `Menu`, `Message`, `TextInput`, `Ui`, `UiEvent`, `UiSound`, `UiSounds`,
  `Value`, `Widget`
- `Vehicle`

Not in the prelude (import by path): `audio_code::Runner`, `audio_code::Error`,
`audio_code::audio::SoundBuilder`, `audio_code::input::RawInput`,
`audio_code::ui::{Status, UiContext, MenuItem, ItemKind}`, `audio_code::wx::GameHost`.
