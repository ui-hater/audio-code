# API reference

A lookup reference for every public item of `audio-code`: each function with its
signature, what it does, its defaults, and a short example underneath. It is written for
people who want to check how a call looks, and for AI assistants that write code against
the engine.

The topic guides (`docs/*.md`) explain concepts and walk through whole games. This folder
is the dictionary next to them: short, complete and in a fixed format.

## Pages

| Page | Covers |
|------|--------|
| [engine.md](engine.md) | `audio_code::run`, `Engine` (configuration and start), `Scene`, `Runner`, `Error`, `Result`, the prelude |
| [context.md](context.md) | `Context`: fields, quitting, scene changes, shared data, shortcuts (`speak`, `play`, `once`, `follow`, `loop_on`, ...) |
| [audio.md](audio.md) | `Audio` (`ctx.audio`), `SoundBuilder`, `Sound`, `Group`, `SoundData`, `Synth`, `Wave` |
| [speech.md](speech.md) | `Speech` (`ctx.speech`) |
| [input.md](input.md) | `Input` (`ctx.input`), `Key`, `Modifiers`, `RawInput` |
| [time.md](time.md) | `Time` (`ctx.time`), `Timer`, `Cooldown`, `Interval` |
| [physics.md](physics.md) | `World` (`ctx.physics`), `Body`, `BodyKind`, `Shape`, `BodyId`, `PhysicsEvent`, `RayHit`, `MoveResult`, `EARTH_GRAVITY` |
| [level.md](level.md) | `World::build_level`, `Level` |
| [ui.md](ui.md) | `Ui` (`ctx.ui`), `Menu`, `MenuItem`, `ItemKind`, `Message`, `Confirm`, `TextInput`, `Value`, `UiEvent`, `UiSound`, `UiSounds`, `Widget`, `Status`, `UiContext` |
| [math.md](math.md) | `Vec2`, `Vec3`, `vec2`, `vec3`, angle and direction helpers, decibels, `math::random` |
| [storage.md](storage.md) | `Storage` (`ctx.storage`) |
| [wx.md](wx.md) | wxDragon backend (feature `wxdragon`): `Engine::run_wx`, `GameHost` |

## How to read an entry

Every entry has the same shape:

````text
### `receiver.method(arg: Type, ...) -> Return`

What it does, defaults, edge cases.

```rust
// example
```
````

- The receiver shows where the function is reached: `ctx.audio.play(..)` is a method of
  `Audio`, reached through the `audio` field of the `Context`. `Body::dynamic(..)` is an
  associated function (a constructor). `sound.stop()` is a method on a value you hold.
- Parameters written as `impl Into<String>` accept `&str` and `String`. `impl AsRef<str>`
  accepts `&str`, `String` and `format!(..)` results.
- Builder methods (`Engine`, `SoundBuilder`, `Body`, `Menu`, `Synth`, ...) take `self` and
  return `Self`, so they are chained.

## Assumptions in the examples

Examples are snippets, not whole programs, unless they contain `fn main`. They assume:

```rust
use audio_code::prelude::*;
```

and that they run inside a scene method or a closure game, where these names exist:

| Name | Type | Meaning |
|------|------|---------|
| `ctx` | `&mut Context` | the engine context passed to every scene method |
| `player` | `BodyId` | a body created earlier with `ctx.physics.add(..)` |
| `enemy`, `door` | `BodyId` | other bodies |
| `sensor` | `BodyId` | a body created with `Body::sensor(..)` |

Items that are not in the prelude are imported with their full path in the example
(`use audio_code::audio::SoundBuilder;`).

## Units and conventions (apply everywhere)

| Quantity | Unit |
|----------|------|
| Time | seconds, `f32` (game time totals are `f64`) |
| Distance, position | meters |
| Speed | meters per second |
| Angles (yaw, facing, bearing) | compass degrees: 0 = north, 90 = east, 180 = south, 270 = west, positive turns right |
| Pitch | degrees above the horizon: 0 = level, 90 = straight up, -90 = straight down |
| Volume | linear factor: 1.0 = unchanged, 0.5 = half, 0.0 = silent |
| Playback pitch | speed factor: 1.0 = original, 2.0 = one octave up |
| Frequency | hertz |
| Mass / force | kilograms / newtons |

Axes: `+x` east, `+y` north, `+z` up. `vec3(3.0, -2.0, 1.5)` is 3 m east, 2 m south and
1.5 m up.

Robustness rules that hold for the whole API:

- A missing or broken sound file prints a warning once and returns a silent `Sound`
  handle. It never panics.
- Methods that take a `BodyId` of a removed body do nothing and return a neutral value
  (`Vec3::ZERO`, `0.0`, `""`, `None`, `false`).
- Methods on a `Sound` that has finished do nothing.
- While a UI widget is open, the game-facing `ctx.input` queries return `false` / empty.

## Quick index

Where to find a function by name:

| Name | Page |
|------|------|
| `run`, `Engine::*`, `Scene`, `Runner`, `Error`, `Result` | [engine.md](engine.md) |
| `ctx.quit`, `ctx.switch_scene`, `ctx.push_scene`, `ctx.pop_scene`, `ctx.insert`, `ctx.get`, `ctx.get_mut`, `ctx.get_or_default`, `ctx.remove`, `ctx.title` | [context.md](context.md) |
| `ctx.speak`, `ctx.play`, `ctx.play_at`, `ctx.pressed`, `ctx.held`, `ctx.once`, `ctx.follow`, `ctx.unfollow`, `ctx.followed`, `ctx.play_on`, `ctx.loop_on`, `ctx.attach`, `ctx.stop_sounds_on` | [context.md](context.md) |
| `play`, `play_at`, `play_looped`, `play_looped_at`, `play_random`, `play_tone`, `sound(..)...play()`, `play_music`, `stop_music`, `set_listener`, `set_master_volume`, `set_group_volume`, `register`, `preload` | [audio.md](audio.md) |
| `Sound::stop`, `fade_out`, `set_volume`, `fade_to`, `set_pitch`, `set_position`, `pause`, `resume`, `seek` | [audio.md](audio.md#sound) |
| `Synth::sine`, `square`, `triangle`, `saw`, `noise`, `then`, `with`, `decay` | [audio.md](audio.md#synth) |
| `speak`, `speak_queued`, `stop`, `repeat_last`, `braille`, `history` | [speech.md](speech.md) |
| `pressed`, `held`, `released`, `repeating`, `axis`, `movement`, `movement_3d`, `text`, `modifiers`, `raw` | [input.md](input.md) |
| `delta`, `elapsed`, `set_scale`, `pause`, `after`, `every`, `fired`, `Timer`, `Cooldown`, `Interval` | [time.md](time.md) |
| `add`, `remove`, `move_by`, `move_forward`, `strafe`, `move_up`, `turn`, `tilt`, `jump`, `raycast`, `entered_tag`, `landed`, `describe`, `Body::*` | [physics.md](physics.md) |
| `build_level`, `Level::point`, `Level::points` | [level.md](level.md) |
| `ctx.ui.open`, `message`, `confirm`, `input`, `clicked`, `changed`, `confirmed`, `submitted`, `Menu::*` | [ui.md](ui.md) |
| `forward`, `right`, `bearing`, `describe_position`, `direction_name`, `clock_position`, `look_at`, `random::*` | [math.md](math.md) |
| `ctx.storage.get`, `get_or`, `set`, `add`, `save` | [storage.md](storage.md) |
