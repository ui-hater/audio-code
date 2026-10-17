//! Keyboard input. Available as `ctx.input`.
//!
//! The engine collects key events between frames, so a key that is pressed and released
//! within a single frame is still reported by [`Input::pressed`].
//!
//! While a virtual UI element (menu, dialog, text field) is open it owns the keyboard and
//! all game-facing queries return `false`. Use [`Input::raw`] if you really need to peek.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// A key on the keyboard.
///
/// Letters and digits follow the label printed on the key in the active keyboard layout,
/// so `Key::Z` is the key labelled "Z" on both QWERTY and QWERTZ keyboards.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Up, Down, Left, Right,
    Enter, Escape, Space, Tab, Backspace, Delete, Insert,
    Home, End, PageUp, PageDown,
    Shift, Control, Alt,
    Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
    NumpadAdd, NumpadSubtract, NumpadMultiply, NumpadDivide, NumpadDecimal, NumpadEnter,
    Comma, Period, Minus, Plus, Slash, Backslash, Semicolon, Quote, LeftBracket, RightBracket, Backquote,
}

impl Key {
    /// Maps a letter or digit to its key (case insensitive).
    pub fn from_char(c: char) -> Option<Key> {
        use Key::*;
        const LETTERS: [Key; 26] =
            [A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z];
        const DIGITS: [Key; 10] =
            [Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9];
        let c = c.to_ascii_lowercase();
        match c {
            'a'..='z' => Some(LETTERS[(c as u8 - b'a') as usize]),
            '0'..='9' => Some(DIGITS[(c as u8 - b'0') as usize]),
            _ => None,
        }
    }

    /// A short human readable name, e.g. "Page Up". Handy for "press X to continue" messages.
    pub fn name(self) -> String {
        let debug = format!("{self:?}");
        if let Some(d) = debug.strip_prefix("Digit") {
            return d.to_string();
        }
        // Split CamelCase into words: "PageUp" -> "Page Up".
        let mut out = String::new();
        for (i, c) in debug.chars().enumerate() {
            if i > 0 && c.is_uppercase() {
                out.push(' ');
            }
            out.push(c);
        }
        out
    }
}

/// State of the modifier keys.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    /// Either shift key is held.
    pub shift: bool,
    /// Either control key is held.
    pub control: bool,
    /// Either alt key is held.
    pub alt: bool,
}

/// A raw key event coming from the window backend.
#[derive(Debug, Clone, Copy)]
pub(crate) enum KeyEvent {
    Down { key: Key, repeat: bool },
    Up { key: Key },
}

/// Keyboard state for the current frame.
#[derive(Debug, Default)]
pub struct Input {
    queue: Vec<KeyEvent>,
    text_queue: String,

    held: HashMap<Key, Instant>,
    pressed: HashSet<Key>,
    repeated: HashSet<Key>,
    released: HashSet<Key>,
    order: Vec<Key>,
    text: String,
    held_now: HashMap<Key, f32>,
    held_prev: HashMap<Key, f32>,

    blocked: bool,
}

impl Input {
    pub(crate) fn push(&mut self, event: KeyEvent) {
        self.queue.push(event);
    }

    pub(crate) fn push_text(&mut self, text: &str) {
        self.text_queue.extend(text.chars().filter(|c| !c.is_control()));
    }

    /// Forgets all held keys, e.g. when the window loses focus.
    pub(crate) fn release_all(&mut self) {
        let keys: Vec<Key> = self.held.keys().copied().collect();
        for key in keys {
            self.queue.push(KeyEvent::Up { key });
        }
    }

    /// Turns the queued events into this frame's state.
    pub(crate) fn begin_frame(&mut self, now: Instant) {
        self.pressed.clear();
        self.repeated.clear();
        self.released.clear();
        self.order.clear();
        self.text = std::mem::take(&mut self.text_queue);
        for event in std::mem::take(&mut self.queue) {
            match event {
                KeyEvent::Down { key, repeat } => {
                    if repeat || self.held.contains_key(&key) {
                        self.repeated.insert(key);
                    } else {
                        self.held.insert(key, now);
                        self.pressed.insert(key);
                    }
                    self.order.push(key);
                }
                KeyEvent::Up { key } => {
                    if self.held.remove(&key).is_some() {
                        self.released.insert(key);
                    }
                }
            }
        }
        self.held_prev = std::mem::take(&mut self.held_now);
        for (key, since) in &self.held {
            self.held_now.insert(*key, now.duration_since(*since).as_secs_f32());
        }
    }

    pub(crate) fn set_blocked(&mut self, blocked: bool) {
        self.blocked = blocked;
    }

    /// Whether game input is currently blocked by an open UI element.
    pub fn is_blocked(&self) -> bool {
        self.blocked
    }

    /// `true` in the frame the key went down. OS key repeat is ignored.
    pub fn pressed(&self, key: Key) -> bool {
        !self.blocked && self.pressed.contains(&key)
    }

    /// `true` in the frame the key went down **and** whenever the operating system sends a
    /// key repeat while it is held. This is what text fields and menus use.
    pub fn pressed_or_repeated(&self, key: Key) -> bool {
        !self.blocked && (self.pressed.contains(&key) || self.repeated.contains(&key))
    }

    /// `true` as long as the key is held down.
    pub fn held(&self, key: Key) -> bool {
        !self.blocked && self.held.contains_key(&key)
    }

    /// `true` in the frame the key was let go.
    pub fn released(&self, key: Key) -> bool {
        !self.blocked && self.released.contains(&key)
    }

    /// How long the key has been held in seconds, or `None` if it is up.
    pub fn held_for(&self, key: Key) -> Option<f32> {
        if self.blocked { None } else { self.held_now.get(&key).copied() }
    }

    /// Game-controlled key repeat with your own timing, independent of OS settings.
    ///
    /// Returns `true` when the key is pressed, then again after `delay` seconds and from then
    /// on every `every` seconds while it stays held. Perfect for walking on a grid:
    ///
    /// ```no_run
    /// # use audio_code::prelude::*;
    /// # fn f(ctx: &mut Context) {
    /// if ctx.input.repeating(Key::Up, 0.3, 0.3) {
    ///     // take one step forward
    /// }
    /// # }
    /// ```
    pub fn repeating(&self, key: Key, delay: f32, every: f32) -> bool {
        if self.blocked {
            return false;
        }
        if self.pressed.contains(&key) {
            return true;
        }
        let (Some(&now), Some(&prev)) = (self.held_now.get(&key), self.held_prev.get(&key)) else {
            return false;
        };
        if now < delay {
            return false;
        }
        let every = every.max(0.001);
        // Count how many repeat points have been crossed; report if a new one was crossed.
        let ticks = |t: f32| if t < delay { -1.0 } else { ((t - delay) / every).floor() };
        ticks(now) > ticks(prev)
    }

    /// -1.0, 0.0 or 1.0 depending on which of two held keys is down, e.g.
    /// `ctx.input.axis(Key::Left, Key::Right)` for running in a side scroller.
    pub fn axis(&self, negative: Key, positive: Key) -> f32 {
        self.held(positive) as i32 as f32 - self.held(negative) as i32 as f32
    }

    /// Movement direction from the arrow keys or WASD: `x` is right, `y` is forward.
    /// Diagonals are normalized, so moving diagonally is not faster.
    pub fn movement(&self) -> glam::Vec2 {
        let x = (self.held(Key::Right) || self.held(Key::D)) as i32 - (self.held(Key::Left) || self.held(Key::A)) as i32;
        let y = (self.held(Key::Up) || self.held(Key::W)) as i32 - (self.held(Key::Down) || self.held(Key::S)) as i32;
        glam::Vec2::new(x as f32, y as f32).normalize_or_zero()
    }

    /// Like [`Input::movement`], plus `z` for up (Page Up or E) and down (Page Down or Q),
    /// for flying, swimming or climbing. `x` is right, `y` is forward, `z` is up. Diagonals
    /// are normalized.
    pub fn movement_3d(&self) -> glam::Vec3 {
        let flat = self.movement();
        let z = (self.held(Key::PageUp) || self.held(Key::E)) as i32 - (self.held(Key::PageDown) || self.held(Key::Q)) as i32;
        glam::Vec3::new(flat.x, flat.y, z as f32).normalize_or_zero()
    }

    /// `true` if any key went down this frame.
    pub fn any_pressed(&self) -> bool {
        !self.blocked && !self.pressed.is_empty()
    }

    /// Keys that went down this frame in the order they were pressed (including OS repeats).
    pub fn keys_pressed(&self) -> &[Key] {
        if self.blocked { &[] } else { &self.order }
    }

    /// Text typed this frame, respecting keyboard layout and shift state.
    pub fn text(&self) -> &str {
        if self.blocked { "" } else { &self.text }
    }

    /// Current state of shift, control and alt.
    pub fn modifiers(&self) -> Modifiers {
        let held = |k| !self.blocked && self.held.contains_key(&k);
        Modifiers { shift: held(Key::Shift), control: held(Key::Control), alt: held(Key::Alt) }
    }

    /// Unblocked view of the input, even while a UI element is open.
    pub fn raw(&self) -> RawInput<'_> {
        RawInput(self)
    }

    /// Duration the key has been held, using a real `Duration`.
    pub fn held_duration(&self, key: Key) -> Option<Duration> {
        self.held_for(key).map(Duration::from_secs_f32)
    }
}

/// Input queries that ignore UI blocking. Obtain it with [`Input::raw`].
#[derive(Debug, Clone, Copy)]
pub struct RawInput<'a>(&'a Input);

impl RawInput<'_> {
    /// See [`Input::pressed`].
    pub fn pressed(&self, key: Key) -> bool {
        self.0.pressed.contains(&key)
    }

    /// See [`Input::pressed_or_repeated`].
    pub fn pressed_or_repeated(&self, key: Key) -> bool {
        self.0.pressed.contains(&key) || self.0.repeated.contains(&key)
    }

    /// See [`Input::held`].
    pub fn held(&self, key: Key) -> bool {
        self.0.held.contains_key(&key)
    }

    /// See [`Input::released`].
    pub fn released(&self, key: Key) -> bool {
        self.0.released.contains(&key)
    }

    /// See [`Input::text`].
    pub fn text(&self) -> &str {
        &self.0.text
    }

    /// See [`Input::keys_pressed`].
    pub fn keys_pressed(&self) -> &[Key] {
        &self.0.order
    }

    /// See [`Input::modifiers`].
    pub fn modifiers(&self) -> Modifiers {
        let held = |k| self.0.held.contains_key(&k);
        Modifiers { shift: held(Key::Shift), control: held(Key::Control), alt: held(Key::Alt) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_and_release_in_one_frame_is_seen() {
        let mut input = Input::default();
        input.push(KeyEvent::Down { key: Key::Space, repeat: false });
        input.push(KeyEvent::Up { key: Key::Space });
        input.begin_frame(Instant::now());
        assert!(input.pressed(Key::Space));
        assert!(input.released(Key::Space));
        assert!(!input.held(Key::Space));
    }

    #[test]
    fn blocking_hides_keys() {
        let mut input = Input::default();
        input.push(KeyEvent::Down { key: Key::A, repeat: false });
        input.begin_frame(Instant::now());
        input.set_blocked(true);
        assert!(!input.pressed(Key::A));
        assert!(input.raw().pressed(Key::A));
    }

    #[test]
    fn game_repeat() {
        let mut input = Input::default();
        let t = Instant::now();
        input.push(KeyEvent::Down { key: Key::Up, repeat: false });
        input.begin_frame(t);
        assert!(input.repeating(Key::Up, 0.3, 0.1));
        input.begin_frame(t + Duration::from_millis(200));
        assert!(!input.repeating(Key::Up, 0.3, 0.1));
        input.begin_frame(t + Duration::from_millis(310));
        assert!(input.repeating(Key::Up, 0.3, 0.1));
        input.begin_frame(t + Duration::from_millis(350));
        assert!(!input.repeating(Key::Up, 0.3, 0.1));
        input.begin_frame(t + Duration::from_millis(420));
        assert!(input.repeating(Key::Up, 0.3, 0.1));
    }

    #[test]
    fn movement_3d_adds_up_and_down() {
        let mut input = Input::default();
        input.push(KeyEvent::Down { key: Key::PageUp, repeat: false });
        input.begin_frame(Instant::now());
        assert_eq!(input.movement_3d(), glam::Vec3::Z);
        input.push(KeyEvent::Down { key: Key::W, repeat: false });
        input.begin_frame(Instant::now());
        assert!((input.movement_3d() - glam::Vec3::new(0.0, 1.0, 1.0).normalize()).length() < 1e-5);
    }

    #[test]
    fn key_names() {
        assert_eq!(Key::PageUp.name(), "Page Up");
        assert_eq!(Key::Digit5.name(), "5");
        assert_eq!(Key::from_char('Q'), Some(Key::Q));
    }
}
