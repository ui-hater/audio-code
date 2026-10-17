//! The default window backend built on winit.
//!
//! The window is empty; it exists to receive keyboard focus and to give the screen reader a
//! title to announce.

use std::collections::HashMap;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, KeyLocation, NamedKey, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::engine::{Engine, Runner, Scene};
use crate::error::{Error, Result};
use crate::input::Key;

pub(crate) fn run(config: Engine, scene: Box<dyn Scene>) -> Result<()> {
    let event_loop = EventLoop::new().map_err(|e| Error::Window(e.to_string()))?;
    let mut app = App {
        size: config.window_size,
        title: config.title.clone(),
        runner: Runner::new(config, scene),
        window: None,
        down: HashMap::new(),
        error: None,
    };
    event_loop.run_app(&mut app).map_err(|e| Error::Window(e.to_string()))?;
    match app.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

struct App {
    runner: Runner,
    window: Option<Window>,
    title: String,
    size: (u32, u32),
    /// Which engine key each physical key produced when it went down, so the release maps
    /// to the same key even if the layout or modifiers changed in between.
    down: HashMap<PhysicalKey, Key>,
    error: Option<Error>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(self.title.clone())
            .with_inner_size(LogicalSize::new(self.size.0, self.size.1));
        match event_loop.create_window(attributes) {
            Ok(window) => {
                window.focus_window();
                self.window = Some(window);
            }
            Err(e) => {
                self.error = Some(Error::Window(e.to_string()));
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.runner.shutdown();
                event_loop.exit();
            }
            WindowEvent::Focused(false) => {
                self.down.clear();
                self.runner.focus_lost();
            }
            WindowEvent::KeyboardInput { event, is_synthetic, .. } => match event.state {
                // Synthetic presses are sent on focus gain for keys that were already down
                // (e.g. Alt+Tab); they must not count as fresh key presses.
                ElementState::Pressed if is_synthetic => {}
                ElementState::Pressed => {
                    let key = self.down.get(&event.physical_key).copied().or_else(|| map_key(&event));
                    if let Some(key) = key {
                        self.down.insert(event.physical_key, key);
                        self.runner.key_down(key, event.repeat);
                    }
                    if let Some(text) = &event.text {
                        self.runner.text(text);
                    }
                }
                ElementState::Released => {
                    if let Some(key) = self.down.remove(&event.physical_key).or_else(|| map_key(&event)) {
                        self.runner.key_up(key);
                    }
                }
            },
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if std::time::Instant::now() >= self.runner.next_frame_time() && !self.runner.frame() {
            event_loop.exit();
            return;
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.runner.next_frame_time()));
    }
}

/// Maps a winit key event to an engine key.
///
/// Letters and digits use the logical key so they follow the keyboard layout; everything
/// else uses the physical position.
pub(crate) fn map_key(event: &winit::event::KeyEvent) -> Option<Key> {
    use winit::keyboard::Key as Logical;
    if event.location != KeyLocation::Numpad
        && let Logical::Character(s) = &event.logical_key
        && let Some(key) = s.chars().next().and_then(Key::from_char)
    {
        return Some(key);
    }
    if let Logical::Named(named) = &event.logical_key {
        let key = match named {
            NamedKey::Shift => Some(Key::Shift),
            NamedKey::Control => Some(Key::Control),
            NamedKey::Alt | NamedKey::AltGraph => Some(Key::Alt),
            _ => None,
        };
        if key.is_some() {
            return key;
        }
    }
    let PhysicalKey::Code(code) = event.physical_key else {
        return None;
    };
    map_code(code)
}

fn map_code(code: KeyCode) -> Option<Key> {
    use KeyCode as C;
    Some(match code {
        C::KeyA => Key::A,
        C::KeyB => Key::B,
        C::KeyC => Key::C,
        C::KeyD => Key::D,
        C::KeyE => Key::E,
        C::KeyF => Key::F,
        C::KeyG => Key::G,
        C::KeyH => Key::H,
        C::KeyI => Key::I,
        C::KeyJ => Key::J,
        C::KeyK => Key::K,
        C::KeyL => Key::L,
        C::KeyM => Key::M,
        C::KeyN => Key::N,
        C::KeyO => Key::O,
        C::KeyP => Key::P,
        C::KeyQ => Key::Q,
        C::KeyR => Key::R,
        C::KeyS => Key::S,
        C::KeyT => Key::T,
        C::KeyU => Key::U,
        C::KeyV => Key::V,
        C::KeyW => Key::W,
        C::KeyX => Key::X,
        C::KeyY => Key::Y,
        C::KeyZ => Key::Z,
        C::Digit0 => Key::Digit0,
        C::Digit1 => Key::Digit1,
        C::Digit2 => Key::Digit2,
        C::Digit3 => Key::Digit3,
        C::Digit4 => Key::Digit4,
        C::Digit5 => Key::Digit5,
        C::Digit6 => Key::Digit6,
        C::Digit7 => Key::Digit7,
        C::Digit8 => Key::Digit8,
        C::Digit9 => Key::Digit9,
        C::F1 => Key::F1,
        C::F2 => Key::F2,
        C::F3 => Key::F3,
        C::F4 => Key::F4,
        C::F5 => Key::F5,
        C::F6 => Key::F6,
        C::F7 => Key::F7,
        C::F8 => Key::F8,
        C::F9 => Key::F9,
        C::F10 => Key::F10,
        C::F11 => Key::F11,
        C::F12 => Key::F12,
        C::ArrowUp => Key::Up,
        C::ArrowDown => Key::Down,
        C::ArrowLeft => Key::Left,
        C::ArrowRight => Key::Right,
        C::Enter => Key::Enter,
        C::Escape => Key::Escape,
        C::Space => Key::Space,
        C::Tab => Key::Tab,
        C::Backspace => Key::Backspace,
        C::Delete => Key::Delete,
        C::Insert => Key::Insert,
        C::Home => Key::Home,
        C::End => Key::End,
        C::PageUp => Key::PageUp,
        C::PageDown => Key::PageDown,
        C::ShiftLeft | C::ShiftRight => Key::Shift,
        C::ControlLeft | C::ControlRight => Key::Control,
        C::AltLeft | C::AltRight => Key::Alt,
        C::Numpad0 => Key::Numpad0,
        C::Numpad1 => Key::Numpad1,
        C::Numpad2 => Key::Numpad2,
        C::Numpad3 => Key::Numpad3,
        C::Numpad4 => Key::Numpad4,
        C::Numpad5 => Key::Numpad5,
        C::Numpad6 => Key::Numpad6,
        C::Numpad7 => Key::Numpad7,
        C::Numpad8 => Key::Numpad8,
        C::Numpad9 => Key::Numpad9,
        C::NumpadAdd => Key::NumpadAdd,
        C::NumpadSubtract => Key::NumpadSubtract,
        C::NumpadMultiply => Key::NumpadMultiply,
        C::NumpadDivide => Key::NumpadDivide,
        C::NumpadDecimal => Key::NumpadDecimal,
        C::NumpadEnter => Key::NumpadEnter,
        C::Comma => Key::Comma,
        C::Period => Key::Period,
        C::Minus => Key::Minus,
        C::Equal => Key::Plus,
        C::Slash => Key::Slash,
        C::Backslash => Key::Backslash,
        C::Semicolon => Key::Semicolon,
        C::Quote => Key::Quote,
        C::BracketLeft => Key::LeftBracket,
        C::BracketRight => Key::RightBracket,
        C::Backquote => Key::Backquote,
        _ => return None,
    })
}
