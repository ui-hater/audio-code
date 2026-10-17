# Scenes and shared data

## The scene lifecycle

```rust
impl Scene for Level {
    fn start(&mut self, ctx: &mut Context) {}          // once, when it becomes active
    fn update(&mut self, ctx: &mut Context) {}         // every frame (required)
    fn fixed_update(&mut self, ctx: &mut Context) {}   // after every physics step
    fn on_ui(&mut self, e: &UiEvent, ctx: &mut Context) {} // for each UI event
    fn pause(&mut self, ctx: &mut Context) {}          // another scene was pushed on top
    fn resume(&mut self, ctx: &mut Context) {}         // back on top
    fn end(&mut self, ctx: &mut Context) {}            // removed or game quits
}
```

Each frame runs in this order: input → UI → `on_ui` → physics steps with `fixed_update` →
`update` → scene changes.

## Switching scenes

```rust
ctx.switch_scene(Level::new(2));   // replace the current scene
ctx.push_scene(PauseMenu);         // put on top, current scene is paused
ctx.pop_scene();                   // back to the scene below
ctx.quit();                        // end the game
```

Changes take effect at the end of the current frame. Popping the last scene quits the game.

Scenes often need the context while they are built, for example to add bodies. Inside a
scene you already have `ctx`:

```rust
let level = Level::new(ctx, 2);
ctx.switch_scene(level);
```

For the first scene, use `run_with`:

```rust
fn main() -> audio_code::Result<()> {
    Engine::new("Game").run_with(|ctx| Level::new(ctx, 1))
}
```

## A pause menu

```rust
struct Pause;

impl Scene for Pause {
    fn start(&mut self, ctx: &mut Context) {
        ctx.time.pause();
        ctx.ui.open(Menu::new("pause", "Paused").button("resume", "Resume").button("quit", "Quit"));
    }

    fn update(&mut self, ctx: &mut Context) {
        if ctx.ui.clicked("pause", "resume") || ctx.ui.closed("pause") {
            ctx.ui.close_id("pause");
            ctx.pop_scene();
        }
        if ctx.ui.clicked("pause", "quit") {
            ctx.quit();
        }
    }

    fn end(&mut self, ctx: &mut Context) {
        ctx.time.resume();
    }
}

// in the level:
if ctx.input.pressed(Key::Escape) {
    ctx.push_scene(Pause);
}
```

## Shared data

Store data that several scenes need (settings, score, unlocked levels) in the context. There
is one slot per type:

```rust
#[derive(Default)]
struct Progress {
    level: u32,
    score: u64,
}

ctx.insert(Progress::default());
ctx.get::<Progress>();                 // Option<&Progress>
ctx.get_mut::<Progress>();             // Option<&mut Progress>
ctx.get_or_default::<Progress>().score += 10;
ctx.remove::<Progress>();
```

The physics world, the audio cache and the UI stack are shared by all scenes. Call
`ctx.physics.clear()` when a new level starts.

## Engine settings

```rust
Engine::new("My game")
    .fps(60)                 // frames per second
    .physics_rate(60)        // physics steps per second
    .sound_dir("assets/sfx") // where sounds are looked up
    .echo_speech(true)       // print speech to the console
    .window_size(800, 600)
    .run(MainMenu)
```

`Engine::headless()` uses no audio device and prints speech to the console. Together with
`Engine::build(scene)` you can drive a game frame by frame in tests:

```rust
let mut runner = Engine::new("test").headless().build(MyScene::default());
runner.frame();
runner.key_down(Key::Space, false);
runner.frame();
assert!(runner.ctx.speech.last() == Some("Jump"));
```
