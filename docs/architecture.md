# Architecture

This page explains how audio-code works inside: for contributors, and for game developers
who want to know why the engine behaves the way it does. The [README](../README.md#how-it-works-inside)
has the short version.

## Overview

```text
 main thread                         audio thread (cpal)            speech thread
 ───────────                         ───────────────────            ─────────────
 window events ──► Runner            Mixer                          prism / NVDA / macOS / console
                     │                 ▲  drains commands              ▲
                     ▼                 │  each audio block             │ Speak, Stop, ...
   your Scene ◄──► Context ── Audio ───┘ (channel)                     │
                            └ Speech ──────────────────────────────────┘ (channel)

 background threads: one per music file being decoded
```

Your code always runs on the main thread and only touches `Context`. The other threads are
reached through channels. No game code ever holds a lock.

## Engine, Runner and the frame (`engine.rs`)

- **`Engine`** only collects settings. Its `run*` methods hand a `Runner` to a window
  backend.
- **`Runner`** owns the `Context` and the scene stack (`Vec<Box<dyn Scene>>`). Because
  these are two separate fields, `scene.update(&mut self.ctx)` borrows two different things,
  so no `RefCell` is needed.
- **Window backends** call `Runner::key_down`, `key_up`, `text` and `focus_lost` when the
  operating system reports something, and call `Runner::frame()` when the next frame is due
  (`Runner::next_frame_time`). Frames are never "caught up": if the game falls behind, the
  next frame is simply scheduled from now.

`Runner::frame()`:

1. On the very first call: advance the clock, call `start` on the first scene, apply scene
   changes, and return. `update` runs from the second frame on.
2. `time.advance(now)`. The delta is clamped to 0.25 s, so a debugger pause does not shoot
   bodies through walls. The scheduler fires named events here.
3. `input.begin_frame(now)` turns the queued key events into this frame's
   pressed/released/held sets.
4. `ui.update(...)` runs the top widget. If a widget is open, `input` is set to *blocked*.
5. Each UI event is given to `Scene::on_ui`.
6. `physics.clear_events()`, then physics steps at a fixed rate with an accumulator (at most
   8 steps per frame, so a slow machine cannot spiral). `fixed_update` runs after each step.
7. `Scene::update`.
8. `ctx.sync_world()` moves the listener and attached sounds (see [Shortcuts](#shortcuts-shortcutsrs)),
   then `ctx.sync_vehicles()` updates engine loops and plays shift and crash sounds.
9. `apply_change()` applies `push_scene` / `pop_scene` / `switch_scene`. It loops, because a
   scene's `start` may request another change.
10. If `ctx.quit()` was called: `shutdown()` ends all scenes and saves `ctx.storage`.

Physics events are cleared once per frame (not per step), so events from every step of the
frame are still visible in `update`.

`run_with(build)` wraps the build closure in a private `Lazy` scene that builds the real
scene in its `start` and forwards every call to it. `run_fn(closure)` wraps a closure in
`FnScene`, whose `update` calls the closure.

## Input (`input.rs`, `window.rs`, `wx.rs`)

Key events are queued, and `begin_frame` processes them in one go. That is why a key pressed
and released within one frame still counts as `pressed`. A `Down` for a key that is already
held counts as an OS repeat (wxWidgets does not flag repeats, so this rule detects them).

`repeating(key, delay, every)` is computed without extra state: the input keeps how long each
key was held in this frame and in the previous one, and reports `true` when a repeat point
lies between the two.

**Blocking.** While a UI widget is open, all normal queries return `false`. Widgets read
the keys through `RawInput` (`input.raw()`), which ignores blocking.

**winit key mapping.** Letters and digits use the *logical* key, so they follow the keyboard
layout (Z on a German keyboard is `Key::Z`). Everything else uses the physical key. The key
reported at press time is remembered per physical key, so the release maps to the same
`Key` even if Shift changed in between. Synthetic presses, which winit sends on focus gain,
are ignored.

**wxWidgets.** `GameHost` listens to the frame's *char hook*. It sees every key before
wxWidgets uses Tab or the arrow keys for dialog navigation. Printable keys are passed on, so
the following `Char` event delivers layout-correct text. A wx `Timer` drives `Runner::frame`.
The state lives in `Rc<RefCell<..>>`, and every handler uses `try_borrow_mut`, because
closing the frame from inside a tick fires the close handler synchronously.

## Time (`time.rs`)

`Time` has two clocks: *game time* (scaled, can be paused) and *real time*. The scheduler
(`after`, `every`, `fired`) runs on game time. Repeating events keep their rhythm by adding
the interval to the previous due time, not to "now". `Timer`, `Cooldown` and `Interval` are
independent structs on top of `Instant`, so they work without a context.

## Audio (`audio/`)

### The public side (`mod.rs`)

`Audio` keeps:

- a **cache** `name → Arc<SoundSlot>`. A slot holds the decoded `SoundData` in a `OnceLock`,
  or a "failed" flag.
- the **Sender** of the command channel to the mixer
- copies of the listener and volume settings, so getters work without asking the audio thread

`play(name)` resolves the name to a file, decodes it once (`data.rs`, symphonia, into
interleaved `f32`), builds a `Voice` and sends `Command::Play`. It returns a `Sound` handle
containing the voice id, an `Arc<VoiceShared>` (atomics: finished flag and position) and a
clone of the sender. Every handle method is a `Command::Change(id, ...)`. Dropping a handle
does not stop the sound, because the mixer owns the voice.

Missing files go through `slot_or_warn`, which warns once per name and returns a dead handle
(`Sound::none()`).

`play_music` creates an empty slot, starts a thread that decodes the file into it, and
creates the voice right away. The mixer skips voices whose slot is not ready yet, so the
music starts as soon as decoding finishes, without blocking the game.

If no output device can be opened, `Audio::silent` is used: the receiver is dropped and
commands go nowhere.

### The mixer (`mixer.rs`)

The cpal callback calls `Mixer::render(stereo_buffer)` for every block:

1. Drain all commands (`try_recv`, never blocking). More than `MAX_VOICES` (512) voices
   drops the oldest one.
2. For every voice:
   - **Targets.** Compute left/right gain and a low-pass coefficient.
     - Non-positioned: stereo balance from `pan`.
     - Positioned: transform the offset to the listener into listener space using the
       compass yaw and the pitch: `local_x` along `math::right(yaw)`, `local_y` along
       `math::forward_3d(yaw, pitch)`, `local_z` along `math::up_3d(yaw, pitch)`. Then
       - `gain = (min / max(distance, min))^rolloff · (1 − t²)`, where `t` goes from 0 at
         `min_distance` to 1 at `max_distance` (silent from there)
       - `pan = local_x / distance · 0.9`, with equal-power panning
       - `behind = max(0, −local_y / distance)`: up to 30 % quieter and a low-pass from
         20 kHz down to 2.5 kHz. Front and back sound different on headphones without HRTF.
       - `below = max(0, −local_z / distance)`: up to 15 % quieter (less when also behind),
         and the filter uses `max(behind, 0.6 · below)`, so below is duller than above but
         clearer than behind. Sounds straight above or below are centered, because only
         `local_x` pans.
   - **Smoothing.** Gains and the filter ramp linearly from last block's values to the new
     targets, so moving sounds do not click.
   - **Rendering.** Read the source with linear interpolation at
     `step = source_rate / device_rate · pitch`. This one number does both resampling and
     pitch. Then apply volume fades (`fade_step` per sample; `stop_after_fade` finishes the
     voice at 0), the one-pole low-pass, and add to the output. At the end of the data the
     voice either loops or finishes.
3. Clamp to −1..1. The callback then converts to the device's sample format and channel
   count (mono devices get the average, extra channels get silence).

Group volumes (sound, music, UI) and the master volume are multiplied in per voice.

### The synth (`synth.rs`)

A `Synth` is one oscillator (sine, square, triangle, saw or noise) with a frequency sweep and
an envelope (attack, exponential decay, linear release that reaches exactly zero at the last
sample). `with` adds layers that are summed; `then` appends parts. `build()` renders to mono
44.1 kHz `SoundData`. `From<Synth> for SoundData` lets `Audio::register` and
`Ui::set_sound_data` accept a synth directly.

## Speech (`speech/`)

`Speech::new` spawns one thread and keeps the sender. The thread tries, in order:

1. `run_prism`: create a prism context, pick the best backend, then loop over commands
   until the channel closes.
2. `nvda::run` (Windows): load `nvdaControllerClient.dll` with `libloading` and copy out four
   function pointers (`testIfRunning`, `speakText`, `cancelSpeech`, `brailleMessage`).
   Strings are sent as NUL-terminated UTF-16. "Interrupt" means `cancelSpeech` and then
   `speakText`. If NVDA is not running when a message arrives, the message is printed
   instead.
3. `macos::run` (macOS): fail only if `/usr/bin/say` is missing. Per message, check (cached
   for two seconds) whether VoiceOver runs with `pgrep -x VoiceOver`. If so, speak with
   `osascript` (`tell application id "com.apple.VoiceOver" to output ...`, the text passed as
   an argument so it needs no escaping). If VoiceOver refuses AppleScript once, use the system
   voice from then on: one `say` process per message, killed on interrupt or `stop`. Queued
   messages wait in a `VecDeque`, and while one plays the loop polls the channel with
   `recv_timeout` to start the next as soon as the process exits.
4. Nothing works: print why, and drop the receiver.

On the main thread, `Speech::send` notices a closed channel and switches to console output
for good. Messages sent while the thread is still starting are not lost, because they wait in
the channel.

History (100 messages), mute and echo live on the main thread. Echo is an `AtomicBool`
shared with the thread, so the NVDA "not running" path does not print a message twice.

## Virtual UI (`ui/`)

`Ui` holds a stack of entries `{ widget: Box<dyn Widget>, state }` with these states:

| State | Next `ui.update` does |
|-------|-----------------------|
| `Fresh` | `widget.open()`: announce itself and play the open sound |
| `Active` | `widget.update()`: handle keys |
| `Resume(value)` | `widget.resume(value)`: the widget above closed; re-announce, accept its value |

`update` returns a `Status`:

- `Open`: stay
- `Close`: pop
- `CloseWith(value)`: pop and give `value` to the widget below
- `Push(widget)`: open a child

Opening is deferred to the next frame on purpose: the key that opened a menu in
`Scene::update` has already been handled, so it cannot also activate the first item.

**Menus** (`menu.rs`) are a list of `MenuItem { id, label, kind }`. Pressing Enter on a
`Text` item pushes a `TextInput` marked as a child. When the child submits, it returns
`CloseWith(Value::Text)`, and the menu stores the text in `resume` and emits `Changed`.
First-letter navigation searches labels cyclically from the current item.

**Events** are collected in `Ui::events` during `ui.update` and cleared at the start of the
next one. The query helpers (`clicked`, `changed`, `confirmed`, ...) just search that list.

**Sounds** (`sounds.rs`). `UiSounds` maps each `UiSound` slot to a sound *name*. Three kinds
of names exist:

- `ui:builtin:*`: synthesized on first use by `ensure_builtin`
- `ui:custom:*`: created by `Ui::set_sound_data`. The data waits in `pending_sounds` and is
  registered with `Audio` at the next `ui.update`, because `Ui` has no access to `Audio`
  outside of updates.
- anything else: a normal file or registered sound

**Music.** At the start of every `ui.update`, `update_music` looks for the topmost widget
whose `Widget::music` returns a name. If that name differs from the widget music `Ui`
started last, it calls `Audio::play_music` (which cross fades). Before the first widget
music starts, the current `Audio::music_name` is saved in `previous_music`. When no widget
wants music any more, that track is played again, or the music fades out, but only if the
widget music is still the current track; music the game started meanwhile is kept.

`Wrap` falls back to `Navigate` when unset. Menus can override single slots
(`Menu::sound`) or replace the whole set (`Menu::sounds`).

## Physics (`physics.rs`)

Bodies live in a `BTreeMap<BodyId, Body>`. The ordered map keeps the simulation
deterministic. Shapes are axis-aligned boxes and spheres. They never rotate; `facing` and
`pitch` are only numbers for your game, `move_forward` (`math::forward_3d`) and the
listener.

**`penetration(a, b)`** returns the shortest direction to push `a` out of `b`, and the depth:

- box–box: the axis with the smallest overlap
- sphere–sphere: along the line between the centers
- sphere–box: from the closest point on the box; if the center is inside, through the
  nearest face

**`resolve(id)`** (up to 4 iterations, to settle corners) pushes `id` out of the deepest
overlap. Who moves:

| Mover | Other | Result |
|-------|-------|--------|
| any | fixed / kinematic | mover is pushed out fully |
| kinematic | dynamic | the dynamic body is pushed away fully, which is how a player shoves a crate |
| dynamic | dynamic | the overlap is shared by mass |

If pushing the other body would push it into a third solid in the push direction
(`obstructed`: a crate against a wall), the push is undone and the mover is blocked
instead.

**`step(dt)`**:

0. Drive vehicles (see [Vehicles](#vehicles-vehiclers)).
1. Move kinematic bodies (vehicles excluded) by `velocity · dt` and remember their displacement.
2. For each dynamic body:
   1. If its `ground` moved, apply that displacement (platforms carry bodies).
   2. Add `velocity += (gravity · gravity_scale + force / mass) · dt` and clear the force.
   3. Apply damping, ground friction (horizontal only) and the `max_speed` clamp.
   4. Move in sub steps no longer than the body's smallest half extent; after each sub step,
      `resolve`.
   5. For each contact, `bounce`: against dynamic bodies, an impulse
      `j = −(1 + e) · v_closing / (1/m_a + 1/m_b)`; against walls, remove (or reflect, with
      bounciness) the velocity into the surface. The closing speed becomes the impact speed.
   6. A contact whose normal points up (`dot > 0.7`) sets `on_ground` and `ground`. A
      transition from air to ground emits `Landed`.
3. A contact pair that did not exist last step and hit with some speed emits
   `CollisionStarted`. Pairs that ended emit `CollisionEnded`.
4. Sensor overlaps are compared with last step and emit `Entered` / `Left`.

A resting body keeps `on_ground` because gravity pushes it a tiny bit into the floor every
step, which is resolved again.

**`move_by(delta)`** runs the same sub step + `resolve` loop immediately, without
velocities. `blocked` is set when a contact normal opposes the movement. `move_forward`,
`strafe` and `turn` are thin helpers using `facing`.

**Ray casts** use the slab method for boxes and the quadratic formula for spheres, and
ignore sensors.

### Vehicles (`vehicle.rs`)

`Body::vehicle: Option<Vehicle>`; bodies with a vehicle are kinematic. In `step_vehicle` the
vehicle is taken out of its body (so `move_by` can borrow the world), simulated, and put
back:

1. `Vehicle::simulate(mass, dt)`: weight = mass + load. Arcade pedals: moving forward the
   brake brakes, moving backward the gas brakes, standing still (< 0.3 m/s) gas minus brake
   drives. The pedal that braked to a stop is held (`hold`) until released, so holding the
   brake does not flip into reverse. Engine acceleration is
   `engine_force · drive · (1 − (v/top)²) / weight`, clamped to `grip`. Braking is
   `brake_force · pedal / weight` (handbrake: `grip`), clamped to `grip`, plus rolling
   resistance, plus engine braking and `0.3 · engine_force / weight · (v/top)²` air drag
   when no pedal drives. Deceleration never crosses zero.
2. Turn rate = `min(|v| / turn_radius` in degrees`, max_turn_rate) · steer · sign(v)`.
3. Fuel: `consumption / 100 000` liters per meter, times `0.3 + 0.7 · pedal` and
   `weight / mass`. An empty tank stops the engine.
4. Gearbox: gear bands are `top · g / gears`; shift up above 95 % of the band, down below
   70 % of the band below. rpm = idle + (max − idle) · speed / band (at least 30 % of the
   throttle while revving). Changes emit `GearChanged`.
5. The new facing and `forward(facing) · speed · dt` go to `move_by`. If it was blocked,
   `contact_ahead` probes 2 cm further to find the obstacle and its normal; the velocity
   into the normal is removed (sliding keeps the rest) and, above 0.5 m/s, `Crashed` is
   emitted with the lost speed. `velocity` is set for other code to read.

### Levels (`level.rs`)

`build_level` trims blank lines and the common indentation, then scans row by row. Runs of
`#` in a row become a single box (fewer bodies, and no seams to catch on). Other characters
are stored as tile centers. Row 0 is the northmost row.

## Shortcuts (`shortcuts.rs`)

The convenience methods on `Context` are thin wrappers, plus two pieces of state:

- `follow: Option<BodyId>`: `sync_world` copies the body's position and facing to the
  listener, but only when they changed, to avoid flooding the audio channel.
- `attached: Vec<(BodyId, Sound, last_position)>`: `sync_world` moves each sound to its body
  when the body moved, drops entries whose sound finished, and stops sounds whose body was
  removed.

`once(name)` is a `HashSet<String>` insert.

`engine_sounds: Vec<(BodyId, Sound)>` holds one engine loop per vehicle with an engine
sound. `sync_vehicles` (once per frame, from `Runner::frame`, not from `follow`) starts the
loop when the engine runs, sets its position, pitch (`engine_pitch`) and volume, fades it
out when the engine stops or the body is removed, and plays shift and crash sounds for this
frame's events.

## Storage (`storage.rs`)

A `BTreeMap<String, String>` that is loaded from and written to `key = value` lines.
Backslash, newline and carriage return in values are escaped. Values are converted with
`ToString` / `FromStr`, so numbers, `bool` and `String` work without extra code. A dirty
flag avoids writing when nothing changed. Headless runners use in-memory storage, so tests
never write files.

**Encryption** (optional, `open_encrypted` / `Engine::encrypt_save`): the key is
`SHA-256("audio-code save key v1\0" + secret)`. An encrypted file is
`"AUDIOCODE-SAVE-ENC1\n"`, a random 24 byte nonce (`getrandom`), and the XChaCha20-Poly1305
ciphertext of the same `key = value` text with its 16 byte tag; the magic header is the
associated data. Every save uses a new nonce. Loading picks the format from the header, so
plain files still load (and are marked dirty to be rewritten encrypted). A file that fails
to decrypt or is not text is renamed to `<file>.broken` before anything can overwrite it;
if the rename fails, saving is disabled. The reason is kept in `load_error`. `Debug` never
prints the key.

## Design decisions

- **Own mixer instead of an audio library.** Full control over 3D panning, the behind-the-
  listener filter, fades and background loading, with a small, stable API. Trade-off: no
  HRTF and no audio effects beyond the low-pass.
- **No rotation in physics.** Audio games rarely need it, and it keeps collision code short
  and behavior easy to predict and describe with sound.
- **String ids for UI and scheduler events.** Easy to read and write. Typos are possible,
  but nothing has to be declared up front.
- **Polling plus callbacks.** Events are kept for one frame, so they can be polled where
  they are used (`ctx.ui.clicked`, `ctx.physics.landed`). `Scene::on_ui` exists for those
  who prefer callbacks.
- **Warnings instead of errors for missing assets.** A game keeps running with one missing
  sound, and the warning names the file.
- **Backends only feed the Runner.** That makes headless tests trivial, and new backends
  (SDL, a terminal, ...) only need key events and a timer.

## Extending

- **A new widget:** implement `ui::Widget` (`id`, `open`, `update`, optionally `resume` and
  `value`) and open it with `ctx.ui.open(MyWidget)`.
- **A new window backend:** create a `Runner` with `Engine::build(scene)`, forward keys with
  `key_down` / `key_up` / `text`, and call `frame()` at `next_frame_time()`.
- **A new key:** add it to `input::Key`, then map it in `window.rs` (`map_code`) and `wx.rs`
  (`map_key`).
- **A new speech output:** add a `run_*` function to the fallback chain in `speech/mod.rs`
  that returns `Err` when it is unavailable.
