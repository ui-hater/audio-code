# Getting started

This guide builds a small game step by step. You walk along a corridor, listen for a
ticking bomb and defuse it before time runs out.

## 1. Create a project

```sh
cargo new bomb
cd bomb
cargo add audio-code --git https://github.com/<you>/audio-code
```

See the [README](../README.md#requirements) for build requirements (CMake, and on Windows the
Visual Studio "C++ ATL" component for speech).

## 2. The smallest game

The very smallest game is a closure that runs every frame:

```rust
use audio_code::prelude::*;

fn main() -> audio_code::Result<()> {
    audio_code::run("Bomb", |ctx| {
        if ctx.once("intro") {
            ctx.speak("Find the bomb!");
        }
        if ctx.pressed(Key::Escape) {
            ctx.quit();
        }
    })
}
```

That is fine for experiments. For real games, use **scenes**: every game has at least one. A scene is a struct that implements [`Scene`]. The
engine calls `start` once and then `update` every frame (60 times per second by default).

```rust
use audio_code::prelude::*;

struct Corridor;

impl Scene for Corridor {
    fn start(&mut self, ctx: &mut Context) {
        ctx.speech.speak("Find the bomb! Left and right to walk.");
    }

    fn update(&mut self, ctx: &mut Context) {
        if ctx.input.pressed(Key::Escape) {
            ctx.quit();
        }
    }
}

fn main() -> audio_code::Result<()> {
    Engine::new("Bomb").run(Corridor)
}
```

`cargo run` opens an empty window titled "Bomb" and your screen reader speaks the text.

Everything the game can do is in the **context** (`ctx`):

| Field          | Purpose                                   |
|----------------|-------------------------------------------|
| `ctx.speech`   | speak text through the screen reader      |
| `ctx.audio`    | play sounds and music                     |
| `ctx.input`    | keyboard                                  |
| `ctx.ui`       | menus and dialogs                         |
| `ctx.time`     | frame time, scheduler                     |
| `ctx.physics`  | collision world                           |

## 3. Moving around

Store game state in your struct. `repeating` gives you key repeat with your own timing:
one step when the key goes down, and another step every 0.3 seconds while it is held.

```rust
struct Corridor {
    x: f32,
}

impl Scene for Corridor {
    fn update(&mut self, ctx: &mut Context) {
        if ctx.input.repeating(Key::Left, 0.3, 0.3) {
            self.x -= 1.0;
            ctx.audio.play("step");
        }
        if ctx.input.repeating(Key::Right, 0.3, 0.3) {
            self.x += 1.0;
            ctx.audio.play("step");
        }
        // The player's ears are where the player is.
        ctx.audio.set_listener(vec3(self.x, 0.0, 0.0), 0.0);
    }
}
```

`ctx.audio.play("step")` looks for `sounds/step.ogg` (or `.wav`, `.mp3`, ...). If you have
no sound files yet, generate one:

```rust
ctx.audio.register("step", SoundData::tone(200.0, 0.05, 0.5));
```

## 4. A sound in the world

Positioned sounds are panned and get quieter with distance relative to the listener. Sounds
behind the player are muffled a little.

```rust
struct Corridor {
    x: f32,
    tick: Sound,
}

fn start(&mut self, ctx: &mut Context) {
    // Keep the handle so the sound can be stopped later.
    self.tick = ctx.audio.play_looped_at("tick", vec3(15.0, 0.0, 0.0));
}
```

## 5. A time limit

The scheduler fires named events in game time:

```rust
fn start(&mut self, ctx: &mut Context) {
    ctx.time.after("boom", 30.0);
    ctx.time.every("warning", 10.0);
}

fn update(&mut self, ctx: &mut Context) {
    if ctx.time.fired("warning") {
        let left = ctx.time.remaining("boom").unwrap_or(0.0);
        ctx.speech.speak(format!("{left:.0} seconds left"));
    }
    if ctx.time.fired("boom") {
        ctx.audio.play("explosion");
        ctx.ui.message("lost", "The bomb exploded. Press enter.");
    }
    if ctx.ui.closed("lost") {
        ctx.quit();
    }
    if (self.x - 15.0).abs() < 0.5 && ctx.input.pressed(Key::Space) {
        self.tick.stop();
        ctx.time.cancel("boom");
        ctx.speech.speak("Defused! You win.");
    }
}
```

## 6. A main menu

Add a second scene and switch between them:

```rust
struct MainMenu;

impl Scene for MainMenu {
    fn start(&mut self, ctx: &mut Context) {
        ctx.ui.open(
            Menu::new("main", "Bomb")
                .button("play", "Play")
                .button("quit", "Quit")
                .closable(false),
        );
    }

    fn update(&mut self, ctx: &mut Context) {
        if ctx.ui.clicked("main", "play") {
            ctx.ui.close();
            ctx.switch_scene(Corridor { x: 0.0, tick: Sound::none() });
        }
        if ctx.ui.clicked("main", "quit") {
            ctx.quit();
        }
    }
}

fn main() -> audio_code::Result<()> {
    Engine::new("Bomb").run(MainMenu)
}
```

While the menu is open it owns the keyboard, so the arrow keys move through the menu and
your game sees no key presses.

## Shortcuts

The most common calls have short forms directly on `ctx`: `ctx.speak(..)`, `ctx.play(..)`,
`ctx.play_at(..)`, `ctx.pressed(..)` and `ctx.held(..)`. The [cheat sheet](cheatsheet.md)
lists them all.

## Next steps

- [Cheat sheet](cheatsheet.md)
- [Architecture](architecture.md), if you want to know how it works inside
- [Sound](audio.md)
- [Virtual UI](ui.md)
- [Physics](physics.md)
- [Time and input](time-and-input.md)
- [Scenes and shared data](scenes.md)
- The `examples` folder

[`Scene`]: ../src/engine.rs
