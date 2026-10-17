//! The game loop: [`Engine`], [`Scene`] and [`Context`].

use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::audio::{Audio, Sound};
use crate::error::Result;
use crate::input::{Input, Key, KeyEvent};
use crate::physics::{BodyId, World};
use crate::shortcuts::Attached;
use crate::speech::Speech;
use crate::storage::Storage;
use crate::time::Time;
use crate::ui::{Ui, UiEvent};

/// One screen or state of your game: main menu, a level, the game over screen...
///
/// Only [`Scene::update`] is required. Scenes are switched with [`Context::switch_scene`],
/// stacked with [`Context::push_scene`] (e.g. a pause screen) and removed with
/// [`Context::pop_scene`].
pub trait Scene: 'static {
    /// Called once when the scene becomes active for the first time.
    fn start(&mut self, ctx: &mut Context) {
        let _ = ctx;
    }

    /// Called every frame. Put your game logic here.
    fn update(&mut self, ctx: &mut Context);

    /// Called at a fixed rate (60 times per second by default) right after each physics step.
    /// Use it for logic that must not depend on the frame rate.
    fn fixed_update(&mut self, ctx: &mut Context) {
        let _ = ctx;
    }

    /// Called for every UI event before [`Scene::update`]. An alternative to polling
    /// `ctx.ui.clicked(...)` and friends in `update`.
    fn on_ui(&mut self, event: &UiEvent, ctx: &mut Context) {
        let _ = (event, ctx);
    }

    /// Called when another scene is pushed on top of this one.
    fn pause(&mut self, ctx: &mut Context) {
        let _ = ctx;
    }

    /// Called when this scene is on top again after the scene above was popped.
    fn resume(&mut self, ctx: &mut Context) {
        let _ = ctx;
    }

    /// Called when the scene is removed (popped, switched away, or the game quits).
    fn end(&mut self, ctx: &mut Context) {
        let _ = ctx;
    }
}

enum SceneChange {
    Push(Box<dyn Scene>),
    Pop,
    Switch(Box<dyn Scene>),
}

/// Everything your game can use, passed to every [`Scene`] method.
pub struct Context {
    /// Frame timing and scheduling.
    pub time: Time,
    /// Keyboard state.
    pub input: Input,
    /// Sound effects, positioned sounds and music.
    pub audio: Audio,
    /// Screen reader / text-to-speech output.
    pub speech: Speech,
    /// Virtual menus and dialogs.
    pub ui: Ui,
    /// Physics world.
    pub physics: World,
    /// Values that are saved when the game quits (high scores, settings).
    pub storage: Storage,
    pub(crate) follow: Option<BodyId>,
    pub(crate) attached: Vec<Attached>,
    pub(crate) engine_sounds: Vec<(BodyId, Sound)>,
    pub(crate) once: HashSet<String>,
    title: String,
    quit: bool,
    change: Option<SceneChange>,
    globals: HashMap<TypeId, Box<dyn Any>>,
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context").field("title", &self.title).field("time", &self.time).finish_non_exhaustive()
    }
}

impl Context {
    /// Requests the game to end after this frame.
    pub fn quit(&mut self) {
        self.quit = true;
    }

    /// Whether [`Context::quit`] was called.
    pub fn is_quitting(&self) -> bool {
        self.quit
    }

    /// The game title given to [`Engine::new`].
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Replaces the current scene with another one after this frame.
    pub fn switch_scene(&mut self, scene: impl Scene) {
        self.change = Some(SceneChange::Switch(Box::new(scene)));
    }

    /// Puts a scene on top of the current one (which is paused) after this frame.
    pub fn push_scene(&mut self, scene: impl Scene) {
        self.change = Some(SceneChange::Push(Box::new(scene)));
    }

    /// Removes the current scene after this frame. Popping the last scene quits the game.
    pub fn pop_scene(&mut self) {
        self.change = Some(SceneChange::Pop);
    }

    /// Stores a value shared by all scenes (settings, score, save data...).
    /// There is one slot per type; storing again replaces the old value.
    pub fn insert<T: 'static>(&mut self, value: T) {
        self.globals.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Reads a shared value stored with [`Context::insert`].
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.globals.get(&TypeId::of::<T>()).and_then(|b| b.downcast_ref())
    }

    /// Mutable access to a shared value.
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.globals.get_mut(&TypeId::of::<T>()).and_then(|b| b.downcast_mut())
    }

    /// Mutable access to a shared value, inserting `T::default()` if it is missing.
    pub fn get_or_default<T: Default + 'static>(&mut self) -> &mut T {
        self.globals
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(T::default()))
            .downcast_mut()
            .expect("type id matches")
    }

    /// Removes a shared value and returns it.
    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.globals.remove(&TypeId::of::<T>()).and_then(|b| b.downcast().ok()).map(|b| *b)
    }
}

/// Configures and starts the game.
///
/// ```no_run
/// use audio_code::prelude::*;
///
/// struct Hello;
///
/// impl Scene for Hello {
///     fn start(&mut self, ctx: &mut Context) {
///         ctx.speech.speak("Hello! Press space to beep, escape to quit.");
///     }
///
///     fn update(&mut self, ctx: &mut Context) {
///         if ctx.input.pressed(Key::Space) {
///             ctx.audio.play_tone(440.0, 0.2);
///         }
///         if ctx.input.pressed(Key::Escape) {
///             ctx.quit();
///         }
///     }
/// }
///
/// fn main() -> audio_code::Result<()> {
///     Engine::new("Hello").run(Hello)
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Engine {
    pub(crate) title: String,
    pub(crate) fps: u32,
    pub(crate) physics_rate: u32,
    pub(crate) sound_dir: PathBuf,
    pub(crate) echo_speech: bool,
    pub(crate) window_size: (u32, u32),
    pub(crate) headless: bool,
    pub(crate) nvda_dll: Option<PathBuf>,
    pub(crate) gravity: glam::Vec3,
    pub(crate) save_file: Option<PathBuf>,
}

impl Engine {
    /// Creates a new engine configuration. The title is used for the window and is what the
    /// screen reader announces when the window gets focus.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            fps: 60,
            physics_rate: 60,
            sound_dir: PathBuf::from("sounds"),
            echo_speech: false,
            window_size: (640, 480),
            headless: false,
            nvda_dll: None,
            gravity: glam::Vec3::ZERO,
            save_file: None,
        }
    }

    /// Target frames per second (default 60).
    pub fn fps(mut self, fps: u32) -> Self {
        self.fps = fps.clamp(1, 1000);
        self
    }

    /// Physics and [`Scene::fixed_update`] steps per second (default 60).
    pub fn physics_rate(mut self, rate: u32) -> Self {
        self.physics_rate = rate.clamp(1, 1000);
        self
    }

    /// Gravity of the physics world, e.g. [`EARTH_GRAVITY`](crate::physics::EARTH_GRAVITY).
    /// Default is none (top down games). Can be changed later with `ctx.physics.set_gravity`.
    pub fn gravity(mut self, gravity: glam::Vec3) -> Self {
        self.gravity = gravity;
        self
    }

    /// File used by [`Context::storage`] (default `<title>.save` in the working directory).
    pub fn save_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.save_file = Some(path.into());
        self
    }

    /// Folder searched for sounds (default `"sounds"`). See [`crate::audio`].
    pub fn sound_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.sound_dir = dir.into();
        self
    }

    /// Print everything spoken to the console as well (default `false`).
    pub fn echo_speech(mut self, echo: bool) -> Self {
        self.echo_speech = echo;
        self
    }

    /// Window size in pixels (default 640×480). The window shows nothing, but sighted
    /// helpers can still find and focus it.
    pub fn window_size(mut self, width: u32, height: u32) -> Self {
        self.window_size = (width, height);
        self
    }

    /// Where to find NVDA's controller client DLL, used for speech when prism is not
    /// available (Windows, feature `nvda`). Accepts the DLL file or the folder containing it.
    /// Without this, the folder of the executable, the working directory and the system
    /// search path are tried.
    pub fn nvda_dll(mut self, path: impl Into<PathBuf>) -> Self {
        self.nvda_dll = Some(path.into());
        self
    }

    /// Use no audio device and print speech to the console instead of using prism.
    /// Meant for automated tests and tools.
    pub fn headless(mut self) -> Self {
        self.headless = true;
        self
    }

    /// Opens the window and runs the game until it quits.
    pub fn run(self, scene: impl Scene) -> Result<()> {
        crate::window::run(self, Box::new(scene))
    }

    /// Like [`Engine::run`], but builds the first scene with access to the [`Context`].
    /// Handy when the scene needs to create bodies or load sounds in its constructor:
    ///
    /// ```no_run
    /// # use audio_code::prelude::*;
    /// struct Level { player: BodyId }
    ///
    /// impl Level {
    ///     fn new(ctx: &mut Context) -> Self {
    ///         Self { player: ctx.physics.add(Body::kinematic(Vec3::ZERO)) }
    ///     }
    /// }
    /// # impl Scene for Level { fn update(&mut self, _: &mut Context) {} }
    ///
    /// fn main() -> audio_code::Result<()> {
    ///     Engine::new("Game").run_with(Level::new)
    /// }
    /// ```
    pub fn run_with<S: Scene>(self, build: impl FnOnce(&mut Context) -> S + 'static) -> Result<()> {
        self.run(Lazy::new(build))
    }

    /// Like [`Engine::run`], but uses a native wxWidgets frame (feature `wxdragon`).
    /// See [`crate::wx`] for embedding a game into an existing wxDragon application.
    #[cfg(feature = "wxdragon")]
    pub fn run_wx(self, scene: impl Scene) -> Result<()> {
        crate::wx::run(self, Box::new(scene))
    }

    /// Like [`Engine::run_with`] for a wxWidgets frame (feature `wxdragon`).
    #[cfg(feature = "wxdragon")]
    pub fn run_wx_with<S: Scene>(self, build: impl FnOnce(&mut Context) -> S + 'static) -> Result<()> {
        self.run_wx(Lazy::new(build))
    }

    /// Runs a game written as a single closure that is called every frame.
    /// The closure can keep its own state in captured variables. See [`crate::run`].
    pub fn run_fn(self, update: impl FnMut(&mut Context) + 'static) -> Result<()> {
        self.run(FnScene(update))
    }

    /// Like [`Engine::build`] for a closure game.
    pub fn build_fn(self, update: impl FnMut(&mut Context) + 'static) -> Runner {
        self.build(FnScene(update))
    }

    /// Creates the [`Context`] and runner without a window. Window backends use this; you
    /// can use it for tests or to drive the engine from your own loop.
    pub fn build(self, scene: impl Scene) -> Runner {
        Runner::new(self, Box::new(scene))
    }
}

/// A scene made from a closure.
struct FnScene<F>(F);

impl<F: FnMut(&mut Context) + 'static> Scene for FnScene<F> {
    fn update(&mut self, ctx: &mut Context) {
        (self.0)(ctx);
    }
}

/// A scene that is built on its first `start` call.
struct Lazy<S, F> {
    build: Option<F>,
    scene: Option<S>,
}

impl<S: Scene, F: FnOnce(&mut Context) -> S + 'static> Lazy<S, F> {
    fn new(build: F) -> Self {
        Self { build: Some(build), scene: None }
    }
}

impl<S: Scene, F: FnOnce(&mut Context) -> S + 'static> Scene for Lazy<S, F> {
    fn start(&mut self, ctx: &mut Context) {
        if let Some(build) = self.build.take() {
            let mut scene = build(ctx);
            scene.start(ctx);
            self.scene = Some(scene);
        }
    }
    fn update(&mut self, ctx: &mut Context) {
        if let Some(s) = &mut self.scene {
            s.update(ctx);
        }
    }
    fn fixed_update(&mut self, ctx: &mut Context) {
        if let Some(s) = &mut self.scene {
            s.fixed_update(ctx);
        }
    }
    fn on_ui(&mut self, event: &UiEvent, ctx: &mut Context) {
        if let Some(s) = &mut self.scene {
            s.on_ui(event, ctx);
        }
    }
    fn pause(&mut self, ctx: &mut Context) {
        if let Some(s) = &mut self.scene {
            s.pause(ctx);
        }
    }
    fn resume(&mut self, ctx: &mut Context) {
        if let Some(s) = &mut self.scene {
            s.resume(ctx);
        }
    }
    fn end(&mut self, ctx: &mut Context) {
        if let Some(s) = &mut self.scene {
            s.end(ctx);
        }
    }
}

/// Drives scenes frame by frame. Window backends feed it key events and call
/// [`Runner::frame`]. You only need this when writing your own backend or tests.
pub struct Runner {
    /// The context passed to scenes.
    pub ctx: Context,
    scenes: Vec<Box<dyn Scene>>,
    started: bool,
    accumulator: f32,
    fixed_dt: f32,
    frame_time: Duration,
    next_frame: Instant,
}

impl std::fmt::Debug for Runner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runner").field("scenes", &self.scenes.len()).finish_non_exhaustive()
    }
}

impl Runner {
    pub(crate) fn new(config: Engine, scene: Box<dyn Scene>) -> Self {
        let ctx = Context {
            time: Time::new(),
            input: Input::default(),
            audio: if config.headless {
                Audio::silent(config.sound_dir.clone())
            } else {
                Audio::new(config.sound_dir.clone())
            },
            speech: if config.headless { Speech::console() } else { Speech::new(config.echo_speech, config.nvda_dll.clone()) },
            ui: Ui::default(),
            physics: {
                let mut world = World::new();
                world.set_gravity(config.gravity);
                world
            },
            storage: if config.headless {
                Storage::in_memory()
            } else {
                Storage::open(config.save_file.clone().unwrap_or_else(|| crate::storage::default_file(&config.title)))
            },
            follow: None,
            attached: Vec::new(),
            engine_sounds: Vec::new(),
            once: HashSet::new(),
            title: config.title.clone(),
            quit: false,
            change: None,
            globals: HashMap::new(),
        };
        Self {
            ctx,
            scenes: vec![scene],
            started: false,
            accumulator: 0.0,
            fixed_dt: 1.0 / config.physics_rate as f32,
            frame_time: Duration::from_secs_f64(1.0 / config.fps as f64),
            next_frame: Instant::now(),
        }
    }

    /// Reports a key going down. `repeat` is `true` for OS key repeats.
    pub fn key_down(&mut self, key: Key, repeat: bool) {
        self.ctx.input.push(KeyEvent::Down { key, repeat });
    }

    /// Reports a key going up.
    pub fn key_up(&mut self, key: Key) {
        self.ctx.input.push(KeyEvent::Up { key });
    }

    /// Reports typed text.
    pub fn text(&mut self, text: &str) {
        self.ctx.input.push_text(text);
    }

    /// Releases all keys (call when the window loses focus).
    pub fn focus_lost(&mut self) {
        self.ctx.input.release_all();
    }

    /// When the next frame is due.
    pub fn next_frame_time(&self) -> Instant {
        self.next_frame
    }

    /// Whether the game wants to quit.
    pub fn should_quit(&self) -> bool {
        self.ctx.quit
    }

    /// Runs one frame now. Returns `false` when the game has quit.
    pub fn frame(&mut self) -> bool {
        let now = Instant::now();
        // Schedule the next frame, but never try to catch up on missed ones.
        self.next_frame = (self.next_frame + self.frame_time).max(now);

        if !self.started {
            self.started = true;
            self.ctx.time.advance(now);
            if let Some(scene) = self.scenes.last_mut() {
                scene.start(&mut self.ctx);
            }
            self.apply_change();
            return !self.ctx.quit;
        }

        let ctx = &mut self.ctx;
        ctx.time.advance(now);
        ctx.input.begin_frame(now);

        let consumed = ctx.ui.update(&ctx.input, &mut ctx.speech, &mut ctx.audio);
        ctx.input.set_blocked(consumed || ctx.ui.is_open());

        if let Some(scene) = self.scenes.last_mut() {
            for event in self.ctx.ui.take_events() {
                scene.on_ui(&event, &mut self.ctx);
            }

            self.ctx.physics.clear_events();
            self.accumulator += self.ctx.time.delta();
            let mut steps = 0;
            while self.accumulator >= self.fixed_dt && steps < 8 {
                self.accumulator -= self.fixed_dt;
                self.ctx.physics.step(self.fixed_dt);
                scene.fixed_update(&mut self.ctx);
                steps += 1;
            }
            if steps == 8 {
                self.accumulator = 0.0;
            }

            scene.update(&mut self.ctx);
        }
        self.ctx.sync_world();
        self.ctx.sync_vehicles();
        self.apply_change();

        if self.ctx.quit {
            self.shutdown();
            return false;
        }
        true
    }

    fn apply_change(&mut self) {
        // A scene's start() may itself request another change, so loop.
        while let Some(change) = self.ctx.change.take() {
            match change {
                SceneChange::Push(mut scene) => {
                    if let Some(top) = self.scenes.last_mut() {
                        top.pause(&mut self.ctx);
                    }
                    scene.start(&mut self.ctx);
                    self.scenes.push(scene);
                }
                SceneChange::Pop => {
                    if let Some(mut top) = self.scenes.pop() {
                        top.end(&mut self.ctx);
                    }
                    match self.scenes.last_mut() {
                        Some(top) => top.resume(&mut self.ctx),
                        None => self.ctx.quit = true,
                    }
                }
                SceneChange::Switch(mut scene) => {
                    if let Some(mut top) = self.scenes.pop() {
                        top.end(&mut self.ctx);
                    }
                    scene.start(&mut self.ctx);
                    self.scenes.push(scene);
                }
            }
        }
    }

    /// Ends all scenes. Called automatically when the game quits.
    pub fn shutdown(&mut self) {
        while let Some(mut scene) = self.scenes.pop() {
            scene.end(&mut self.ctx);
        }
        if let Err(e) = self.ctx.storage.save() {
            eprintln!("audio-code: could not save storage: {e}");
        }
        self.ctx.quit = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Recorder(Rc<RefCell<Vec<String>>>, &'static str);

    impl Scene for Recorder {
        fn start(&mut self, _: &mut Context) {
            self.0.borrow_mut().push(format!("{} start", self.1));
        }
        fn update(&mut self, ctx: &mut Context) {
            self.0.borrow_mut().push(format!("{} update", self.1));
            if self.1 == "a" && ctx.input.pressed(Key::Enter) {
                ctx.push_scene(Recorder(self.0.clone(), "b"));
            }
            if self.1 == "b" && ctx.input.pressed(Key::Escape) {
                ctx.pop_scene();
            }
        }
        fn pause(&mut self, _: &mut Context) {
            self.0.borrow_mut().push(format!("{} pause", self.1));
        }
        fn resume(&mut self, _: &mut Context) {
            self.0.borrow_mut().push(format!("{} resume", self.1));
        }
        fn end(&mut self, _: &mut Context) {
            self.0.borrow_mut().push(format!("{} end", self.1));
        }
    }

    #[test]
    fn scene_stack() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut runner = Engine::new("test").headless().build(Recorder(log.clone(), "a"));
        runner.frame();
        runner.key_down(Key::Enter, false);
        runner.frame();
        runner.key_up(Key::Enter);
        runner.key_down(Key::Escape, false);
        runner.frame();
        runner.key_up(Key::Escape);
        runner.key_down(Key::Escape, false);
        runner.frame();
        assert_eq!(
            *log.borrow(),
            ["a start", "a update", "a pause", "b start", "b update", "b end", "a resume", "a update"]
        );
    }

    #[test]
    fn globals() {
        let mut runner = Engine::new("test").headless().build(Recorder(Rc::default(), "x"));
        runner.ctx.insert(5u32);
        *runner.ctx.get_mut::<u32>().unwrap() += 1;
        assert_eq!(runner.ctx.get::<u32>(), Some(&6));
        assert_eq!(*runner.ctx.get_or_default::<String>(), "");
    }
}
