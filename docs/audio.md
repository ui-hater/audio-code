# Sound

All sound goes through `ctx.audio`. Playback runs in its own real-time mixer, so the game
loop never waits for audio.

## Playing sounds

```rust
ctx.audio.play("door");                       // once, centered
ctx.audio.play_at("door", vec3(3.0, 1.0, 0.0)); // once, at a position
let hum = ctx.audio.play_looped("hum");       // loops until stopped
let car = ctx.audio.play_looped_at("engine", car_pos);
ctx.audio.play_random(&["step1", "step2", "step3"]);
ctx.audio.play_tone(880.0, 0.1);              // generated beep, no file needed
```

For everything else, use the builder:

```rust
ctx.audio
    .sound("explosion")
    .volume(0.8)            // or .volume_db(-3.0)
    .pitch(0.9)             // or .random_pitch(0.9, 1.1)
    .at(vec3(5.0, 0.0, 0.0))
    .distance(2.0, 60.0)    // full volume within 2 m, silent at 60 m
    .rolloff(0.5)           // keep far sounds louder
    .fade_in(0.5)
    .start_at(1.2)          // skip the first 1.2 seconds
    .play();
```

Other builder options: `.looped()`, `.pan(-1.0..=1.0)`, `.group(Group::Ui)`, `.paused()`.

## Controlling a playing sound

Every play call returns a [`Sound`] handle. It is cheap to clone, and dropping it does
**not** stop the sound.

```rust
car.set_position(new_pos);   // move a positioned sound
car.set_pitch(1.0 + speed / 50.0);
car.fade_to(0.3, 2.0);       // volume 0.3 over two seconds
car.fade_out(1.0);           // fade and stop
car.stop();
car.pause();
car.resume();
car.seek(10.0);
car.set_looping(false);
car.is_playing();
car.position();              // seconds
```

Handles to finished sounds do nothing when called. Use `Sound::none()` as a placeholder in
your structs.

## 3D sound and the listener

The listener is the player's ears. Update it whenever the player moves or turns:

```rust
ctx.audio.set_listener(player_pos, facing);
```

`facing` is a compass angle: 0 = north (+y), 90 = east (+x). See the
[coordinate system](../src/math.rs).

For each positioned sound the engine:

- pans it left or right by its direction from the listener
- lowers the volume with distance (`distance(min, max)` and `rolloff`)
- muffles it and lowers the volume a little when it is **behind** the listener, so you can
  tell front from back on headphones

Defaults for all sounds can be set once:

```rust
ctx.audio.set_default_distance(1.0, 40.0);
ctx.audio.set_default_rolloff(0.7);
```

## Music

```rust
ctx.audio.play_music("music/level1");        // loops, cross fades from the previous track
ctx.audio.play_music_with_fade("boss", 3.0);
ctx.audio.stop_music(2.0);
```

Music files are decoded on a background thread, so starting a long track never causes a
stutter. Calling `play_music` with the track that is already playing does nothing, so you
can call it every frame without problems.

## Volume groups

Each sound belongs to a group: `Group::Sound` (default), `Group::Music` or `Group::Ui`
(menu sounds). Each group has its own volume, which is ideal for an options menu:

```rust
ctx.audio.set_master_volume(0.8);
ctx.audio.set_music_volume(0.5);
ctx.audio.set_sound_volume(1.0);
ctx.audio.set_group_volume(Group::Ui, 0.6);
ctx.audio.stop_group(Group::Sound, 0.5);
ctx.audio.stop_all();
```

## Finding files

`play("door")` searches in this order:

1. `door` as a path relative to the working directory
2. `<sound_dir>/door` (`sound_dir` is `"sounds"` unless set with `Engine::sound_dir`)
3. if the name has no extension: `.ogg`, `.wav`, `.mp3`, `.flac`, `.opus`, `.m4a`

Subfolders work: `play("weapons/pistol")` finds `sounds/weapons/pistol.ogg`.

Supported formats: WAV, Ogg Vorbis, MP3, FLAC, AIFF, AAC/M4A, MKV/WebM audio (via
symphonia).

## Loading and memory

Files are decoded into memory on first use and cached. To avoid a small delay the first
time a sound plays:

```rust
ctx.audio.preload("explosion")?;
ctx.audio.preload_dir("weapons")?;   // loads every file in sounds/weapons
ctx.audio.unload("explosion");       // frees the cache entry
```

A missing or broken file prints one warning and returns a silent handle, so a typo never
crashes the game.

## Generated sounds

`SoundData` lets you make sounds in code, which is good for prototypes and retro effects:

```rust
use std::f32::consts::TAU;

ctx.audio.register("beep", SoundData::tone(880.0, 0.1, 0.5));
ctx.audio.register("sweep", SoundData::generate(0.3, |t| (t * (200.0 + t * 3000.0) * TAU).sin() * 0.4));
ctx.audio.register("noise", SoundData::generate(0.1, |t| random::range(-1.0, 1.0) * (-t * 50.0).exp()));
```

`SoundData::from_samples(samples, channels, sample_rate)` accepts raw interleaved samples.

## No audio device?

If no output device can be opened, the engine prints a warning and keeps running without
sound. `ctx.audio.has_output()` tells you which case you are in.

[`Sound`]: ../src/audio/mod.rs
