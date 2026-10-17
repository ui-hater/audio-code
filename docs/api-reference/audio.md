# Audio

Sound effects, 3D positioned sounds, music, volume groups and sounds generated in code.
The audio system is `ctx.audio` (type `Audio`). Playing returns a `Sound` handle.

```rust
use audio_code::prelude::*;                // Audio, Sound, Group, SoundData, Synth, Wave
use audio_code::audio::SoundBuilder;       // only if you name the builder type
```

## How sound names are found

Every function that takes a `name: &str` resolves it like this:

1. The name as a path (`"sounds/step.ogg"`, `"C:/sfx/step.wav"`).
2. The name inside the sound folder (`Engine::sound_dir`, default `"sounds"`).
3. If the name has no extension: `.ogg`, `.wav`, `.mp3`, `.flac`, `.opus`, `.m4a` are tried,
   in that order, for both of the above.

So `ctx.audio.play("step")` finds `sounds/step.ogg`, and `"ui/click"` finds
`sounds/ui/click.wav`. Names registered with `register` (and generated tones) are looked up
first. Loaded sounds are cached by the name string. A missing or broken file prints one
warning and returns a silent handle.

## Playing

### `ctx.audio.play(name: &str) -> Sound`

Plays a sound once, centered (not positioned). The handle can be ignored.

```rust
ctx.audio.play("door_open");
```

### `ctx.audio.play_at(name: &str, position: Vec3) -> Sound`

Plays a sound once at a position in the world. It is panned, made quieter with distance
and muffled when behind the listener.

```rust
ctx.audio.play_at("splash", vec3(-3.0, 4.0, 0.0));
```

### `ctx.audio.play_looped(name: &str) -> Sound`

Plays a sound in an endless loop, centered. Keep the handle to stop it.

```rust
let rain = ctx.audio.play_looped("ambience/rain");
// later
rain.fade_out(2.0);
```

### `ctx.audio.play_looped_at(name: &str, position: Vec3) -> Sound`

Plays a looping sound at a position. For a sound that moves with a body, use
`ctx.loop_on` instead ([context.md](context.md#shortcuts)).

```rust
let fire = ctx.audio.play_looped_at("campfire", vec3(5.0, 5.0, 0.0));
```

### `ctx.audio.play_random(names: &[&str]) -> Sound`

Plays one of several sounds, picked at random. An empty slice plays nothing.

```rust
ctx.audio.play_random(&["step1", "step2", "step3"]);
```

### `ctx.audio.play_random_at(names: &[&str], position: Vec3) -> Sound`

Like `play_random`, at a position.

```rust
let at = ctx.physics.position(enemy);
ctx.audio.play_random_at(&["hit1", "hit2"], at);
```

### `ctx.audio.play_tone(frequency: f32, seconds: f32) -> Sound`

Plays a generated sine beep: `frequency` in hertz, `seconds` long, at volume 0.4. For
prototyping without sound files. Tones are cached by frequency and length.

```rust
ctx.audio.play_tone(880.0, 0.1);
```

### `ctx.audio.sound(name: &str) -> SoundBuilder<'_>`

Starts a builder with full control over the settings. Chain settings, then call `.play()`.
Without `.play()` nothing happens (the compiler warns).

```rust
ctx.audio
    .sound("explosion")
    .volume(0.8)
    .random_pitch(0.9, 1.1)
    .at(vec3(3.0, 0.0, 0.0))
    .distance(2.0, 60.0)
    .play();
```

## SoundBuilder

Created by `ctx.audio.sound(name)`. Each method returns the builder. Defaults: volume 1.0,
pitch 1.0, pan 0.0, not looped, not positioned, distance and rolloff from
`set_default_distance` / `set_default_rolloff`, group `Group::Sound`, no fade, start at 0.

### `builder.volume(volume: f32) -> SoundBuilder`

Linear volume: 1.0 as recorded, 0.5 half (about -6 dB), 0.0 silent. Above 1.0 amplifies
and may distort.

```rust
ctx.audio.sound("wind").volume(0.3).looped().play();
```

### `builder.volume_db(db: f32) -> SoundBuilder`

Volume in decibels: 0 original, -6 about half, -20 a tenth, +6 about double.

```rust
ctx.audio.sound("alarm").volume_db(-12.0).play();
```

### `builder.pitch(pitch: f32) -> SoundBuilder`

Playback speed, which changes pitch and length together: 1.0 original, 2.0 one octave up
and twice as fast, 0.5 one octave down. Clamped to 0.01..=16.

```rust
ctx.audio.sound("voice").pitch(0.8).play();
```

### `builder.random_pitch(min: f32, max: f32) -> SoundBuilder`

Picks a random pitch between `min` and `max`. `0.9, 1.1` keeps repeated footsteps and hits
from sounding identical.

```rust
ctx.audio.sound("step").random_pitch(0.9, 1.1).play();
```

### `builder.pan(pan: f32) -> SoundBuilder`

Stereo position for non-positioned sounds: -1.0 left, 0.0 center, 1.0 right. Ignored once
the sound has a position (`at`).

```rust
ctx.audio.sound("tick").pan(-1.0).play();
```

### `builder.looped() -> SoundBuilder`

Loops endlessly until stopped.

```rust
let hum = ctx.audio.sound("hum").looped().play();
```

### `builder.at(position: Vec3) -> SoundBuilder`

Places the sound in the world, so it is spatialized relative to the listener.

```rust
ctx.audio.sound("bird").at(vec3(0.0, 10.0, 4.0)).play();
```

### `builder.distance(min: f32, max: f32) -> SoundBuilder`

Audible range of a positioned sound: full volume up to `min` meters, silent from `max`
meters. Default 1.0 and 30.0.

```rust
ctx.audio.sound("thunder").at(vec3(50.0, 0.0, 20.0)).distance(10.0, 200.0).play();
```

### `builder.rolloff(rolloff: f32) -> SoundBuilder`

How fast volume drops between `min` and `max` distance. 1.0 is physically accurate (twice
the distance, half the volume), lower values keep distant sounds louder, 0.0 keeps full
volume until shortly before `max`. Default 0.7.

```rust
ctx.audio.sound("beacon").at(vec3(20.0, 0.0, 0.0)).rolloff(0.3).looped().play();
```

### `builder.group(group: Group) -> SoundBuilder`

Volume group. Default `Group::Sound`.

```rust
ctx.audio.sound("click").group(Group::Ui).play();
```

### `builder.fade_in(seconds: f32) -> SoundBuilder`

Fades in from silence to the volume over `seconds`.

```rust
ctx.audio.sound("ambience/cave").looped().fade_in(3.0).play();
```

### `builder.start_at(seconds: f32) -> SoundBuilder`

Starts playback this many seconds into the sound.

```rust
ctx.audio.sound("speech/intro").start_at(4.5).play();
```

### `builder.paused() -> SoundBuilder`

Creates the sound paused. Start it later with `sound.resume()`.

```rust
let ready = ctx.audio.sound("countdown").paused().play();
// later
ready.resume();
```

### `builder.play() -> Sound`

Plays the sound with the chosen settings and returns its handle.

```rust
let s: Sound = ctx.audio.sound("door").volume(0.5).play();
```

## Sound

A handle to a playing sound. Cheap to clone. Dropping it does **not** stop the sound.
Every method is safe to call after the sound finished; it then does nothing.
`Sound::default()` equals `Sound::none()`.

### `Sound::none() -> Sound`

A handle that refers to no sound. Placeholder for struct fields.

```rust
struct Car {
    engine: Sound,
}
let mut car = Car { engine: Sound::none() };
car.engine = ctx.audio.play_looped("engine");
```

### `sound.is_playing() -> bool`

`true` while the sound plays, is paused, or is still loading (music).

```rust
let speech = ctx.audio.play("narration/chapter1");
if !speech.is_playing() {
    ctx.speak("Chapter one is over");
}
```

### `sound.is_finished() -> bool`

`true` once the sound ended or was stopped. The opposite of `is_playing`.

```rust
let s = ctx.audio.play("door");
if s.is_finished() {
    ctx.play("lock");
}
```

### `sound.position() -> f32`

Current playback position in seconds.

```rust
let music = ctx.audio.play_music("music/theme");
let seconds = music.position();
```

### `sound.stop()`

Stops immediately.

```rust
let s = ctx.audio.play_looped("alarm");
s.stop();
```

### `sound.fade_out(seconds: f32)`

Fades to silence over `seconds`, then stops. Ends loops without a click.

```rust
let s = ctx.audio.play_looped("engine");
s.fade_out(0.5);
```

### `sound.set_volume(volume: f32)`

Sets the linear volume immediately.

```rust
let s = ctx.audio.play_looped("radio");
s.set_volume(0.2);
```

### `sound.fade_to(volume: f32, seconds: f32)`

Fades the volume to `volume` over `seconds`. The sound keeps playing, also at 0.0.

```rust
let s = ctx.audio.play_looped("crowd");
s.fade_to(0.1, 2.0);
```

### `sound.set_pitch(pitch: f32)`

Changes playback speed and pitch. Can be called every frame, for example for an engine
that follows the speed.

```rust
let engine = ctx.audio.play_looped("engine");
let speed = ctx.physics.velocity(player).length();
engine.set_pitch(0.8 + speed / 20.0);
```

### `sound.set_pan(pan: f32)`

Stereo panning for non-positioned sounds, -1.0 left to 1.0 right.

```rust
let s = ctx.audio.play_looped("metronome");
s.set_pan(0.5);
```

### `sound.set_position(position: Vec3)`

Moves a positioned sound (one created with `at`, `play_at` or `play_looped_at`).

```rust
let drone = ctx.audio.play_looped_at("drone", Vec3::ZERO);
drone.set_position(vec3(3.0, 4.0, 2.0));
```

### `sound.set_distance(min: f32, max: f32)`

Changes the audible range of a positioned sound.

```rust
let s = ctx.audio.play_looped_at("siren", vec3(30.0, 0.0, 0.0));
s.set_distance(5.0, 100.0);
```

### `sound.pause()`

Pauses playback. The position is kept.

```rust
let music = ctx.audio.play_music("music/level");
music.pause();
```

### `sound.resume()`

Continues after `pause`, or starts a sound created with `.paused()`.

```rust
let music = ctx.audio.play_music("music/level");
music.resume();
```

### `sound.set_looping(looping: bool)`

Turns looping on or off while playing. Turning it off lets the sound end after the current
pass.

```rust
let s = ctx.audio.play_looped("bell");
s.set_looping(false);
```

### `sound.seek(seconds: f32)`

Jumps to a position in seconds.

```rust
let s = ctx.audio.play("audiobook");
s.seek(60.0);
```

## Music

Music plays in a loop in `Group::Music`. Files are decoded on a background thread, so
starting music never stutters. There is one current music track.

### `ctx.audio.play_music(name: &str) -> Sound`

Plays background music in a loop, cross fading over 1 second from any previous music.
Calling it again with the music that is already playing does nothing and returns the same
handle, so it is safe to call every frame.

```rust
ctx.audio.play_music("music/level1");
```

### `ctx.audio.play_music_with_fade(name: &str, fade: f32) -> Sound`

Like `play_music` with a custom cross fade time in seconds.

```rust
ctx.audio.play_music_with_fade("music/boss", 3.0);
```

### `ctx.audio.stop_music(fade: f32)`

Fades out the current music over `fade` seconds.

```rust
ctx.audio.stop_music(2.0);
```

### `ctx.audio.music() -> Option<&Sound>`

Handle of the current music, if any.

```rust
if let Some(music) = ctx.audio.music() {
    music.fade_to(0.3, 1.0);
}
```

### `ctx.audio.music_name() -> Option<&str>`

Name of the current music as given to `play_music`.

```rust
if ctx.audio.music_name() != Some("music/menu") {
    ctx.audio.play_music("music/menu");
}
```

## Listener

The listener is the player's ears: a position, a compass direction (yaw) and a pitch.
`ctx.follow(body)` sets it every frame for you.

### `ctx.audio.set_listener(position: Vec3, yaw: f32)`

Places the listener at `position`, facing compass direction `yaw` (0 = north, 90 = east),
looking level.

```rust
ctx.audio.set_listener(vec3(5.0, 5.0, 0.0), 90.0);
```

### `ctx.audio.set_listener_3d(position: Vec3, yaw: f32, pitch: f32)`

Like `set_listener`, plus `pitch` in degrees (positive up, 90 straight up), clamped to
-90..=90. For flying, swimming and space games.

```rust
ctx.audio.set_listener_3d(vec3(0.0, 0.0, 10.0), 0.0, -30.0);
```

### `ctx.audio.listener_position() -> Vec3`

Current listener position.

```rust
let ears = ctx.audio.listener_position();
```

### `ctx.audio.listener_yaw() -> f32`

Current listener compass direction in degrees.

```rust
let facing = ctx.audio.listener_yaw();
ctx.speak(math::direction_name(facing));
```

### `ctx.audio.listener_pitch() -> f32`

Current listener pitch in degrees.

```rust
let looking_up = ctx.audio.listener_pitch() > 0.0;
```

### `ctx.audio.set_default_distance(min: f32, max: f32)`

Default audible range for positioned sounds started afterwards: full volume up to `min`
meters, silent from `max`. Default 1.0 and 30.0.

```rust
ctx.audio.set_default_distance(2.0, 50.0);
```

### `ctx.audio.set_default_rolloff(rolloff: f32)`

Default rolloff for positioned sounds started afterwards. Default 0.7 (32 % volume at 5 m,
18 % at 10 m). See `builder.rolloff`.

```rust
ctx.audio.set_default_rolloff(1.0);
```

## Volume

The heard volume of a sound is: its own volume × its group volume × master volume.

### `ctx.audio.set_master_volume(volume: f32)`

Overall volume as a linear factor. Default 1.0.

```rust
ctx.audio.set_master_volume(0.8);
```

### `ctx.audio.master_volume() -> f32`

Current overall volume.

```rust
let percent = (ctx.audio.master_volume() * 100.0).round();
```

### `ctx.audio.set_group_volume(group: Group, volume: f32)`

Volume of one group as a linear factor. Default 1.0 for every group.

```rust
ctx.audio.set_group_volume(Group::Ui, 0.5);
```

### `ctx.audio.group_volume(group: Group) -> f32`

Current volume of one group.

```rust
let music = ctx.audio.group_volume(Group::Music);
```

### `ctx.audio.set_music_volume(volume: f32)`

Shortcut for `set_group_volume(Group::Music, volume)`.

```rust
ctx.audio.set_music_volume(0.4);
```

### `ctx.audio.set_sound_volume(volume: f32)`

Shortcut for `set_group_volume(Group::Sound, volume)`.

```rust
ctx.audio.set_sound_volume(0.9);
```

### `ctx.audio.stop_all()`

Stops every sound immediately, including music.

```rust
ctx.audio.stop_all();
```

### `ctx.audio.stop_group(group: Group, fade: f32)`

Fades out all sounds of a group over `fade` seconds and stops them.

```rust
ctx.audio.stop_group(Group::Sound, 0.5);
```

## Group

```rust
pub enum Group {
    Sound,   // sound effects (default for every played sound)
    Music,   // background music (play_music uses it)
    Ui,      // menu and interface sounds (the UI uses it)
}
```

```rust
ctx.audio.sound("menu_tick").group(Group::Ui).play();
```

## Loading and cache

### `ctx.audio.preload(name: &str) -> Result<()>`

Loads a sound into the cache now, so the first play has no delay. Unlike playing, it
returns the error instead of only warning.

```rust
if let Err(e) = ctx.audio.preload("explosion") {
    eprintln!("{e}");
}
```

### `ctx.audio.preload_dir(dir: impl AsRef<Path>) -> Result<usize>`

Loads every sound file in a folder inside the sound folder (not recursive). Names are the
paths relative to the sound folder without extension, with `/` as separator. Returns how
many files were loaded.

```rust
// loads sounds/steps/grass1.ogg as "steps/grass1", ...
let count = ctx.audio.preload_dir("steps").unwrap_or(0);
```

### `ctx.audio.register(name: impl Into<String>, data: impl Into<SoundData>)`

Registers sound data made in code (a `SoundData` or a `Synth`) under a name, so it plays
like a file. Registering a name again replaces the sound.

```rust
ctx.audio.register("beep", SoundData::tone(880.0, 0.1, 0.5));
ctx.audio.register("click", Synth::sine(1500.0).length(0.03).decay(120.0));
ctx.audio.play("click");
```

### `ctx.audio.is_loaded(name: &str) -> bool`

Whether a sound with this name is in the cache (loaded or registered).

```rust
if !ctx.audio.is_loaded("beep") {
    ctx.audio.register("beep", SoundData::tone(880.0, 0.1, 0.5));
}
```

### `ctx.audio.unload(name: &str)`

Removes a sound from the cache to free memory. Sounds that are playing keep playing.

```rust
ctx.audio.unload("music/intro");
```

### `ctx.audio.duration(name: &str) -> Option<f32>`

Length of a sound in seconds. Loads it if needed. `None` if it cannot be loaded (or music
is still decoding).

```rust
if let Some(len) = ctx.audio.duration("narration/intro") {
    ctx.time.after("intro_done", len);
}
```

### `ctx.audio.set_sound_dir(dir: impl Into<PathBuf>)`

Changes the folder searched for sounds. Already cached names stay cached.

```rust
ctx.audio.set_sound_dir("mods/my_mod/sounds");
```

### `ctx.audio.sample_rate() -> u32`

Sample rate of the output device in Hz (44100 when silent).

```rust
let rate = ctx.audio.sample_rate();
```

### `ctx.audio.has_output() -> bool`

Whether an audio device is open. `false` means sounds are silently ignored (no device, or
headless).

```rust
if !ctx.audio.has_output() {
    ctx.speak("No sound device found");
}
```

### `Audio::silent(sound_dir: impl Into<PathBuf>) -> Audio`

An audio system without an output device. Everything works, nothing is heard. The engine
uses it when no device opens and in headless mode.

```rust
let audio = Audio::silent("sounds");
assert!(!audio.has_output());
```

## SoundData

Decoded audio in memory: interleaved `f32` samples in -1.0..=1.0. You only need it to make
sounds in code or to decode a file yourself. Register it with `ctx.audio.register`.

### `SoundData::tone(frequency: f32, seconds: f32, volume: f32) -> SoundData`

A sine tone with short fades. `frequency` in hertz (440 = A), `volume` linear 0.0..=1.0.

```rust
let a = SoundData::tone(440.0, 0.5, 0.4);
ctx.audio.register("a4", a);
```

### `SoundData::generate(seconds: f32, f: impl FnMut(f32) -> f32) -> SoundData`

Mono sound at 44100 Hz from a function of time in seconds. Return samples in -1.0..=1.0.
5 ms fades are added at both ends against clicks.

```rust
// A siren: the frequency wobbles between 600 and 1000 Hz.
let mut phase = 0.0f32;
let siren = SoundData::generate(2.0, move |t| {
    let freq = 800.0 + 200.0 * (t * 3.0 * std::f32::consts::TAU).sin();
    phase += freq / 44_100.0;
    (phase * std::f32::consts::TAU).sin() * 0.4
});
ctx.audio.register("siren", siren);
```

### `SoundData::from_samples(samples: Vec<f32>, channels: usize, sample_rate: u32) -> SoundData`

Wraps interleaved samples. Panics if `channels` or `sample_rate` is 0.

```rust
let silence = SoundData::from_samples(vec![0.0; 44_100 * 2], 2, 44_100);
assert_eq!(silence.duration(), 1.0);
```

### `SoundData::load(path: impl AsRef<Path>) -> Result<SoundData>`

Loads and decodes a file (wav, ogg, mp3, flac, aiff, m4a/aac, ...). Does not use the sound
folder or the cache; `ctx.audio.play` does that for you.

```rust
let data = SoundData::load("sounds/door.ogg");
if let Ok(data) = data {
    ctx.audio.register("door_copy", data);
}
```

### `data.duration() -> f32`

Length in seconds.

```rust
let len = SoundData::tone(440.0, 0.25, 0.5).duration();   // 0.25
```

### `data.channels() -> usize`

1 for mono, 2 for stereo.

```rust
let mono = SoundData::tone(440.0, 0.1, 0.5).channels() == 1;
```

### `data.sample_rate() -> u32`

Sample rate in Hz.

```rust
let rate = SoundData::tone(440.0, 0.1, 0.5).sample_rate();   // 44100
```

## Synth

A tiny synthesizer for clicks, beeps and effects, built with chained calls. A `Synth`
converts into `SoundData`, so it can be passed directly to `ctx.audio.register` and
`ctx.ui.set_sound_data`. Settings apply to the part they are called on; parts added with
`then` and `with` keep their own settings.

Defaults of a new part: 0.1 s long, attack 2 ms, no decay, release 5 ms, volume 0.5, no
sweep.

### `Synth::new(wave: Wave, frequency: f32) -> Synth`

A part with the given wave shape and frequency in hertz.

```rust
let buzz = Synth::new(Wave::Saw, 110.0).length(0.3);
```

### `Synth::sine(frequency: f32) -> Synth`

Sine wave: smooth and pure.

```rust
let ping = Synth::sine(1200.0).length(0.1).decay(30.0);
```

### `Synth::square(frequency: f32) -> Synth`

Square wave: hollow, retro.

```rust
let blip = Synth::square(600.0).length(0.05).volume(0.2);
```

### `Synth::triangle(frequency: f32) -> Synth`

Triangle wave: soft, between sine and square.

```rust
let soft = Synth::triangle(660.0).length(0.08);
```

### `Synth::saw(frequency: f32) -> Synth`

Sawtooth wave: bright and buzzy.

```rust
let laser = Synth::saw(1500.0).sweep_to(200.0).length(0.2).volume(0.3);
```

### `Synth::noise() -> Synth`

White noise: clicks, steps, hiss, explosions.

```rust
let hiss = Synth::noise().length(0.5).volume(0.1);
```

### `Synth::silence(seconds: f32) -> Synth`

Silence, as a pause between parts.

```rust
let double = Synth::sine(900.0).length(0.05)
    .then(Synth::silence(0.05))
    .then(Synth::sine(900.0).length(0.05));
```

### `synth.length(seconds: f32) -> Synth`

Length of this part in seconds.

```rust
let long = Synth::sine(300.0).length(1.5);
```

### `synth.sweep_to(frequency: f32) -> Synth`

Glides the frequency linearly to `frequency` over the whole length.

```rust
let rise = Synth::sine(400.0).sweep_to(1200.0).length(0.3);
```

### `synth.attack(seconds: f32) -> Synth`

Fade-in time. Default 2 ms.

```rust
let swell = Synth::triangle(220.0).length(1.0).attack(0.5);
```

### `synth.decay(rate: f32) -> Synth`

Exponential fade out: volume is multiplied by `e^(-rate * t)`. 0 none (default), 10 rings
about 0.7 s, 30 a pluck, 120 a click (60 ms), 300 a tick (20 ms).

```rust
let pluck = Synth::triangle(440.0).length(0.4).decay(30.0);
```

### `synth.release(seconds: f32) -> Synth`

Linear fade out at the end. Default 5 ms, which prevents clicks.

```rust
let tail = Synth::sine(500.0).length(0.5).release(0.2);
```

### `synth.volume(volume: f32) -> Synth`

Linear loudness of this part, 0.0 to 1.0. Default 0.5.

```rust
let quiet = Synth::noise().length(0.02).volume(0.15);
```

### `synth.with(other: Synth) -> Synth`

Plays `other` at the same time (layered).

```rust
let thump = Synth::sine(150.0).sweep_to(50.0).length(0.15).decay(30.0)
    .with(Synth::noise().length(0.05).decay(80.0).volume(0.3));
```

### `synth.then(other: Synth) -> Synth`

Plays `other` after this part (and after any parts already chained with `then`).

```rust
let pling = Synth::triangle(660.0).length(0.06)
    .then(Synth::triangle(990.0).length(0.12).decay(25.0));
ctx.audio.register("pling", pling);
```

### `synth.build() -> SoundData`

Renders the sound (mono, 44100 Hz). Usually not needed, because `register` and
`set_sound_data` accept the `Synth` directly.

```rust
let data = Synth::sine(1000.0).length(0.05).build();
let seconds = data.duration();
```

## Wave

```rust
pub enum Wave {
    Sine,       // smooth and pure
    Square,     // hollow, retro
    Triangle,   // soft
    Saw,        // bright and buzzy
    Noise,      // white noise
}
```

```rust
let tone = Synth::new(Wave::Triangle, 330.0);
```
