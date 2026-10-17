# audio-code

A simple, high level game engine for **audio games**, written in Rust.

Audio games are played by ear: everything the player needs to know comes from speech and
sound, and nothing has to be seen on the screen. Blind and visually impaired players play
them with a screen reader, and many sighted players enjoy them too.

audio-code bundles everything such a game needs in one crate, behind an API that tries to
make the common things one line long:

| Area | What you get |
|------|--------------|
| **Speech** | Output through the player's screen reader (NVDA, JAWS, VoiceOver, Orca, ...) or the system voice via [prism](https://github.com/ethindp/prism). NVDA's controller client is a fallback on Windows, and the console is the last resort. |
| **Sound** | One-line playback, 3D positioned sounds (left/right, front/back, above/below, a listener that can look up and down), loops, pitch, fades, music with cross fades, volume groups. Files are found and cached for you. |
| **Synth** | Design clicks, beeps and effects in code. You do not need any sound files to start. |
| **Virtual UI** | Spoken menus with buttons, sliders, checkboxes, choices and text fields, plus message boxes, yes/no questions and text input. Keyboard only, with replaceable interface sounds. |
| **Physics** | Full 3D: walls, floors, ceilings, gravity, jumping, flying with facing and pitch, mass, forces, pushable objects, moving platforms, trigger zones, ray casts, landing and collision events. |
| **Vehicles** | Cars and trucks with weight and cargo, speed in km/h, engine, automatic gears, brakes, handbrake, reverse, steering, fuel, crashes, and engine, horn, shift and crash sounds. Optional: other bodies are not affected. |
| **Levels** | Draw a level as a text map: `#` is a wall, letters are spawn points. |
| **Time** | Frame delta, slow motion and pause, a named scheduler (`after`, `every`), timers, cooldowns, intervals. |
| **Input** | Pressed / held / released keys, key repeat with your own timing, typed text, direction helpers. |
| **Scenes** | Main menu, levels and pause screens as a stack. Shared data between scenes. |
| **Saving** | `ctx.storage` stores high scores and settings and writes them when the game quits. Optionally encrypted, so players cannot edit their save. |
| **Windows** | A plain window (winit) by default, or a native wxWidgets window via [wxDragon](https://crates.io/crates/wxdragon). |

## Contents

- [Quick start](#quick-start)
- [A complete game, explained](#a-complete-game-explained)
- [Core concepts](#core-concepts)
- [Installation and requirements](#installation-and-requirements)
- [Examples](#examples)
- [How the code is organized](#how-the-code-is-organized)
- [How it works inside](#how-it-works-inside)
- [Developing audio-code](#developing-audio-code)
- [Documentation](#documentation)
- [License](#license)

## Quick start

Create a project and add the engine:

```sh
cargo new my-game
cd my-game
cargo add audio-code --git https://github.com/<you>/audio-code
```

Replace `src/main.rs` with:

```rust,no_run
// The prelude brings in everything you normally need: Context, Key, Body, vec3, ...
use audio_code::prelude::*;

fn main() -> audio_code::Result<()> {
    // `run` opens a window titled "Hello" and calls the closure every frame
    // (60 times per second) until the game quits.
    audio_code::run("Hello", |ctx| {
        // `once` is true only the first time it is called with this name,
        // so the greeting is spoken once at startup.
        if ctx.once("intro") {
            ctx.speak("Press space to beep, escape to quit.");
        }

        // `pressed` is true in the one frame in which the key went down.
        if ctx.pressed(Key::Space) {
            // A generated 440 Hz beep, 0.1 seconds long. No sound file needed.
            ctx.audio.play_tone(440.0, 0.1);
        }

        if ctx.pressed(Key::Escape) {
            ctx.quit();
        }
    })
}
```

Run it with `cargo run`. Your screen reader speaks the greeting, and Space beeps.

`ctx` (the [`Context`](#the-context)) is your access to everything in the engine. The closure
style is good for experiments. Real games use [scenes](#scenes), as in the next section.

## A complete game, explained

A maze: walk through corridors, follow the sound of three beacons, and try to beat your best
time. This is a shortened version of `examples/beacon.rs`.

```rust,no_run
use audio_code::prelude::*;

// The level as a text map. Every character is one tile:
// `#` = wall, `P` = player start, `B` = beacon, `.` = floor.
// The first line is north.
const MAP: &str = "
    ###########
    #B....#..B#
    #.###.#.#.#
    #...#...#.#
    ###.#####.#
    #B........#
    #....P....#
    ###########
";

// A scene holds the state of one part of the game. Here: the maze level.
struct Maze {
    player: BodyId,
    beacons_left: usize,
    clock: Timer,
}

impl Maze {
    // Building the scene needs the context (to create bodies and sounds),
    // so it is started with `run_with(Maze::new)` below.
    fn new(ctx: &mut Context) -> Self {
        // Sounds made with the synth, registered under a name like a file would be.
        ctx.audio.register("step", Synth::noise().length(0.08).decay(60.0));
        ctx.audio.register("wall", Synth::sine(90.0).length(0.15).decay(25.0));
        ctx.audio.register("beacon", Synth::sine(1320.0).length(0.1).then(Synth::silence(0.9)));

        // Turn the text map into walls. Each tile is 2 x 2 meters.
        let level = ctx.physics.build_level(MAP, 2.0);

        // The player is a small kinematic sphere: it only moves when our code moves it.
        let start = level.point('P').unwrap();
        let player = ctx.physics.add(Body::kinematic(start).with_sphere(0.4));

        // The player's ears follow the player's position and facing from now on.
        ctx.follow(player);

        // Every beacon is a trigger zone (sensor) that plays a looping beep.
        // `loop_on` keeps the sound at the body and stops it when the body is removed.
        let spots = level.points('B');
        for spot in &spots {
            let beacon = ctx.physics.add(Body::sensor(*spot).with_sphere(0.8).with_tag("beacon"));
            ctx.loop_on(beacon, "beacon");
        }

        Self { player, beacons_left: spots.len(), clock: Timer::new() }
    }
}

impl Scene for Maze {
    // Called once when the scene starts.
    fn start(&mut self, ctx: &mut Context) {
        ctx.speak("Find the beacons. Up and down walk, left and right turn.");
    }

    // Called every frame.
    fn update(&mut self, ctx: &mut Context) {
        // Turn by 45 degrees and say the new compass direction.
        if ctx.pressed(Key::Left) || ctx.pressed(Key::Right) {
            let degrees = if ctx.pressed(Key::Left) { -45.0 } else { 45.0 };
            let facing = ctx.physics.turn(self.player, degrees);
            ctx.speak(math::direction_name(facing));
        }

        // One step when the key goes down, then another every 0.35 seconds while held.
        if ctx.input.repeating(Key::Up, 0.35, 0.35) {
            // Moves one meter forward and slides along walls. `blocked` tells
            // us that a wall was in the way.
            if ctx.physics.move_forward(self.player, 1.0).blocked {
                ctx.play("wall");
            } else {
                ctx.play("step");
            }
        }

        // Did the player walk into a beacon this frame?
        if let Some(beacon) = ctx.physics.entered_tag(self.player, "beacon") {
            ctx.physics.remove(beacon); // its sound stops by itself
            self.beacons_left -= 1;
            if self.beacons_left > 0 {
                ctx.speak(format!("{} left", self.beacons_left));
            } else {
                // Compare with the best time saved from earlier runs.
                let time = self.clock.elapsed_secs().round() as u32;
                let best: u32 = ctx.storage.get_or("best", u32::MAX);
                if time < best {
                    ctx.storage.set("best", time); // written to disk when the game quits
                }
                ctx.ui.message("done", format!("Done in {time} seconds!"));
            }
        }

        // The message box was closed with Enter or Escape.
        if ctx.ui.closed("done") || ctx.pressed(Key::Escape) {
            ctx.quit();
        }
    }
}

fn main() -> audio_code::Result<()> {
    Engine::new("Beacon maze").run_with(Maze::new)
}
```

What happens here, in order:

1. `Engine::new("Beacon maze")` configures the game. `run_with(Maze::new)` opens the
   window, starts audio and speech, and then calls `Maze::new(ctx)` to build the first
   scene.
2. `build_level` creates one wall body for each unbroken run of `#`s in a row and remembers
   where the letters are.
3. `ctx.follow(player)` and `ctx.loop_on(beacon, ..)` connect sound to the physics world. The
   engine then moves the listener and the beacon sounds every frame for you.
4. Every frame the engine reads the keyboard, runs physics, and calls `update`. Your code
   only decides what happens.
5. `ctx.storage` was loaded from `Beacon_maze.save` at startup and is written back on quit.

## Core concepts

### The engine

`Engine` is a builder that holds the settings, followed by one of the `run` methods:

```rust,no_run
use audio_code::prelude::*;

struct MainMenu;

impl Scene for MainMenu {
    fn update(&mut self, _ctx: &mut Context) {}
}

fn main() -> audio_code::Result<()> {
    Engine::new("My game")          // the title is the window title the screen reader announces
        .fps(60)                    // frames per second (default 60)
        .physics_rate(60)           // physics steps per second (default 60)
        .gravity(EARTH_GRAVITY)     // default: no gravity (top down games)
        .sound_dir("sounds")        // where sound names are looked up (default "sounds")
        .save_file("my-game.save")  // where ctx.storage is saved (default "<title>.save")
        .echo_speech(true)          // also print speech to the console, handy while developing
        .run(MainMenu)              // or .run_with(|ctx| ...), .run_fn(|ctx| ...), .run_wx(...)
}
```

### Scenes

A scene is a struct implementing `Scene`. Only `update` is required:

| Method | Called |
|--------|--------|
| `start` | once, when the scene becomes active |
| `update` | every frame |
| `fixed_update` | at a fixed rate, right after each physics step |
| `on_ui` | for every UI event (menu item pressed, value changed, ...) |
| `pause` / `resume` | when another scene is pushed on top / popped again |
| `end` | when the scene is removed or the game quits |

Scenes form a stack. `ctx.switch_scene(next)` replaces the current scene,
`ctx.push_scene(pause_menu)` puts one on top, `ctx.pop_scene()` returns to the one below,
and `ctx.quit()` ends the game. Changes happen at the end of the frame.

### The context

Every scene method receives `ctx: &mut Context`, which holds all subsystems as public
fields:

| Field | Type | Purpose |
|-------|------|---------|
| `ctx.time` | `Time` | delta time, pause, scheduler |
| `ctx.input` | `Input` | keyboard |
| `ctx.audio` | `Audio` | sounds, music, listener |
| `ctx.speech` | `Speech` | screen reader output |
| `ctx.ui` | `Ui` | menus and dialogs |
| `ctx.physics` | `World` | bodies, collisions, events |
| `ctx.storage` | `Storage` | saved values |

On top of that, `ctx` has shortcuts for the most common calls: `speak`, `play`, `play_at`,
`pressed`, `held`, `once`, `follow`, `loop_on`, `play_on`, `attach`. It also stores data
shared between scenes with `ctx.insert(value)` / `ctx.get::<T>()`.

### One frame

Each frame runs in this order. Knowing it explains most behavior:

1. **Time** advances (`ctx.time.delta()` is the time since the last frame).
2. **Input** collects the keys pressed since the last frame.
3. **UI**: if a menu or dialog is open, it handles the keys, speaks and plays its sounds.
   While a widget is open, your game sees no key presses, so Enter on a menu item does not
   also make your character jump.
4. **`on_ui`** is called for each UI event.
5. **Physics** steps at its fixed rate (possibly several times in one frame), each step
   followed by `fixed_update`.
6. **`update`** runs your game logic.
7. **Sync**: the listener moves to the followed body, and attached sounds move to their
   bodies.
8. **Scene changes** requested during the frame are applied.

### Coordinates and directions

- `+x` is east (right), `+y` is north (forward), `+z` is up.
- Angles are **compass degrees**: 0 = north, 90 = east, 180 = south, 270 = west.
- A position is `vec3(x, y, z)` in meters: `vec3(3.0, -2.0, 1.5)` is 3 m east, 2 m south
  and 1.5 m up.
- Pitch (looking up or down) is in degrees too: 0 = level, 90 = straight up.
- A top down game uses x and y. A side scroller uses x and z with gravity. A flying or
  swimming game uses all three.
- `math::direction_name(90.0)` returns `"east"`. `ctx.physics.describe(player, target)`
  returns `"3 o'clock, 12 meters"`, or `"3 o'clock, 12 meters, 4 meters above"`.
- [docs/3d.md](docs/3d.md) explains positions, directions and 3D in detail.

### Sound names

`ctx.play("door")` looks for `door` as a path, then for `sounds/door`. If the name has no
extension, it also tries `.ogg`, `.wav`, `.mp3`, `.flac`, `.opus` and `.m4a`. Decoded
sounds are cached. A missing file prints one warning and plays nothing, so a typo never
crashes the game. Sounds made in code are registered under a name with
`ctx.audio.register("click", Synth::...)` and played the same way.

## Installation and requirements

```toml
[dependencies]
audio-code = { git = "https://github.com/<you>/audio-code" }
```

The default build is pure Rust. On Windows it speaks through NVDA's controller client DLL
(the `nvda` feature): put `nvdaControllerClient.dll` (from the NVDA controller client
download) next to your game's `.exe`. Details are in
[docs/speech.md](docs/speech.md#nvda-controller-client).

### With prism

For speech through every common screen reader and the system voice, turn on the optional
`prism` feature:

```toml
audio-code = { git = "...", features = ["prism"] }
```

prism is a C++ library that is compiled automatically during the first build. For that you
need **CMake 3.24+** and a **C++23 compiler**:

| System | Additionally needed |
|--------|---------------------|
| Windows | Visual Studio Build Tools with *Desktop development with C++* **and "C++ ATL for latest build tools"** (Visual Studio Installer → Modify → Individual components). Without ATL the build fails with `atlbase.h: No such file or directory`. |
| Linux | `pkg-config`, `libasound2-dev` (audio), and for speech glibmm 2.68+ (`libglibmm-2.68-dev`) and speech-dispatcher 0.11.1+ (`libspeechd-dev`). See the [prism README](https://github.com/ethindp/prism). |
| macOS | Xcode command line tools |

### Feature flags

| Feature    | Default | What it does |
|------------|---------|--------------|
| `prism`    | no      | Speech and braille through screen readers and system voices. |
| `nvda`     | yes     | Windows: speech through `nvdaControllerClient.dll` when prism is unavailable. |
| `wxdragon` | no      | `Engine::run_wx` and `audio_code::wx` for native wxWidgets windows. |

If no speech output is available at all, speech is printed to the console and the game keeps
running.

## Examples

Every example generates its sounds in code, so no audio files are needed. Use headphones for
the 3D sound.

| Example | Shows |
|---------|-------|
| `hello` | speech, beeps, repeating the last message, quitting |
| `clicker` | a whole game in one closure, scheduler, saved high score |
| `menus` | main menu, options with slider/choice/text field, name input, quit confirmation, custom click sounds |
| `beacon` | text map maze, turning and walking, 3D beacons, pause scene, saved best time |
| `platformer` | gravity, jumping, landing sounds, a lift, a pushable crate, collecting coins |
| `flight` | full 3D: fly a drone, look up and down, find birds at different heights |
| `driving` | drive a car or a truck through a town: gears, reverse, horn, fuel, loading the truck, crashes |
| `wx_game` | a game inside a wxWidgets window next to a native button |

```sh
cargo run --example beacon
# with prism speech:
cargo run --features prism --example beacon
# the wxWidgets example:
cargo run --features wxdragon --example wx_game
```

## How the code is organized

```text
src/
├── lib.rs          crate docs, module list, `run()` and the prelude
├── engine.rs       Engine (settings), Scene (trait), Context (fields), Runner (the frame loop)
├── shortcuts.rs    the convenience methods on Context: speak, play, follow, loop_on, once, ...
├── window.rs       default window backend (winit): turns OS key events into engine keys
├── wx.rs           optional wxWidgets backend (feature `wxdragon`)
├── audio/
│   ├── mod.rs      Audio (the public API), Sound handles, SoundBuilder, file lookup and cache
│   ├── mixer.rs    the real-time mixer running on the audio thread, 3D panning
│   ├── data.rs     decoding files into memory (symphonia), generated sound data
│   └── synth.rs    the synthesizer
├── speech/
│   ├── mod.rs      Speech (public API), the speech thread and its fallback chain
│   └── nvda.rs     NVDA controller client, loaded at runtime (Windows)
├── ui/
│   ├── mod.rs      Ui (widget stack), Widget trait, UiEvent, Value
│   ├── menu.rs     Menu and its items
│   ├── dialog.rs   Message, Confirm, TextInput
│   └── sounds.rs   interface sound slots and the built-in sounds
├── physics.rs      World, Body, collision resolution, events, ray casts
├── vehicle.rs      cars and trucks: weight, speed, engine, gears, brakes, fuel
├── level.rs        building walls from text maps
├── input.rs        Key, Input (pressed/held/released, repeat, text)
├── time.rs         Time (frame time + scheduler), Timer, Cooldown, Interval
├── storage.rs      key-value saving
├── math.rs         coordinate conventions, angles, directions, random helpers
└── error.rs        Error and Result
```

## How it works inside

audio-code uses a few threads, but your game code always runs on the main thread. You never
deal with locks or channels yourself.

```text
 main thread                         audio thread (cpal)            speech thread
 ───────────                         ───────────────────            ─────────────
 window events ──► Runner            Mixer                          prism / NVDA / console
                     │                 ▲  drains commands              ▲
                     ▼                 │  each audio block             │ Speak, Stop, ...
   your Scene ◄──► Context ── Audio ───┘ (channel)                     │
                            └ Speech ──────────────────────────────────┘ (channel)
```

**Window and frame loop.** The window backend (`window.rs` or `wx.rs`) knows nothing about
games. It forwards key presses, key releases and typed text to the `Runner` and calls
`Runner::frame()` about 60 times per second. `Runner` owns the `Context` and the scene stack
and runs the [frame steps](#one-frame) in order. Because the backends only talk to `Runner`,
the same game runs in a winit window, a wxWidgets window, or headless in tests.

**Audio.** `ctx.audio` never mixes sound itself. `play` decodes the file once (into a
cache), creates a *voice* and sends it over a channel to the mixer on the audio thread.
`Sound` handles send further commands (volume, pitch, position, stop) the same way.
Information flows back through shared atomics, for example whether a sound has finished.
For each audio block the mixer:

1. applies the queued commands
2. computes the left and right volume of every positioned voice from the listener: panning
   by direction, quieter with distance, muffled when behind the listener
3. resamples each voice to the device rate (this is also how pitch works), applies fades, and
   adds everything up

Music is decoded on a background thread. Its voice already exists and simply stays silent
until the data is ready, so starting music never makes the game stutter.

**Speech.** `ctx.speech.speak` puts the text into a channel and returns immediately. The
speech thread tries prism first, then the NVDA controller client, and uses the first that
works. If neither works, the thread ends, and `Speech` notices the closed channel and prints
to the console from then on. A history of the last 100 messages stays on the main thread,
which is what makes `repeat_last` possible.

**Virtual UI.** `ctx.ui` is a stack of widgets (`Menu`, `Message`, `Confirm`, `TextInput`, or
your own type implementing `Widget`). Only the top widget gets the keyboard. A widget answers
each frame with a status: stay open, close, close and hand a value to the widget below, or
open a child widget. A menu's text field works by opening a `TextInput` as a child; the typed
text comes back as the value. Events such as "button pressed" are collected for one frame,
where `ctx.ui.clicked(..)` and `Scene::on_ui` can see them.

**Physics.** Bodies are boxes or spheres that never rotate, which keeps the math simple and
predictable. Each step:

1. vehicles turn and change speed from their pedals, then drive like a character (stopped
   by walls)
2. kinematic bodies (moved by your code: platforms, doors) move first and carry whatever
   stands on them
3. dynamic bodies get gravity, forces, damping and friction, then move in small sub steps so
   fast bodies do not pass through thin walls
4. overlaps are pushed apart along the shortest way out, and speed into the surface is
   removed or bounced
5. new contacts become `CollisionStarted` / `Landed` events, and sensors produce `Entered` /
   `Left`

`move_by` / `move_forward` use the same push-out logic right away, which makes them a
simple character controller.

More detail, including design decisions, is in [docs/architecture.md](docs/architecture.md).

## Developing audio-code

```sh
cargo test                                   # unit tests and doc tests (including this README)
cargo test physics                           # only tests whose name contains "physics"
cargo clippy --all-targets --features wxdragon
cargo doc --open                             # API reference
```

Without the ATL component on Windows, add `--no-default-features --features nvda` to every
command. Tests never open a window or an audio device: they use `Engine::headless()`, which
gives silent audio and console speech, and drive frames by hand with `Runner::frame()`.

## Documentation

- [Cheat sheet](docs/cheatsheet.md): the most used calls on one page
- [Getting started](docs/getting-started.md): your first game, step by step
- [Sound](docs/audio.md): playing, positioning, music, the synth, file lookup
- [Speech](docs/speech.md): screen readers, NVDA fallback, history, braille
- [Virtual UI](docs/ui.md): menus, dialogs, text input, custom click sounds, custom widgets
- [Physics](docs/physics.md): bodies, gravity, jumping, events, levels from text maps
- [Cars and trucks](docs/vehicles.md): weight, speed, steering, gears, brakes, fuel, vehicle sounds
- [Positions, directions and 3D](docs/3d.md): what `vec3(x, y, z)` means, angles, flying, sound above and below
- [Time and input](docs/time-and-input.md): delta time, scheduler, timers, keys
- [Scenes, shared data and saving](docs/scenes.md)
- [wxDragon integration](docs/wxdragon.md)
- [Architecture](docs/architecture.md): how the engine works inside
- [API reference](docs/api-reference/README.md): every public function with its signature and an example, for looking things up (also `cargo doc --open`)

## License

MIT, see [LICENSE](LICENSE).
