//! Virtual, speech based user interface. Available as `ctx.ui`.
//!
//! Nothing here is drawn on screen. Every widget announces itself through
//! [`Speech`] and plays short interface sounds, and reacts to the
//! keyboard directly. While any widget is open it owns the keyboard: game input is blocked
//! (see [`Input::is_blocked`](crate::input::Input::is_blocked)).
//!
//! Widgets form a stack. The top one receives input; closing it returns to the one below,
//! which re-announces itself. This makes sub menus trivial.
//!
//! ```no_run
//! # use audio_code::prelude::*;
//! # fn f(ctx: &mut Context) {
//! ctx.ui.open(
//!     Menu::new("main", "Main menu")
//!         .button("play", "Play")
//!         .slider("volume", "Volume", 80.0, 0.0, 100.0, 10.0)
//!         .checkbox("music", "Music", true)
//!         .button("quit", "Quit"),
//! );
//! // ...later, in update():
//! if ctx.ui.clicked("main", "play") {
//!     ctx.ui.close();
//! }
//! if let Some(volume) = ctx.ui.changed("main", "volume") {
//!     ctx.audio.set_master_volume(volume.as_f32() / 100.0);
//! }
//! # }
//! ```
//!
//! Built in widgets: [`Menu`], [`Message`], [`Confirm`] and [`TextInput`]. You can write your
//! own by implementing [`Widget`].

mod dialog;
mod menu;
mod sounds;

pub use dialog::{Confirm, Message, TextInput};
pub use menu::{ItemKind, Menu, MenuItem};
pub use sounds::{UiSound, UiSounds};

use crate::audio::{Audio, SoundData};
use crate::input::{Input, Key, RawInput};
use crate::speech::Speech;

/// A value carried by a UI event or stored in a menu item.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Checkbox state.
    Bool(bool),
    /// Slider value.
    Number(f32),
    /// Selected option of a choice: index and text.
    Choice(usize, String),
    /// Text field contents.
    Text(String),
}

impl Value {
    /// The value as `bool`. Numbers are `true` when not zero, text when not empty.
    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::Choice(i, _) => *i != 0,
            Value::Text(t) => !t.is_empty(),
        }
    }

    /// The value as number (choices give their index, text is parsed, `bool` gives 0 or 1).
    pub fn as_f32(&self) -> f32 {
        match self {
            Value::Bool(b) => *b as u8 as f32,
            Value::Number(n) => *n,
            Value::Choice(i, _) => *i as f32,
            Value::Text(t) => t.trim().parse().unwrap_or(0.0),
        }
    }

    /// The value as index (for choices) or rounded number.
    pub fn as_index(&self) -> usize {
        self.as_f32().max(0.0).round() as usize
    }

    /// The value as text.
    pub fn as_str(&self) -> &str {
        match self {
            Value::Bool(true) => "on",
            Value::Bool(false) => "off",
            Value::Number(_) => "",
            Value::Choice(_, s) | Value::Text(s) => s,
        }
    }

    /// The value formatted for speaking.
    pub fn to_text(&self) -> String {
        match self {
            Value::Number(n) => format_number(*n),
            other => other.as_str().to_string(),
        }
    }
}

pub(crate) fn format_number(n: f32) -> String {
    if (n - n.round()).abs() < 1e-4 { format!("{}", n.round() as i64) } else { format!("{n:.2}") }
}

/// Something that happened in the UI this frame.
#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    /// A menu button was pressed.
    Activated {
        /// Menu id.
        menu: String,
        /// Item id.
        item: String,
    },
    /// A checkbox, slider, choice or text field in a menu changed.
    Changed {
        /// Menu id.
        menu: String,
        /// Item id.
        item: String,
        /// New value.
        value: Value,
    },
    /// A widget was closed by the player (Escape, or Enter on a message).
    Closed {
        /// Widget id.
        id: String,
    },
    /// The player answered a [`Confirm`] dialog.
    Confirmed {
        /// Widget id.
        id: String,
        /// `true` for yes.
        yes: bool,
    },
    /// The player pressed Enter in a [`TextInput`].
    TextSubmitted {
        /// Widget id.
        id: String,
        /// The entered text.
        text: String,
    },
}

/// What a widget wants to happen after its update.
#[derive(Default)]
pub enum Status {
    /// Stay open.
    #[default]
    Open,
    /// Close this widget.
    Close,
    /// Close this widget and hand a value to the widget below
    /// (it receives it in [`Widget::resume`]).
    CloseWith(Value),
    /// Open another widget on top of this one.
    Push(Box<dyn Widget>),
}

/// A virtual UI element. Implement this to create your own widgets.
pub trait Widget: 'static {
    /// Identifier used in events.
    fn id(&self) -> &str;

    /// Called once when the widget becomes active. Announce yourself here.
    fn open(&mut self, ui: &mut UiContext<'_>);

    /// Called every frame while this widget is on top of the stack.
    fn update(&mut self, ui: &mut UiContext<'_>) -> Status;

    /// Called when the widget becomes the top again after the one above closed.
    /// `value` is what the closed widget passed with [`Status::CloseWith`].
    fn resume(&mut self, value: Option<Value>, ui: &mut UiContext<'_>) {
        let _ = value;
        self.open(ui);
    }

    /// Current value of an item, used by [`Ui::value`].
    fn value(&self, item: &str) -> Option<Value> {
        let _ = item;
        None
    }

    /// Music that plays while this widget is open, see [`Menu::music`].
    /// Widgets above it without music of their own keep it playing.
    fn music(&self) -> Option<&str> {
        None
    }
}

/// Everything a widget may use while it updates.
pub struct UiContext<'a> {
    /// Keyboard state (never blocked for widgets).
    pub input: RawInput<'a>,
    /// Speech output.
    pub speech: &'a mut Speech,
    /// Audio output.
    pub audio: &'a mut Audio,
    sounds: &'a UiSounds,
    events: &'a mut Vec<UiEvent>,
}

impl UiContext<'_> {
    /// Reports an event to the game.
    pub fn emit(&mut self, event: UiEvent) {
        self.events.push(event);
    }

    /// Plays one of the interface sounds, using `custom` if given, else the global set.
    pub fn play(&mut self, sound: UiSound, custom: Option<&UiSounds>) {
        let set = custom.unwrap_or(self.sounds);
        if let Some(name) = set.get(sound) {
            let name = name.to_string();
            self.play_named(&name);
        }
    }

    /// Plays a sound by name in the UI volume group.
    pub fn play_named(&mut self, name: &str) {
        sounds::ensure_builtin(self.audio, name);
        self.audio.sound(name).group(crate::audio::Group::Ui).play();
    }

    /// Shortcut: key went down or repeats.
    pub fn key(&self, key: Key) -> bool {
        self.input.pressed_or_repeated(key)
    }
}

enum State {
    Fresh,
    Active,
    Resume(Option<Value>),
}

struct Entry {
    widget: Box<dyn Widget>,
    state: State,
}

/// The widget stack. Reach it through `ctx.ui`.
#[derive(Default)]
pub struct Ui {
    stack: Vec<Entry>,
    events: Vec<UiEvent>,
    sounds: UiSounds,
    /// Sounds made with `set_sound_data`, registered with the audio system on the next update.
    pending_sounds: Vec<(String, SoundData)>,
    /// Widget music that the UI started and that is playing now.
    music: Option<String>,
    /// Music that played before the widget music, restored when no widget wants music.
    previous_music: Option<String>,
}

impl std::fmt::Debug for Ui {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ids: Vec<&str> = self.stack.iter().map(|e| e.widget.id()).collect();
        f.debug_struct("Ui").field("stack", &ids).field("events", &self.events).finish()
    }
}



impl Ui {
    /// Runs the top widget. Returns `true` if the UI used the keyboard this frame.
    pub(crate) fn update(&mut self, input: &Input, speech: &mut Speech, audio: &mut Audio) -> bool {
        self.events.clear();
        for (name, data) in self.pending_sounds.drain(..) {
            audio.register(name, data);
        }
        self.update_music(audio);
        let Some(top) = self.stack.last_mut() else {
            return false;
        };
        let mut ui = UiContext { input: input.raw(), speech, audio, sounds: &self.sounds, events: &mut self.events };
        let status = match std::mem::replace(&mut top.state, State::Active) {
            State::Fresh => {
                top.widget.open(&mut ui);
                Status::Open
            }
            State::Resume(value) => {
                top.widget.resume(value, &mut ui);
                Status::Open
            }
            State::Active => top.widget.update(&mut ui),
        };
        match status {
            Status::Open => {}
            Status::Close => self.pop(None),
            Status::CloseWith(value) => self.pop(Some(value)),
            Status::Push(widget) => self.stack.push(Entry { widget, state: State::Fresh }),
        }
        true
    }

    /// Plays the music of the topmost widget that has some. When no widget wants music any
    /// more, the music from before comes back, unless the game started other music meanwhile.
    fn update_music(&mut self, audio: &mut Audio) {
        let wanted = self.stack.iter().rev().find_map(|e| e.widget.music());
        if wanted == self.music.as_deref() {
            return;
        }
        match wanted {
            Some(name) => {
                if self.music.is_none() {
                    self.previous_music = audio.music_name().map(String::from);
                }
                audio.play_music(name);
                self.music = Some(name.to_string());
            }
            None => {
                let ours = audio.music_name().is_some() && audio.music_name() == self.music.as_deref();
                let previous = self.previous_music.take();
                self.music = None;
                if ours {
                    match previous {
                        Some(name) => {
                            audio.play_music(&name);
                        }
                        None => audio.stop_music(1.0),
                    }
                }
            }
        }
    }

    fn pop(&mut self, value: Option<Value>) {
        self.stack.pop();
        if let Some(below) = self.stack.last_mut() {
            below.state = State::Resume(value);
        }
    }

    /// Opens a widget on top of the stack. It announces itself on the next frame.
    pub fn open(&mut self, widget: impl Widget) {
        self.stack.push(Entry { widget: Box::new(widget), state: State::Fresh });
    }

    /// Opens a boxed widget.
    pub fn open_boxed(&mut self, widget: Box<dyn Widget>) {
        self.stack.push(Entry { widget, state: State::Fresh });
    }

    /// Shows a message. Enter, Space or Escape closes it and emits [`UiEvent::Closed`].
    /// Multi line text can be reviewed line by line with the arrow keys.
    pub fn message(&mut self, id: impl Into<String>, text: impl Into<String>) {
        self.open(Message::new(id, text));
    }

    /// Asks a yes/no question. The answer arrives as [`UiEvent::Confirmed`]; see [`Ui::confirmed`].
    pub fn confirm(&mut self, id: impl Into<String>, question: impl Into<String>) {
        self.open(Confirm::new(id, question));
    }

    /// Asks the player to type text. The result arrives as [`UiEvent::TextSubmitted`];
    /// see [`Ui::submitted`].
    pub fn input(&mut self, id: impl Into<String>, prompt: impl Into<String>) {
        self.open(TextInput::new(id, prompt));
    }

    /// Closes the top widget without any sound or event.
    pub fn close(&mut self) {
        self.pop(None);
    }

    /// Closes the widget with the given id and everything above it.
    pub fn close_id(&mut self, id: &str) {
        if let Some(i) = self.stack.iter().position(|e| e.widget.id() == id) {
            self.stack.truncate(i + 1);
            self.pop(None);
        }
    }

    /// Closes every widget.
    pub fn close_all(&mut self) {
        self.stack.clear();
    }

    /// Whether any widget is open.
    pub fn is_open(&self) -> bool {
        !self.stack.is_empty()
    }

    /// Whether a widget with this id is anywhere on the stack.
    pub fn contains(&self, id: &str) -> bool {
        self.stack.iter().any(|e| e.widget.id() == id)
    }

    /// Id of the widget that currently has the keyboard.
    pub fn top(&self) -> Option<&str> {
        self.stack.last().map(|e| e.widget.id())
    }

    /// Replaces the interface sounds used by all widgets. Use [`UiSounds::none`] for silence.
    pub fn set_sounds(&mut self, sounds: UiSounds) {
        self.sounds = sounds;
    }

    /// The interface sounds used by all widgets.
    pub fn sounds(&self) -> &UiSounds {
        &self.sounds
    }

    /// Uses the sound `name` (a file or registered sound) for one kind of interface sound.
    /// `None` makes it silent.
    ///
    /// ```no_run
    /// # use audio_code::prelude::*;
    /// # fn f(ctx: &mut Context) {
    /// ctx.ui.set_sound(UiSound::Navigate, Some("ui/click"));   // sounds/ui/click.ogg
    /// ctx.ui.set_sound(UiSound::Edge, None);
    /// # }
    /// ```
    pub fn set_sound(&mut self, kind: UiSound, name: Option<&str>) {
        self.sounds.set(kind, name.map(String::from));
    }

    /// Uses a sound made in code for one kind of interface sound, typically a [`Synth`]:
    ///
    /// ```no_run
    /// # use audio_code::prelude::*;
    /// # fn f(ctx: &mut Context) {
    /// ctx.ui.set_sound_data(UiSound::Navigate, Synth::triangle(1800.0).length(0.02).decay(150.0));
    /// ctx.ui.set_sound_data(UiSound::Typing, Synth::noise().length(0.01).volume(0.2));
    /// # }
    /// ```
    ///
    /// [`Synth`]: crate::audio::Synth
    pub fn set_sound_data(&mut self, kind: UiSound, data: impl Into<SoundData>) {
        let name = format!("ui:custom:{}", kind.name());
        self.pending_sounds.push((name.clone(), data.into()));
        self.sounds.set(kind, Some(name));
    }

    /// Current value of a menu item while the menu is open.
    pub fn value(&self, menu: &str, item: &str) -> Option<Value> {
        self.stack.iter().rev().find(|e| e.widget.id() == menu).and_then(|e| e.widget.value(item))
    }

    // ----------------------------------------------------------------- events

    /// All UI events of this frame.
    pub fn events(&self) -> &[UiEvent] {
        &self.events
    }

    pub(crate) fn take_events(&mut self) -> Vec<UiEvent> {
        self.events.clone()
    }

    /// Id of the button activated in `menu` this frame.
    pub fn activated(&self, menu: &str) -> Option<&str> {
        self.events.iter().find_map(|e| match e {
            UiEvent::Activated { menu: m, item } if m == menu => Some(item.as_str()),
            _ => None,
        })
    }

    /// Whether button `item` in `menu` was activated this frame.
    pub fn clicked(&self, menu: &str, item: &str) -> bool {
        self.activated(menu) == Some(item)
    }

    /// New value of `item` in `menu` if it changed this frame.
    pub fn changed(&self, menu: &str, item: &str) -> Option<&Value> {
        self.events.iter().find_map(|e| match e {
            UiEvent::Changed { menu: m, item: i, value } if m == menu && i == item => Some(value),
            _ => None,
        })
    }

    /// Whether the widget `id` was closed by the player this frame.
    pub fn closed(&self, id: &str) -> bool {
        self.events.iter().any(|e| matches!(e, UiEvent::Closed { id: i } if i == id))
    }

    /// The answer to confirm dialog `id`, if given this frame.
    pub fn confirmed(&self, id: &str) -> Option<bool> {
        self.events.iter().find_map(|e| match e {
            UiEvent::Confirmed { id: i, yes } if i == id => Some(*yes),
            _ => None,
        })
    }

    /// Text submitted in text input `id` this frame.
    pub fn submitted(&self, id: &str) -> Option<&str> {
        self.events.iter().find_map(|e| match e {
            UiEvent::TextSubmitted { id: i, text } if i == id => Some(text.as_str()),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyEvent;
    use std::time::Instant;

    pub(crate) struct Harness {
        pub ui: Ui,
        pub input: Input,
        pub speech: Speech,
        pub audio: Audio,
    }

    impl Harness {
        pub fn new() -> Self {
            let mut speech = Speech::console();
            speech.set_echo(false);
            Self { ui: Ui::default(), input: Input::default(), speech, audio: Audio::silent("sounds") }
        }

        pub fn frame(&mut self, keys: &[Key], text: &str) {
            for &key in keys {
                self.input.push(KeyEvent::Down { key, repeat: false });
                self.input.push(KeyEvent::Up { key });
            }
            self.input.push_text(text);
            self.input.begin_frame(Instant::now());
            self.ui.update(&self.input, &mut self.speech, &mut self.audio);
        }
    }

    #[test]
    fn menu_navigation_and_activation() {
        let mut h = Harness::new();
        h.ui.open(Menu::new("main", "Main menu").button("play", "Play").button("quit", "Quit"));
        h.frame(&[], "");
        assert_eq!(h.speech.last(), Some("Main menu. Play"));
        h.frame(&[Key::Down], "");
        assert_eq!(h.speech.last(), Some("Quit"));
        h.frame(&[Key::Enter], "");
        assert!(h.ui.clicked("main", "quit"));
        h.frame(&[Key::Escape], "");
        assert!(h.ui.closed("main"));
        assert!(!h.ui.is_open());
    }

    #[test]
    fn slider_and_first_letter() {
        let mut h = Harness::new();
        h.ui.open(
            Menu::new("opts", "")
                .slider("vol", "Volume", 50.0, 0.0, 100.0, 10.0)
                .checkbox("music", "Music", false)
                .button("back", "Back"),
        );
        h.frame(&[], "");
        h.frame(&[Key::Right], "");
        assert_eq!(h.ui.changed("opts", "vol"), Some(&Value::Number(60.0)));
        assert_eq!(h.ui.value("opts", "vol"), Some(Value::Number(60.0)));
        h.frame(&[], "b");
        assert_eq!(h.speech.last(), Some("Back"));
        h.frame(&[], "m");
        h.frame(&[Key::Space], "");
        assert_eq!(h.ui.changed("opts", "music"), Some(&Value::Bool(true)));
    }

    #[test]
    fn text_field_in_menu() {
        let mut h = Harness::new();
        h.ui.open(Menu::new("m", "").text_field("name", "Name", ""));
        h.frame(&[], "");
        h.frame(&[Key::Enter], "");
        h.frame(&[], ""); // text input opens
        h.frame(&[], "Bob");
        h.frame(&[Key::Backspace], "");
        h.frame(&[Key::Enter], "");
        h.frame(&[], ""); // menu resumes
        assert_eq!(h.ui.value("m", "name"), Some(Value::Text("Bo".into())));
        assert_eq!(h.speech.last(), Some("Name, edit, Bo"));
    }

    #[test]
    fn custom_sounds() {
        let mut h = Harness::new();
        h.ui.set_sound_data(UiSound::Navigate, crate::audio::Synth::sine(2000.0).length(0.02));
        h.ui.set_sound(UiSound::Edge, None);
        assert_eq!(h.ui.sounds().get(UiSound::Navigate), Some("ui:custom:navigate"));
        assert_eq!(h.ui.sounds().get(UiSound::Wrap), Some("ui:builtin:wrap"));
        assert_eq!(h.ui.sounds().edge, None);
        h.ui.open(Menu::new("m", "").button("a", "A").sound(UiSound::Open, None));
        h.frame(&[], "");
        assert!(h.audio.is_loaded("ui:custom:navigate"));
        let only_nav = UiSounds::none().with(UiSound::Navigate, "x");
        assert_eq!(only_nav.get(UiSound::Wrap), Some("x"));
    }

    #[test]
    fn positions_on_request() {
        let mut h = Harness::new();
        h.ui.open(Menu::new("m", "").button("a", "A").button("b", "B").positions(true));
        h.frame(&[], "");
        assert_eq!(h.speech.last(), Some("A, 1 of 2"));
    }

    #[test]
    fn menu_music() {
        let mut h = Harness::new();
        let tone = crate::audio::Synth::sine(440.0).length(0.5);
        for name in ["level", "theme", "options"] {
            h.audio.register(name, tone.clone());
        }
        h.audio.play_music("level");
        h.ui.open(Menu::new("pause", "Paused").button("opts", "Options").music("theme"));
        h.frame(&[], "");
        assert_eq!(h.audio.music_name(), Some("theme"));
        h.ui.open(Menu::new("sub", "").button("x", "X"));
        h.frame(&[], "");
        assert_eq!(h.audio.music_name(), Some("theme"), "submenu without music keeps it");
        h.ui.open(Menu::new("opts", "").button("x", "X").music("options"));
        h.frame(&[], "");
        assert_eq!(h.audio.music_name(), Some("options"));
        h.ui.close();
        h.frame(&[], "");
        assert_eq!(h.audio.music_name(), Some("theme"));
        h.ui.close_all();
        h.frame(&[], "");
        assert_eq!(h.audio.music_name(), Some("level"), "music from before comes back");

        h.ui.open(Menu::new("main", "").button("play", "Play").music("theme"));
        h.frame(&[], "");
        h.audio.play_music("options"); // the game starts its own music
        h.ui.close_all();
        h.frame(&[], "");
        assert_eq!(h.audio.music_name(), Some("options"), "game music is left alone");
    }

    #[test]
    fn confirm_dialog() {
        let mut h = Harness::new();
        h.ui.confirm("quit", "Really quit?");
        h.frame(&[], "");
        h.frame(&[Key::Enter], "");
        assert_eq!(h.ui.confirmed("quit"), Some(true));
        h.ui.confirm("quit", "Really quit?");
        h.frame(&[], "");
        h.frame(&[Key::Escape], "");
        assert_eq!(h.ui.confirmed("quit"), Some(false));
    }
}
