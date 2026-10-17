# Cheat sheet

The calls you need most, on one page. Everything is reached through `ctx`.

Units everywhere: times in **seconds**, distances and positions in **meters**, speeds in
**meters per second**, angles in **compass degrees** (0 = north, 90 = east, positive turns
right), volumes as a **linear factor** (1.0 = unchanged, 0.5 = half, 0.0 = silent). Axes:
`+x` east, `+y` north, `+z` up. Pitch: 0 = level, 90 = straight up, -90 = straight down.

Positions: `vec3(x, y, z)` in meters. `vec3(3.0, -2.0, 1.5)` = 3 m east, 2 m south, 1.5 m
up. Details in [Positions, directions and 3D](3d.md).

## Starting

```rust
use audio_code::prelude::*;

// A whole game as one closure:
audio_code::run("My game", move |ctx| { /* every frame */ })?;

// A game with scenes:
Engine::new("My game").run(MainMenu)?;
Engine::new("My game").gravity(EARTH_GRAVITY).run_with(Level::new)?;   // Level::new(ctx) -> Level
```

## Every day

| What | Code |
|------|------|
| Speak | `ctx.speak("Hello")` |
| Play a sound | `ctx.play("door")` (finds `sounds/door.ogg`) |
| Play at a place | `ctx.play_at("door", vec3(3.0, 5.0, 0.0))` |
| Key went down | `ctx.pressed(Key::Space)` |
| Key is held | `ctx.held(Key::Up)` |
| Walk with your own key repeat | `ctx.input.repeating(Key::Up, 0.3, 0.3)` |
| Left/right as -1/0/1 | `ctx.input.axis(Key::Left, Key::Right)` |
| Arrows or WASD as a vector | `ctx.input.movement()` |
| Do something once | `if ctx.once("intro") { ... }` |
| Quit | `ctx.quit()` |

## The world

| What | Code |
|------|------|
| Build walls from a text map | `let level = ctx.physics.build_level(MAP, 2.0);` (2 m per character) |
| Spawn points from the map | `level.point('P')`, `level.points('C')` |
| Player body (top down) | `let p = ctx.physics.add(Body::kinematic(pos).with_sphere(0.4));` (radius 0.4 m) |
| Player body (platformer) | `let p = ctx.physics.add(Body::dynamic(pos).with_box(0.8, 0.8, 1.6));` (width, depth, height in m) |
| Ears follow the player | `ctx.follow(p)` |
| Turn | `ctx.physics.turn(p, 45.0)` (degrees, positive = right) |
| Look up / down | `ctx.physics.tilt(p, 30.0)` (degrees, positive = up) |
| Fly forward / up | `ctx.physics.move_forward(p, 1.0)` follows the pitch, `ctx.physics.move_up(p, 1.0)` |
| Look at a point | `ctx.physics.look_at(p, pos)` |
| Walk forward, hit walls | `if ctx.physics.move_forward(p, 1.0).blocked { ctx.play("wall") }` (meters) |
| Where is it? | `ctx.speak(ctx.physics.describe(p, target))` → "3 o'clock, 12 meters" |
| Something that makes noise | `ctx.loop_on(enemy, "growl")` (follows it, stops when removed) |
| Trigger zone | `ctx.physics.add(Body::sensor(pos).with_sphere(1.0).with_tag("coin"))` |
| Walked into a zone | `if let Some(coin) = ctx.physics.entered_tag(p, "coin") { ... }` |
| Hit something (dynamic body) | `if let Some((other, speed)) = ctx.physics.collided(p) { ... }` (speed in m/s) |
| Hit something (`move_by`) | `for other in &ctx.physics.move_by(p, delta).hits { ... }` |
| Gravity | `Engine::new(..).gravity(EARTH_GRAVITY)` |
| Jump (dynamic body) | `ctx.physics.jump(p, 1.5)` (height in meters) |
| Landed (dynamic body) | `if let Some(speed) = ctx.physics.landed(p) { ... }` (falling speed in m/s) |
| Remove | `ctx.physics.remove(id)` |

## Time

| What | Code |
|------|------|
| Seconds since last frame | `ctx.time.delta()` |
| In 5 seconds | `ctx.time.after("boom", 5.0)` … `if ctx.time.fired("boom")` |
| Every 2 seconds | `ctx.time.every("spawn", 2.0)` |
| Pause the game | `ctx.time.pause()` / `ctx.time.resume()` |
| Stopwatch, cooldown, steady beat | `Timer::new()`, `Cooldown::new(0.5).try_use()`, `Interval::new(1.0).tick()` |

## Menus and dialogs

```rust
ctx.ui.open(Menu::new("main", "Main menu").button("play", "Play").button("quit", "Quit"));
if ctx.ui.clicked("main", "play") { ctx.ui.close(); }
Menu::new("main", "Main menu").music("music/menu")   // loops while the menu is open

ctx.ui.message("intro", "Welcome!");          if ctx.ui.closed("intro") { ... }
ctx.ui.confirm("quit", "Really quit?");       if ctx.ui.confirmed("quit") == Some(true) { ... }
ctx.ui.input("name", "Your name?");           if let Some(name) = ctx.ui.submitted("name") { ... }
```

## Sounds

```rust
let hum = ctx.audio.play_looped("hum");     hum.stop();
ctx.audio.sound("shot").volume(0.5).random_pitch(0.9, 1.1).at(pos).play();
//                     half as loud  speed/pitch ±10 %     position in meters
ctx.audio.play_music("music/theme");
ctx.audio.register("click", Synth::sine(1500.0).length(0.03).decay(120.0));
//                                   1500 Hz      30 ms         fades out fast
ctx.ui.set_sound_data(UiSound::Navigate, Synth::triangle(1800.0).length(0.02).decay(150.0));
```

## Saving

```rust
let best: u32 = ctx.storage.get_or("best", 0);
ctx.storage.set("best", 42);          // written automatically when the game quits
ctx.storage.add("games_played", 1);
```

## Sharing data between scenes

```rust
ctx.insert(Settings::default());
ctx.get_mut::<Settings>().unwrap().volume = 0.5;
ctx.switch_scene(NextLevel::new(ctx));   // or push_scene / pop_scene
```
