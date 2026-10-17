//! Run games inside a [wxDragon](https://crates.io/crates/wxdragon) (wxWidgets) window.
//! Requires the `wxdragon` feature.
//!
//! The simplest way is [`Engine::run_wx`](crate::Engine::run_wx), which behaves exactly like
//! [`Engine::run`](crate::Engine::run) but uses a native wxWidgets frame:
//!
//! ```no_run
//! # use audio_code::prelude::*;
//! # struct MyGame;
//! # impl Scene for MyGame { fn update(&mut self, _: &mut Context) {} }
//! fn main() -> audio_code::Result<()> {
//!     Engine::new("My game").run_wx(MyGame)
//! }
//! ```
//!
//! If you already have a wxDragon application (an editor, a launcher with real native
//! controls...), attach a game to any [`Frame`] with [`GameHost::attach`]. Native widgets
//! and the game can talk to each other through [`GameHost::with_context`].

use std::cell::RefCell;
use std::rc::Rc;

use wxdragon::event::WindowEventData;
use wxdragon::prelude::*;

use crate::engine::{Context, Engine, Runner, Scene};
use crate::error::{Error, Result};
use crate::input::Key;

pub(crate) fn run(config: Engine, scene: Box<dyn Scene>) -> Result<()> {
    let title = config.title.clone();
    let (w, h) = config.window_size;
    wxdragon::main(move |_| {
        let frame = Frame::builder().with_title(&title).with_size(Size::new(w as i32, h as i32)).build();
        let host = GameHost::attach_boxed(&frame, config, scene);
        frame.show(true);
        host.focus();
    })
    .map_err(|e| Error::Window(e.to_string()))
}

struct State {
    runner: Runner,
    timer: Option<Timer<Frame>>,
}

/// A game running inside a wxDragon frame.
///
/// The host creates a [`Panel`] in the frame that receives the keyboard, and a timer that
/// runs the game loop. When the game quits the frame is closed.
#[derive(Clone)]
pub struct GameHost {
    state: Rc<RefCell<State>>,
    panel: Panel,
}

impl GameHost {
    /// Starts `scene` inside `frame`.
    pub fn attach(frame: &Frame, config: Engine, scene: impl Scene) -> Self {
        Self::attach_boxed(frame, config, Box::new(scene))
    }

    fn attach_boxed(frame: &Frame, config: Engine, scene: Box<dyn Scene>) -> Self {
        let interval = (1000 / config.fps.max(1)).max(1) as i32;
        let state = Rc::new(RefCell::new(State { runner: Runner::new(config, scene), timer: None }));
        let panel = Panel::builder(frame).build();

        // CharHook sees every key before wxWidgets uses it for navigation, so arrows,
        // Tab and Enter reach the game.
        let s = state.clone();
        frame.on_char_hook(move |event| {
            if let WindowEventData::Keyboard(k) = event {
                let code = k.get_key_code().unwrap_or(0);
                let printable = (32..127).contains(&code) && !k.control_down() && !k.alt_down();
                if let (Some(key), Ok(mut st)) = (map_key(code), s.try_borrow_mut()) {
                    st.runner.key_down(key, false);
                }
                // Let printable keys continue so a Char event with the typed text follows.
                k.event.skip(printable);
            }
        });

        let s = state.clone();
        panel.on_char(move |event| {
            if let WindowEventData::Keyboard(k) = event
                && let Some(c) = k.get_unicode_key().and_then(|c| char::from_u32(c as u32))
                && !c.is_control()
                && let Ok(mut st) = s.try_borrow_mut()
            {
                st.runner.text(&c.to_string());
            }
        });

        let s = state.clone();
        let on_up = move |event: WindowEventData| {
            if let WindowEventData::Keyboard(k) = event
                && let Some(key) = k.get_key_code().and_then(map_key)
                && let Ok(mut st) = s.try_borrow_mut()
            {
                st.runner.key_up(key);
            }
        };
        panel.on_key_up(on_up.clone());
        frame.on_key_up(on_up);

        let s = state.clone();
        frame.on_activate(move |event| {
            event.skip(true);
            if let Ok(mut st) = s.try_borrow_mut() {
                st.runner.focus_lost();
            }
        });

        let s = state.clone();
        frame.on_close(move |event| {
            if let Ok(mut st) = s.try_borrow_mut() {
                if let Some(timer) = st.timer.take() {
                    timer.stop();
                }
                st.runner.shutdown();
            }
            event.skip(true);
        });

        let timer = Timer::new(frame);
        let s = state.clone();
        let frame_handle = *frame;
        timer.on_tick(move |_| {
            let running = match s.try_borrow_mut() {
                Ok(mut st) => st.runner.frame(),
                Err(_) => true,
            };
            if !running {
                frame_handle.close(true);
            }
        });
        timer.start(interval, false);
        // The tick closure holds the state, the state holds the timer: both live as long as
        // the event loop runs.
        state.borrow_mut().timer = Some(timer);

        Self { state, panel }
    }

    /// The panel that receives keyboard input. Add it to your sizers if your frame has
    /// other widgets.
    pub fn panel(&self) -> &Panel {
        &self.panel
    }

    /// Gives the game panel keyboard focus.
    pub fn focus(&self) {
        self.panel.set_focus();
    }

    /// Runs `f` with the game's [`Context`], e.g. from a native button:
    /// `host.with_context(|ctx| ctx.speech.speak("Saved"))`.
    ///
    /// Returns `None` if the game is busy (called from inside a game callback).
    pub fn with_context<R>(&self, f: impl FnOnce(&mut Context) -> R) -> Option<R> {
        self.state.try_borrow_mut().ok().map(|mut st| f(&mut st.runner.ctx))
    }
}

fn map_key(code: i32) -> Option<Key> {
    use wxdragon::keycode::*;
    if let Some(c) = char::from_u32(code as u32)
        && c.is_ascii_alphanumeric()
    {
        return Key::from_char(c);
    }
    Some(match code {
        WXK_F1 => Key::F1,
        WXK_F2 => Key::F2,
        WXK_F3 => Key::F3,
        WXK_F4 => Key::F4,
        WXK_F5 => Key::F5,
        WXK_F6 => Key::F6,
        WXK_F7 => Key::F7,
        WXK_F8 => Key::F8,
        WXK_F9 => Key::F9,
        WXK_F10 => Key::F10,
        WXK_F11 => Key::F11,
        WXK_F12 => Key::F12,
        WXK_UP => Key::Up,
        WXK_DOWN => Key::Down,
        WXK_LEFT => Key::Left,
        WXK_RIGHT => Key::Right,
        WXK_RETURN => Key::Enter,
        WXK_ESCAPE => Key::Escape,
        WXK_SPACE => Key::Space,
        WXK_TAB => Key::Tab,
        WXK_BACK => Key::Backspace,
        WXK_DELETE => Key::Delete,
        WXK_INSERT => Key::Insert,
        WXK_HOME => Key::Home,
        WXK_END => Key::End,
        WXK_PAGEUP => Key::PageUp,
        WXK_PAGEDOWN => Key::PageDown,
        WXK_SHIFT => Key::Shift,
        WXK_CONTROL => Key::Control,
        WXK_ALT => Key::Alt,
        WXK_NUMPAD0 => Key::Numpad0,
        WXK_NUMPAD1 => Key::Numpad1,
        WXK_NUMPAD2 => Key::Numpad2,
        WXK_NUMPAD3 => Key::Numpad3,
        WXK_NUMPAD4 => Key::Numpad4,
        WXK_NUMPAD5 => Key::Numpad5,
        WXK_NUMPAD6 => Key::Numpad6,
        WXK_NUMPAD7 => Key::Numpad7,
        WXK_NUMPAD8 => Key::Numpad8,
        WXK_NUMPAD9 => Key::Numpad9,
        WXK_NUMPAD_ADD | WXK_ADD => Key::NumpadAdd,
        WXK_NUMPAD_SUBTRACT | WXK_SUBTRACT => Key::NumpadSubtract,
        WXK_NUMPAD_MULTIPLY | WXK_MULTIPLY => Key::NumpadMultiply,
        WXK_NUMPAD_DIVIDE | WXK_DIVIDE => Key::NumpadDivide,
        WXK_NUMPAD_DECIMAL | WXK_DECIMAL => Key::NumpadDecimal,
        WXK_NUMPAD_ENTER => Key::NumpadEnter,
        WXK_NUMPAD_UP => Key::Up,
        WXK_NUMPAD_DOWN => Key::Down,
        WXK_NUMPAD_LEFT => Key::Left,
        WXK_NUMPAD_RIGHT => Key::Right,
        WXK_NUMPAD_HOME => Key::Home,
        WXK_NUMPAD_END => Key::End,
        WXK_NUMPAD_PAGEUP => Key::PageUp,
        WXK_NUMPAD_PAGEDOWN => Key::PageDown,
        44 => Key::Comma,
        46 => Key::Period,
        45 => Key::Minus,
        43 | 61 => Key::Plus,
        47 => Key::Slash,
        92 => Key::Backslash,
        59 => Key::Semicolon,
        39 => Key::Quote,
        91 => Key::LeftBracket,
        93 => Key::RightBracket,
        96 => Key::Backquote,
        _ => return None,
    })
}
