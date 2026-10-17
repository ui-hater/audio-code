# audio-code

A simple, high level game engine for **audio games**, written in Rust.

Audio games are played by ear, and you can play them without seeing the screen. audio-code
gives you what they need in one crate, and a full game fits in a few dozen lines:

- **Speech** through the player's screen reader (NVDA, JAWS, VoiceOver, Orca, ...) or the
  system voice, via [prism](https://github.com/ethindp/prism), with NVDA's controller
  client as a fallback on Windows
- **Sound**: play effects in one line, place 3D sounds in the world, loop them, change their
  pitch, and cross fade music. Files are found and cached for you.
- **Virtual UI**: menus that speak, with buttons, sliders, checkboxes, choices, text fields,
  message boxes and yes/no dialogs. They need no graphics and work from the keyboard only.
  Every click sound can be replaced with your own file or a sound designed with the synth.
- **Synth**: design clicks, beeps and effects in code
- **Physics**: walls, floors, gravity, jumping, mass, forces, pushable objects, moving
  platforms, landing events, trigger zones and ray casts
- **Time**: frame delta, a scheduler (`after`, `every`), timers, cooldowns and intervals
- **Input**: pressed / held / released keys, key repeat with your own timing, and typed text
- **Scenes**: main menu, levels and pause screens as a simple stack
- **wxWidgets** integration through [wxDragon](https://crates.io/crates/wxdragon) (optional)

```rust
use audio_code::prelude::*;

struct Game;

impl Scene for Game {
    fn start(&mut self, ctx: &mut Context) {
        ctx.speech.speak("Press space to fire, escape to quit.");
        ctx.audio.play_music("music/theme");
    }

    fn update(&mut self, ctx: &mut Context) {
        if ctx.input.pressed(Key::Space) {
            ctx.audio.play_at("laser", vec3(2.0, 5.0, 0.0));
        }
        if ctx.input.pressed(Key::Escape) {
            ctx.quit();
        }
    }
}

fn main() -> audio_code::Result<()> {
    Engine::new("My first audio game").run(Game)
}
```

## Installation

```toml
[dependencies]
audio-code = { git = "https://github.com/<you>/audio-code" }
```

### Requirements

Speech uses the prism C++ library, which is built from source automatically. You need:

- **CMake 3.24+** and a **C++23 compiler**
- **Windows:** Visual Studio Build Tools with the components *Desktop development with C++*
  **and "C++ ATL for latest build tools"**. Without ATL the build fails with
  `atlbase.h: No such file or directory`.
- **Linux:** `pkg-config` and `libasound2-dev` (audio); for speech, glibmm 2.68+
  (`libglibmm-2.68-dev`) and speech-dispatcher 0.11.1+ (`libspeechd-dev`). The
  [prism README](https://github.com/ethindp/prism) has the details.
- **macOS:** Xcode command line tools

To build without prism and its C++ build, turn off default features. On Windows you can
keep speech through NVDA's controller client DLL (see [Speech](docs/speech.md#nvda-controller-client)):

```toml
audio-code = { git = "...", default-features = false, features = ["nvda"] }
```

### Feature flags

| Feature    | Default | What it does                                              |
|------------|---------|-----------------------------------------------------------|
| `prism`    | yes     | Screen reader and TTS output via prism.                   |
| `nvda`     | yes     | Windows: speak through `nvdaControllerClient.dll` if prism is unavailable. |
| `wxdragon` | no      | `Engine::run_wx` and `audio_code::wx` for wxWidgets windows. |

Without any speech output, speech is printed to the console.

## Examples

All examples generate their sounds in code, so they work without any audio files.

| Example      | Shows                                                     | Run                                              |
|--------------|-----------------------------------------------------------|--------------------------------------------------|
| `hello`      | speech, beeps, quitting                                    | `cargo run --example hello`                      |
| `menus`      | main menu, options sub menu, text input, confirm dialog    | `cargo run --example menus`                      |
| `beacon`     | 3D sound, walls, sensors, pause scene                      | `cargo run --example beacon`                     |
| `platformer` | gravity, jumping, landing sounds, collecting coins         | `cargo run --example platformer`                 |
| `wx_game`    | a game inside a wxWidgets window next to a native button   | `cargo run --example wx_game --features wxdragon`|

## Documentation

- [Getting started](docs/getting-started.md): your first game, step by step
- [Sound](docs/audio.md): playing, positioning, music, file lookup
- [Speech](docs/speech.md): screen readers, history, braille
- [Virtual UI](docs/ui.md): menus, dialogs, text input, custom widgets
- [Physics](docs/physics.md): bodies, collisions, sensors, ray casts
- [Time and input](docs/time-and-input.md): delta time, scheduler, timers, keys
- [Scenes and shared data](docs/scenes.md)
- [wxDragon integration](docs/wxdragon.md)

API reference: `cargo doc --open`.

## Design principles

1. **One line for common things.** `ctx.audio.play("door")` finds `sounds/door.ogg`,
   decodes and caches it, and plays it.
2. **No crashes for small mistakes.** A missing sound prints a warning once and plays
   nothing.
3. **Accessible by default.** Every UI element speaks, and the interface sounds work without
   any files.
4. **Plain Rust.** There are no macros and no entity component system. Scenes are structs and
   data lives in your fields.

## License

MIT, see [LICENSE](LICENSE).
