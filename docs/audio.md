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

## What the numbers mean

All times are in **seconds** and all positions and distances in **meters** (the same units
as [physics](physics.md#units-and-directions)).

| Setting | Default | Meaning | Examples |
|---------|---------|---------|----------|
| `volume(v)` | `1.0` | Linear volume factor. `1.0` is the file as recorded, `0.5` half the amplitude (about 6 dB quieter, clearly softer), `0.0` silent. Values above `1.0` amplify and may distort. | background hum 0.2–0.4, footsteps 0.5–0.8 |
| `volume_db(db)` | `0.0` | The same in decibels. `0` = original, `-6` = about half, `-20` = a tenth, `+6` = about double. | `-3.0` slightly softer |
| `pitch(p)` | `1.0` | Playback speed. It changes pitch and length together: `2.0` is one octave higher and twice as fast, `0.5` one octave lower and twice as long. | engine sound 0.8–2.0 depending on speed |
| `random_pitch(min, max)` | – | Picks a pitch between `min` and `max` each time. Small ranges keep repeated sounds from sounding mechanical. | footsteps `0.9, 1.1` |
| `pan(p)` | `0.0` | Left/right position for sounds **without** a position: `-1.0` fully left, `0.0` center, `1.0` fully right. Ignored for positioned sounds. | `-0.5` a bit left |
| `at(pos)` | none | Puts the sound in the world. It is then panned and attenuated relative to the listener. | |
| `distance(min, max)` | `1.0, 30.0` | Audible range in meters: full volume up to `min`, silent from `max` on. | small objects `0.5, 15.0`, an alarm `3.0, 100.0` |
| `rolloff(r)` | `0.7` | How fast the volume drops between `min` and `max`, see the table below. | |
| `fade_in(s)` | `0.0` | Fades from silence to the volume over `s` seconds. | |
| `start_at(s)` | `0.0` | Skips the first `s` seconds of the file. | |

### Distance and rolloff

The volume of a positioned sound depends on its distance from the listener. Within `min`
it plays at full volume. From there it gets quieter, and at `max` it is silent. `rolloff`
sets the shape of the curve: `1.0` behaves like real sound (twice the distance, half the
volume), lower values keep far sounds louder, and `0.0` keeps the full volume and only
fades out near `max`. Values above `1.0` drop off even faster.

Volume (as a factor) with the default `distance(1.0, 30.0)`:

| Distance | rolloff 0.0 | rolloff 0.5 | rolloff 0.7 (default) | rolloff 1.0 |
|----------|-------------|-------------|-----------------------|-------------|
| 1 m      | 1.00        | 1.00        | 1.00                  | 1.00        |
| 2 m      | 1.00        | 0.71        | 0.61                  | 0.50        |
| 5 m      | 0.98        | 0.44        | 0.32                  | 0.20        |
| 10 m     | 0.90        | 0.29        | 0.18                  | 0.09        |
| 20 m     | 0.57        | 0.13        | 0.07                  | 0.03        |
| 30 m     | 0           | 0           | 0                     | 0           |

Rules of thumb:

- Make `max` as large as the distance from which the player should still hear the thing.
  Everything beyond `max` is silent, even a loud explosion.
- Raise `min` for big or loud sources (a waterfall, a machine hall) so they stay at full
  volume in a wider area.
- Lower `rolloff` (0.3–0.5) for important sounds the player navigates by, such as beacons
  and enemies, so they stay audible from far away.

Sounds **behind** the listener are up to 30 % quieter and muffled, so you can tell front
from back.

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

### How `set_listener` works

`set_listener(position, yaw)` takes two things:

- `position`: where the ears are, as a `Vec3` in world units (z is up, +y is north,
  +x is east).
- `yaw`: the compass direction the player faces, in degrees: 0 = north (+y), 90 = east
  (+x), 180 = south, 270 = west. Values outside 0–360 are fine.

`vec3(x, y, z)` is a point in meters: `x` east (negative = west), `y` north (negative =
south), `z` up (negative = down). `vec3(10.0, 0.0, 2.0)` is 10 m east and 2 m up. See
[Positions, directions and 3D](3d.md).

To let the player look up or down as well, use `set_listener_3d(position, yaw, pitch)`.
`pitch` is in degrees: 0 = level, 90 = straight up, -90 = straight down. `set_listener`
is the same with a pitch of 0.

The listener starts at the origin facing north. It keeps its position and facing until you
call `set_listener` again. Sounds do not follow it by themselves. Call it every time the
player moves or turns, usually once at the end of `update`. Calling it every frame is cheap.

The call stores the values on the game side, so `ctx.audio.listener_position()` and
`ctx.audio.listener_yaw()` return them right away. It also sends them to the audio thread,
which uses them from the next audio block on (a few milliseconds later). Every positioned
sound that is playing is then recalculated, including looping sounds that started long ago.

Only positioned sounds are affected: those started with `play_at`, `play_looped_at`,
`.at(pos)` on the builder, or `loop_on` / `play_on`. Plain `play` sounds, UI sounds and
music always sound the same, no matter where the listener is.

For each positioned sound the engine:

- turns the offset from the listener to the sound into the listener's own view, using
  `yaw` and `pitch`: how far it is to the right, how far ahead and how far above the head
- pans it left or right by that direction: a sound straight to the right is almost fully
  in the right ear, a sound straight ahead, behind, above or below is centered
- lowers the volume with the 3D distance: full volume up to `min`, silent at `max`
  (`distance(min, max)` and `rolloff`)
- muffles it and lowers the volume by up to 30 % when it is **behind** the listener, so you
  can tell front from back on headphones
- dulls it a little and lowers the volume by up to 15 % when it is **below** the listener;
  sounds above stay clear

So turning matters as much as moving. In this example the same sound first comes from the
right, then from straight ahead:

```rust
let bell = vec3(10.0, 0.0, 0.0);           // 10 units east of the origin
ctx.audio.play_looped_at("bell", bell);

ctx.audio.set_listener(Vec3::ZERO, 0.0);   // facing north: the bell is on the right
ctx.audio.set_listener(Vec3::ZERO, 90.0);  // facing east: the bell is in front
ctx.audio.set_listener(Vec3::ZERO, 270.0); // facing west: the bell is behind, muffled
```

A side scroller, where the player only moves left and right and always faces north, keeps
`yaw` at 0. Sounds to the east are then on the right:

```rust
ctx.audio.set_listener(vec3(self.x, 0.0, 0.0), 0.0);
```

If the player is a physics body, you do not need `set_listener` at all. `ctx.follow(body)`
sets the listener to the body's position, `facing` and `pitch` after every frame (see
[Sounds that follow objects](#sounds-that-follow-objects)). Do not mix both: while a body is
followed, the engine overwrites your own `set_listener` calls at the end of the frame.

Defaults for all sounds can be set once:

```rust
ctx.audio.set_default_distance(1.0, 40.0);
ctx.audio.set_default_rolloff(0.7);
```

## Sounds that follow objects

Instead of moving sounds by hand every frame, attach them to physics bodies:

```rust
ctx.follow(player);                    // the listener follows the player's position and facing
ctx.loop_on(enemy, "growl");           // loops at the enemy, moves with it
ctx.play_on(car, "horn");              // plays once, moves with the car
let hum = ctx.audio.sound("hum").volume(0.3).looped().at(pos).play();
ctx.attach(machine, hum);              // any positioned sound
ctx.stop_sounds_on(enemy);
```

When a body is removed with `ctx.physics.remove(id)`, its attached sounds stop by
themselves.

## Music

```rust
ctx.audio.play_music("music/level1");        // loops, cross fades from the previous track
ctx.audio.play_music_with_fade("boss", 3.0);
ctx.audio.stop_music(2.0);
```

Music files are decoded on a background thread, so starting a long track never causes a
stutter. Calling `play_music` with the track that is already playing does nothing, so you
can call it every frame without problems. `ctx.audio.music_name()` returns the name of the
current track. Menus can bring their own music with `Menu::music` (see [UI](ui.md#menu-music)).

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

## Designing sounds with the synth

`Synth` makes clicks, beeps and effects in code. You do not need any files:

```rust
// A short, soft click.
ctx.audio.register("click", Synth::sine(1500.0).length(0.03).decay(120.0).volume(0.4));

// Two notes one after the other.
ctx.audio.register("coin", Synth::triangle(988.0).length(0.08).then(Synth::triangle(1319.0).length(0.17).decay(15.0)));

// Two layers at the same time: noise plus a falling low tone.
ctx.audio.register("thump", Synth::sine(150.0).sweep_to(50.0).length(0.15).decay(30.0)
    .with(Synth::noise().length(0.05).decay(80.0).volume(0.3)));

// A rhythm with pauses.
ctx.audio.register("alarm", Synth::square(800.0).length(0.1).then(Synth::silence(0.1)).then(Synth::square(800.0).length(0.1)));
```

| Method | Default | Meaning |
|--------|---------|---------|
| `Synth::sine(hz)` | – | pure, soft tone; good for beeps and clicks |
| `Synth::triangle(hz)` | – | a bit brighter than sine, still soft; good for pickups and menus |
| `Synth::square(hz)` | – | hollow, retro game sound, quite loud |
| `Synth::saw(hz)` | – | buzzy and bright; engines, alarms |
| `Synth::noise()` | – | hiss; footsteps, wind, explosions, hits |
| `Synth::silence(seconds)` | – | a pause, for use with `then` |
| `.length(seconds)` | `0.1` | total duration |
| `.sweep_to(hz)` | – | glides the pitch from the start frequency to `hz` over the whole length |
| `.attack(seconds)` | `0.002` | fade in at the start; longer values make a soft start |
| `.decay(rate)` | `0.0` (none) | exponential fade out, see below |
| `.release(seconds)` | `0.005` | short linear fade at the very end, so the sound does not stop with a click |
| `.volume(v)` | `0.5` | linear loudness, `1.0` is full scale |
| `.then(other)` | – | plays `other` afterwards |
| `.with(other)` | – | plays `other` at the same time (layers) |

Settings apply to the part they are called on; parts added with `then` and `with` keep
their own settings.

**Frequency** (`hz`) is the pitch in hertz, vibrations per second. 440 is the A above
middle C, doubling the number goes one octave up. 50–150 Hz is a deep thump or rumble,
200–600 a voice-like range, 800–2000 clear beeps and clicks, above 3000 sharp and piercing.
Some notes: C5 523, E5 659, G5 784, A5 880, B5 988, C6 1047, E6 1319.

**Decay** is how fast the sound fades out after the start: the volume is multiplied by
`e^(-rate * time)`. A higher rate fades faster:

| Rate | Half as loud after | Inaudible (−60 dB) after | Sounds like |
|------|--------------------|--------------------------|-------------|
| 10   | 70 ms              | 0.7 s                    | a ringing bell |
| 30   | 23 ms              | 0.23 s                   | a pluck, a drop |
| 60   | 12 ms              | 0.12 s                   | a knock |
| 120  | 6 ms               | 0.06 s                   | a click |
| 300  | 2 ms               | 0.02 s                   | a tick |

If the decay already makes the sound inaudible, a longer `length` only adds silence.

## Generated sounds from formulas

For full control, `SoundData` takes a function of time:

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
