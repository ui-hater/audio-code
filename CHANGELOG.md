# Changelog

## 0.1.0 – unreleased

First version.

- `Engine`, `Scene`, `Context`: game loop with a scene stack and shared data
- Audio: real-time mixer, 3D positioned sounds (panning, distance, muffling behind the
  listener), looping, pitch, fades, music with cross fades and background loading, volume
  groups, file lookup with caching, generated sounds
- Speech via prism with console fallback, message history and braille
- Virtual UI: menus (buttons, checkboxes, sliders, choices, text fields, labels), messages,
  yes/no dialogs, text input, built-in interface sounds, custom widgets
- Physics: boxes and spheres; fixed, dynamic, kinematic and sensor bodies; gravity,
  collision and sensor events, `move_by`, ray casts and spatial queries
- Time: delta, time scale and pause, named scheduler, `Timer`, `Cooldown`, `Interval`
- Input: pressed / held / released, custom key repeat, typed text, layout aware letters
- winit window backend, optional wxDragon backend (`wxdragon` feature)
- Speech fallback to NVDA's controller client DLL on Windows (`nvda` feature, default on),
  `Engine::nvda_dll`
- `Synth` for designing clicks and effects in code; `Audio::register` accepts synths
- Custom UI sounds: `Ui::set_sound`, `Ui::set_sound_data`, `UiSounds::with/without`,
  `Menu::sound`, new `Wrap` and `Typing` sound slots
- Physics: `EARTH_GRAVITY`, `Engine::gravity`, `World::jump`, `World::apply_force`, mass and
  momentum exchange, terminal velocity (`max_speed`), `Landed` event, pushable bodies and
  moving platforms that carry bodies
