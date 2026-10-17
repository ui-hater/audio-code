# wxDragon backend

Run a game inside a native wxWidgets window through [wxDragon](https://crates.io/crates/wxdragon).
Needs the `wxdragon` feature. The first build compiles wxWidgets and is slow.

```toml
[dependencies]
audio-code = { version = "0.1", features = ["wxdragon"] }
```

Games behave exactly as with the default winit window: same `Scene`, same `Context`. The
backend feeds keys and typed text into the engine and runs `Runner::frame` from a timer at
`Engine::fps`. When the game quits, the frame closes.

## Engine

### `engine.run_wx(scene: impl Scene) -> Result<()>`

Like `Engine::run`, but opens a native wxWidgets frame with the game title and window
size. Returns an `Error::Window` if wxWidgets cannot start.

```rust
use audio_code::prelude::*;

struct MyGame;

impl Scene for MyGame {
    fn update(&mut self, ctx: &mut Context) {
        if ctx.pressed(Key::Escape) {
            ctx.quit();
        }
    }
}

fn main() -> audio_code::Result<()> {
    Engine::new("My game").run_wx(MyGame)
}
```

### `engine.run_wx_with<S: Scene>(build: impl FnOnce(&mut Context) -> S + 'static) -> Result<()>`

Like `Engine::run_with` for a wxWidgets frame: the first scene is built with access to the
`Context`.

```rust
use audio_code::prelude::*;

struct Level {
    player: BodyId,
}

impl Scene for Level {
    fn update(&mut self, ctx: &mut Context) {}
}

fn main() -> audio_code::Result<()> {
    Engine::new("My game").run_wx_with(|ctx| Level { player: ctx.physics.add(Body::kinematic(Vec3::ZERO)) })
}
```

## GameHost (`audio_code::wx::GameHost`)

Attach a game to a frame of an existing wxDragon application, for example an editor or a
launcher with real native controls. The host creates a `Panel` that receives the keyboard
and a `Timer` that runs the game loop. `GameHost` is `Clone`; clones share the same game.

### `GameHost::attach(frame: &Frame, config: Engine, scene: impl Scene) -> GameHost`

Starts `scene` inside `frame` with the settings of `config` (fps, sound folder, gravity,
...; window title and size are ignored because the frame already exists).

```rust
use audio_code::prelude::*;
use audio_code::wx::GameHost;
use wxdragon::prelude::*;

struct MyGame;
impl Scene for MyGame {
    fn update(&mut self, ctx: &mut Context) {}
}

fn main() {
    let _ = wxdragon::main(|_| {
        let frame = Frame::builder().with_title("Editor").build();
        let host = GameHost::attach(&frame, Engine::new("Editor"), MyGame);
        frame.show(true);
        host.focus();
    });
}
```

### `host.panel() -> &Panel`

The panel that receives keyboard input. Add it to your sizers if the frame has other
widgets.

```rust
let sizer = BoxSizer::builder(Orientation::Vertical).build();
sizer.add(host.panel(), 1, SizerFlag::Expand, 0);
```

### `host.focus()`

Gives the game panel keyboard focus.

```rust
host.focus();
```

### `host.with_context<R>(f: impl FnOnce(&mut Context) -> R) -> Option<R>`

Runs `f` with the game's `Context`, for example from a native button. Returns `None` if the
game is busy (when called from inside a game callback).

```rust
let host2 = host.clone();
button.on_click(move |_| {
    host2.with_context(|ctx| ctx.speech.speak("Saved"));
});
```

See also the topic guide [../wxdragon.md](../wxdragon.md) and the example
`cargo run --features wxdragon --example wx_game`.
