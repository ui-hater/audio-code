# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`audio-code` is a Rust library crate (edition 2024): a high level game engine for **audio games**, which have no graphics. It is published as open source. API, doc comments and docs are written in **English**. The user communicates in German.

The design goal is simplicity for game developers. Common actions should be one-liners, small mistakes (such as a missing sound file) must warn instead of crash, and everything must be accessible by default.

## Build, test, lint

On this Windows machine the optional `prism` feature **does not build**, because the Visual Studio "C++ ATL" component is missing (`atlbase.h` not found). Do not enable it; the default features (`nvda`) build fine:

```sh
cargo build
cargo test
cargo test   physics::tests::jump_reaches_height   # single test (path filter)
cargo clippy --features wxdragon --all-targets  # must stay warning-free
RUSTDOCFLAGS="-D warnings" cargo doc --features wxdragon --no-deps
cargo run    --example menus      # hello, menus, beacon, platformer
cargo run    --features wxdragon --example wx_game
```

- Run clippy for each feature combination you touch: none, `nvda`, `wxdragon,nvda`. `cfg`-gated code easily produces dead-code or unused-mut warnings in one combination only.
- Feature-gated items (`wx`, `Engine::run_wx`) must not be intra-doc linked from ungated docs. Write them as plain code text.
- Examples open a real window and audio device and run until Escape. For smoke tests, build first and then run the exe with `timeout`, because linking inside `timeout cargo run` gets killed. The windows can steal keyboard focus from the user.
- To get NVDA speech in the examples, `nvdaControllerClient64.dll` must sit next to the exe (`target/debug/examples/`). A copy exists at `%LOCALAPPDATA%\RemSound\`. Never commit the DLL.
- Tests that need an engine use `Engine::new(..).headless().build(scene)`, which gives a silent audio system, console speech and no window, and drive it with `runner.frame()` / `runner.key_down(..)`. UI tests use the `Harness` in `src/ui/mod.rs`.

## Features

- `prism` (optional): speech via the `prismer` crate. It builds prism C++ with CMake and links it statically.
- `nvda` (default, Windows only): loads `nvdaControllerClient.dll` at runtime with `libloading`.
- `wxdragon`: wxWidgets backend (`src/wx.rs`). The first build compiles wxWidgets and is slow.

## Architecture

**Frame loop** (`src/engine.rs`). `Runner` owns the `Context` and a stack of `Box<dyn Scene>`. Window backends only feed key/text events into the `Runner` and call `Runner::frame()`. There are two backends: `src/window.rs` (winit, default) and `src/wx.rs` (frame char-hook plus a `wx Timer`, with state in `Rc<RefCell<State>>`). Order inside `frame()`:

`time.advance` → `input.begin_frame` → `ui.update` (if a widget is open, `input` is set to *blocked*) → `scene.on_ui` per UI event → fixed-rate loop of `physics.step` + `scene.fixed_update` → `scene.update` → `ctx.sync_world()` (listener follows `ctx.follow` body, attached sounds move with or stop with their bodies) → apply pending scene changes (push/pop/switch, collected in `ctx.change`). `Runner::shutdown` saves `ctx.storage`.

`Engine::run_with(|ctx| Scene)` wraps the first scene in a private `Lazy` scene, so its constructor can use the context. `audio_code::run` / `Engine::run_fn` wrap a closure in `FnScene`.

**Ease-of-use layer.** `src/shortcuts.rs` holds the `impl Context` shortcuts (`speak`, `play`, `pressed`, `once`, `follow`, `loop_on`, ...). `src/level.rs` holds `World::build_level` for text maps. `src/storage.rs` holds `ctx.storage`, which is in memory when headless. The design goal is that examples read like the cheat sheet (`docs/cheatsheet.md`). New conveniences go there and into that page.

**Context** exposes public fields `time`, `input`, `audio`, `speech`, `ui` and `physics`, plus a type-keyed map for shared data (`ctx.insert/get/get_mut`). Scenes receive `&mut Context`. Because `Runner` holds the context and the scenes in separate fields, disjoint borrows work.

**Audio** (`src/audio/`). This is a custom mixer, not kira or rodio:
- The game thread never touches mixer state. `Audio` sends `Command`s over an mpsc channel. The cpal callback (`mixer.rs`) drains them each block and renders stereo f32, then converts to the device format.
- Per-voice status (finished, position) flows back through atomics in `VoiceShared`. `Sound` handles hold that `Arc` plus a `Sender`. Dropping a handle does not stop the sound.
- Sound data lives in a `SoundSlot` (`OnceLock`), so music can be decoded on a background thread while its voice already exists.
- Spatialization happens in the audio thread from the listener and voice positions: equal-power pan, distance rolloff, and a low-pass plus volume drop for sounds behind the listener.
- Name lookup: given path, then `sound_dir/name`, then common extensions. Results are cached by the name string. Generated sounds (`Synth`, `SoundData::generate`) are registered under a name with `Audio::register`.
- If no device opens, the engine falls back to `Audio::silent`.

**Speech** (`src/speech/`). One background thread tries prism (`run_prism`), then NVDA (`nvda::run`). If both fail, the thread drops the receiver, and `Speech::send` then sees the closed channel and switches to console echo. History and echo live on the game-thread side.

**UI** (`src/ui/`):
- A stack of `Box<dyn Widget>` (public trait, so users can write widgets).
- A newly opened widget runs `open()` on the *next* `ui.update`. After a pop, the widget below gets `resume(value)`.
- Widgets return a `Status` (`Open`/`Close`/`CloseWith(Value)`/`Push`). A menu text field works by pushing a child `TextInput` that returns `CloseWith(Value::Text)`.
- Events (`UiEvent`) are rebuilt every frame and queried with helpers (`clicked`, `changed`, `confirmed`, ...).
- Interface sounds are names in `UiSounds`. `ui:builtin:*` names are synthesized lazily by `ensure_builtin`. `ui:custom:*` names come from `Ui::set_sound_data`, which queues the data in `pending_sounds` and registers it on the next update, because `Ui` does not own `Audio`.

**Physics** (`src/physics.rs`):
- Axis-aligned boxes and spheres, no rotation, bodies in a `BTreeMap` for deterministic order.
- `step`: kinematic bodies move first and record their delta, which carries bodies standing on them (`Body::ground`). Then each dynamic body integrates gravity, forces, damping, friction and `max_speed`, and moves in substeps.
- `resolve(id)` pushes out along the minimum translation. Overlap is shared by mass between dynamic bodies. Kinematic pushers move dynamic bodies fully, unless `obstructed` (crate against a wall).
- `bounce` handles momentum and restitution and returns the impact speed for `CollisionStarted` and `Landed` events.
- `move_by` is the immediate, sliding "character controller" API.

**Conventions shared across modules** (`src/math.rs`):
- z is up, +y is north, +x is east.
- Angles are compass degrees (0 = north, clockwise). The listener yaw, `math::forward` and the panning in the mixer all rely on this.
- Game-facing APIs take `f32` seconds, linear volume and string ids for UI items.

## Docs

User docs live in `docs/*.md` (guide per topic), `docs/cheatsheet.md`, the README and `CHANGELOG.md`. `docs/architecture.md` explains the internals (frame order, mixer math, physics algorithm, UI state machine). Keep it accurate when changing those parts. The README's Rust code blocks are compiled as doc tests (`ReadmeDoctests` in `lib.rs`), so they must be complete programs marked `rust,no_run`, and other blocks need a language tag (`text`, `sh`, `toml`). When adding or changing public API, update the matching docs page and the changelog.
