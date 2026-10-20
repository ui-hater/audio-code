# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`audio-code` is a Rust library crate (edition 2024): a high level game engine for **audio games**, which have no graphics. It is published as open source. API, doc comments and docs are written in **English**. The user communicates in German.

The design goal is simplicity for game developers. Common actions should be one-liners, small mistakes (such as a missing sound file) must warn instead of crash, and everything must be accessible by default.

## Build, test, lint

The default `prism` feature uses the bindings from `https://github.com/garo-pro/prism2rust` (git dependency; cargo checks out the prism submodule itself). Its native C++ build needs CMake and the Visual Studio "C++ ATL" component (installed here); the first build takes a few minutes. `PRISM_SYS_NO_NATIVE=1` skips the C++ build for quick clippy/check/doc runs (not for linking).

```sh
cargo build
cargo test
cargo test   physics::tests::jump_reaches_height   # single test (path filter)
cargo clippy --features wxdragon --all-targets  # must stay warning-free
RUSTDOCFLAGS="-D warnings" cargo doc --features wxdragon --no-deps
cargo run    --example menus      # hello, clicker, menus, beacon, bell_catcher, platformer, flight, driving
cargo run    --features wxdragon --example wx_game
```

- prism is linked dynamically. `build.rs` copies `prism.dll` from prism-sys's install prefix (`DEP_PRISM_ROOT`, which is why `prism-sys` is a direct dependency) into `target/<profile>/`, `examples/` and `deps/`. Without it next to the exe, the exe does not start. Never commit the DLL.
- Run clippy for each feature combination you touch: `--no-default-features`, default (`prism`), `wxdragon`. `cfg`-gated code easily produces dead-code or unused-mut warnings in one combination only.
- Feature-gated items (`wx`, `Engine::run_wx`) must not be intra-doc linked from ungated docs. Write them as plain code text.
- Examples open a real window and audio device and run until Escape. For smoke tests, build first and then run the exe with `timeout`, because linking inside `timeout cargo run` gets killed. The windows can steal keyboard focus from the user.
- macOS code (`src/speech/macos.rs`, feature `macos`) is checked from Windows with `PRISM_SYS_NO_NATIVE=1 cargo clippy --target aarch64-apple-darwin --all-targets` (targets are installed). A `.app` is built with `scripts/macos_app.py` via cargo-zigbuild (zig 0.13 from pip, `SDKROOT` pointing to a macOS SDK); see `docs/macos.md`. cargo-zigbuild leaves a `.intentionally-empty-file.o` in the repo root; delete it, never commit it.
- Tests that need an engine use `Engine::new(..).headless().build(scene)`, which gives a silent audio system, console speech and no window, and drive it with `runner.frame()` / `runner.key_down(..)`. UI tests use the `Harness` in `src/ui/mod.rs`.

## Features

- `prism` (default): speech via the `prism` crate from `garo-pro/prism2rust` (git dependency). Its `prism-sys` builds prism C++ with CMake as a shared library (`prism.dll`).
- `macos` (default, macOS only): VoiceOver announcements or `AVSpeechSynthesizer` when prism is unavailable.
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

**Speech** (`src/speech/`). One background thread tries prism (`run_prism`), then macOS (`macos::run`). If all fail, the thread drops the receiver, and `Speech::send` then sees the closed channel and switches to console echo. History and echo live on the game-thread side.

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
- Vehicles (`src/vehicle.rs`) are an optional `Body::vehicle`. `step` drives them first (`step_vehicle`: simulate pedals, gears, fuel, then `move_by`; a blocked move removes the speed into the obstacle and emits `Crashed`). `Context::sync_vehicles` runs once per frame and keeps engine loops and shift/crash sounds in sync.

**Conventions shared across modules** (`src/math.rs`):
- z is up, +y is north, +x is east.
- Angles are compass degrees (0 = north, clockwise). The listener yaw, `math::forward` and the panning in the mixer all rely on this.
- Pitch is degrees up from the horizon (-90..=90). `Body::pitch`, the listener pitch and `math::forward_3d` / `up_3d` use it; with pitch 0 everything behaves as in 2D.
- Game-facing APIs take `f32` seconds, linear volume and string ids for UI items.

## Docs

User docs live in `docs/*.md` (guide per topic), `docs/cheatsheet.md`, the README and `CHANGELOG.md`. `docs/architecture.md` explains the internals (frame order, mixer math, physics algorithm, UI state machine). Keep it accurate when changing those parts. The README's Rust code blocks are compiled as doc tests (`ReadmeDoctests` in `lib.rs`), so they must be complete programs marked `rust,no_run`, and other blocks need a language tag (`text`, `sh`, `toml`). `docs/api-reference/` lists every public item (one page per module, heading = signature, then description and an example). Its examples assume `use audio_code::prelude::*;` plus `ctx`, `player`, `enemy`, `door` and `sensor` in scope. When adding or changing public API, update the matching guide page, the API reference page and the changelog.
