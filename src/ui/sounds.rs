//! Interface sounds for the virtual UI.

use crate::audio::{Audio, SoundData};

/// The moments at which widgets play a sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiSound {
    /// A widget opened.
    Open,
    /// A widget was closed by the player.
    Close,
    /// The focus moved to another item.
    Navigate,
    /// A button was pressed.
    Activate,
    /// The focus hit the first or last item.
    Edge,
    /// A value changed (slider, checkbox, choice, typing).
    Change,
}

/// Which sound to play for each [`UiSound`]. Each entry is a sound name as accepted by
/// [`Audio::play`]; `None` means silence.
///
/// The default is [`UiSounds::builtin`]: small generated clicks that work without any files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiSounds {
    /// Played when a widget opens.
    pub open: Option<String>,
    /// Played when the player closes a widget.
    pub close: Option<String>,
    /// Played when moving between items.
    pub navigate: Option<String>,
    /// Played when a button is pressed.
    pub activate: Option<String>,
    /// Played at the first or last item.
    pub edge: Option<String>,
    /// Played when a value changes.
    pub change: Option<String>,
}

impl Default for UiSounds {
    fn default() -> Self {
        Self::builtin()
    }
}

impl UiSounds {
    /// Generated sounds that need no files.
    pub fn builtin() -> Self {
        let s = |n: &str| Some(format!("ui:builtin:{n}"));
        Self {
            open: s("open"),
            close: s("close"),
            navigate: s("navigate"),
            activate: s("activate"),
            edge: s("edge"),
            change: s("change"),
        }
    }

    /// No sounds at all.
    pub fn none() -> Self {
        Self { open: None, close: None, navigate: None, activate: None, edge: None, change: None }
    }

    /// Loads sounds by convention from a folder: `open`, `close`, `navigate`, `activate`,
    /// `edge` and `change` (any supported extension) inside `folder`.
    /// Missing files fall back to silence with a warning on first use.
    pub fn from_folder(folder: &str) -> Self {
        let s = |n: &str| Some(format!("{}/{n}", folder.trim_end_matches('/')));
        Self {
            open: s("open"),
            close: s("close"),
            navigate: s("navigate"),
            activate: s("activate"),
            edge: s("edge"),
            change: s("change"),
        }
    }

    /// The sound name for a moment, if any.
    pub fn get(&self, sound: UiSound) -> Option<&str> {
        match sound {
            UiSound::Open => self.open.as_deref(),
            UiSound::Close => self.close.as_deref(),
            UiSound::Navigate => self.navigate.as_deref(),
            UiSound::Activate => self.activate.as_deref(),
            UiSound::Edge => self.edge.as_deref(),
            UiSound::Change => self.change.as_deref(),
        }
    }
}

/// Generates a built-in sound the first time it is needed.
pub(crate) fn ensure_builtin(audio: &mut Audio, name: &str) {
    let Some(kind) = name.strip_prefix("ui:builtin:") else {
        return;
    };
    if audio.is_loaded(name) {
        return;
    }
    let sine = |f: f32, t: f32| (t * f * std::f32::consts::TAU).sin();
    let data = match kind {
        "navigate" => SoundData::generate(0.035, |t| sine(1400.0, t) * (-t * 120.0).exp() * 0.35),
        "change" => SoundData::generate(0.04, |t| sine(1000.0, t) * (-t * 90.0).exp() * 0.35),
        "activate" => SoundData::generate(0.12, |t| {
            let f = if t < 0.05 { 700.0 } else { 1050.0 };
            sine(f, t) * (-(t % 0.05) * 40.0).exp() * 0.35
        }),
        "edge" => SoundData::generate(0.08, |t| sine(180.0, t) * (-t * 45.0).exp() * 0.5),
        "open" => SoundData::generate(0.12, |t| sine(500.0 + t * 4000.0, t) * (1.0 - t / 0.12) * 0.25),
        "close" => SoundData::generate(0.12, |t| sine(950.0 - t * 3000.0, t) * (1.0 - t / 0.12) * 0.25),
        _ => return,
    };
    audio.register(name, data);
}
