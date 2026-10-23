# Changelog

## 0.1.0 – unreleased

First version.

- `Engine`, `Scene`, `Context`: game loop with a scene stack and shared data
- Audio: real-time mixer, 3D positioned sounds (panning, distance, muffling behind the
  listener), looping, pitch, fades, music with cross fades and background loading, volume
  groups, file lookup with caching, generated sounds
- Speech via prism (`prism` feature, default on, uses the `prismer` bindings, linked statically) with console fallback, message history and braille
- Virtual UI: menus (buttons, checkboxes, sliders, choices, text fields, labels), messages,
  yes/no dialogs, text input, built-in interface sounds, custom widgets
- Physics: boxes and spheres; fixed, dynamic, kinematic and sensor bodies; gravity,
  collision and sensor events, `move_by`, ray casts and spatial queries
- Time: delta, time scale and pause, named scheduler, `Timer`, `Cooldown`, `Interval`
- Input: pressed / held / released, custom key repeat, typed text, layout aware letters
- winit window backend, optional wxDragon backend (`wxdragon` feature)
- macOS support: speech through VoiceOver announcements or the system voice
  (`AVSpeechSynthesizer`) without prism (`macos` feature, default on)
- Voices: `Speech::voices`, `set_voice`, `reset_voice`, `voice` and the `Voice` type
- macOS app bundles: sounds are also found in `Contents/Resources`, saves go to
  `~/Library/Application Support/<title>/`; `scripts/macos_app.py` builds a universal `.app`
- `bell_catcher` example: a small complete game with a voice menu
- `Synth` for designing clicks and effects in code; `Audio::register` accepts synths
- Custom UI sounds: `Ui::set_sound`, `Ui::set_sound_data`, `UiSounds::with/without`,
  `Menu::sound`, new `Wrap` and `Typing` sound slots
- Physics: `EARTH_GRAVITY`, `Engine::gravity`, `World::jump`, `World::apply_force`, mass and
  momentum exchange, terminal velocity (`max_speed`), `Landed` event, pushable bodies and
  moving platforms that carry bodies
- Easier game code:
  - `audio_code::run(title, closure)` / `Engine::run_fn` for games without a scene struct
  - shortcuts on `Context`: `speak`, `play`, `play_at`, `pressed`, `held`, `once`
  - `ctx.follow(body)`: the listener follows a body's position and facing
  - `ctx.loop_on`, `ctx.play_on`, `ctx.attach`: sounds follow bodies and stop when removed
  - levels from text maps: `World::build_level`, `Level::point(s)`
  - `Body::facing`, `World::turn`, `move_forward`, `strafe`, `describe`, `distance`
  - event shortcuts: `landed`, `collided`, `entered`, `entered_tag`, `left`, `triggered`
  - `Input::axis` and `Input::movement`
  - `ctx.storage`: key-value saving, written on quit (`Engine::save_file`)
  - cheat sheet in `docs/cheatsheet.md`
- Detailed README with an explained example game, and `docs/architecture.md` explaining
  the internals; README code blocks are compiled as doc tests
- Synthetic key presses on window focus are ignored
- Vehicles (`audio_code::vehicle`, optional per body): `Body::car`, `Body::truck`,
  `Body::with_vehicle` and `Vehicle` with weight and cargo (`load`, `max_load`), top and
  reverse speed, engine and brake force, grip, rolling resistance, engine braking and air
  drag, speed dependent steering, automatic gears with rpm and `engine_pitch`, handbrake,
  reverse on the brake pedal, engine on/off, fuel tank and odometer; `ctx.drive`,
  `ctx.horn`, `ctx.start_engine`, `ctx.stop_engine`; automatic engine, shift and crash
  sounds; `World::vehicle`, `vehicle_mut`, `weight`, `speed`, `crashed`, `gear_changed`;
  events `PhysicsEvent::Crashed` and `PhysicsEvent::GearChanged`; `driving` example and
  `docs/vehicles.md`
- Optional save encryption: `Engine::encrypt_save(secret)`, `Storage::open_encrypted`,
  `Storage::set_encryption`, `Storage::is_encrypted` (XChaCha20-Poly1305, key from the
  secret via SHA-256). Plain saves are converted on the next save; unreadable files are kept
  as `<file>.broken` and reported by `Storage::load_error`
- API reference in `docs/api-reference/`: every public function with its signature, defaults
  and an example, one page per module
- Menu music: `Menu::music` loops a track while the menu is open, submenus keep it, and the
  previous music comes back when it closes; `Widget::music` for custom widgets,
  `Audio::music_name`
- Menus no longer say "2 of 5" by default; turn it on with `Menu::positions(true)`
- Docs explain what the numbers mean: units, defaults and typical values for body settings,
  sound volume, pitch, distance and rolloff (with tables), synth frequency and decay, menu
  sliders and choices; the physics guide explains body kinds, which setup to use for top
  down games and platformers, the ways to move a body and what happens in one step
- Full 3D: bodies have a `pitch` (look up or down) with `World::tilt`, `set_pitch`,
  `look_at` and `move_up`; `move_forward` follows the pitch; `Audio::set_listener_3d` and
  `listener_pitch`, and `ctx.follow` passes the pitch on; sounds below the listener are a
  little dull and quieter, sounds above stay clear; `describe` adds heights ("4 meters
  above", "directly below"); `math::forward_3d`, `up_3d`, `elevation`, `look_at`;
  `Input::movement_3d`; `MoveResult` and `RayHit` in the prelude; new `flight` example
- `docs/3d.md` explains what `vec3(x, y, z)` means, directions, angles and 3D games
