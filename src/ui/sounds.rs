//! Interface sounds for the virtual UI.

use crate::audio::{Audio, Synth};

/// The moments at which widgets play a sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiSound {
    /// A widget opened.
    Open,
    /// A widget was closed by the player.
    Close,
    /// The focus moved to another item.
    Navigate,
    /// The focus jumped from the last to the first item or back (menus with `wrap`).
    /// Falls back to [`UiSound::Navigate`] when not set.
    Wrap,
    /// A button was pressed.
    Activate,
    /// The focus hit the first or last item.
    Edge,
    /// A value changed (slider, checkbox, choice).
    Change,
    /// A character was typed or deleted in a text field (silent by default).
    Typing,
}

impl UiSound {
    /// Every kind of interface sound.
    pub const ALL: [UiSound; 8] = [
        UiSound::Open,
        UiSound::Close,
        UiSound::Navigate,
        UiSound::Wrap,
        UiSound::Activate,
        UiSound::Edge,
        UiSound::Change,
        UiSound::Typing,
    ];

    /// Lower case name, also used as file name by [`UiSounds::from_folder`].
    pub fn name(self) -> &'static str {
        match self {
            UiSound::Open => "open",
            UiSound::Close => "close",
            UiSound::Navigate => "navigate",
            UiSound::Wrap => "wrap",
            UiSound::Activate => "activate",
            UiSound::Edge => "edge",
            UiSound::Change => "change",
            UiSound::Typing => "typing",
        }
    }
}

/// Which sound to play for each [`UiSound`]. Each entry is a sound name as accepted by
/// [`Audio::play`]; `None` means silence.
///
/// The default is [`UiSounds::builtin`]: small generated clicks that work without any files.
///
/// ```
/// use audio_code::ui::{UiSound, UiSounds};
///
/// // Built-in sounds, but your own file for moving through menus and no "edge" sound.
/// let sounds = UiSounds::builtin()
///     .with(UiSound::Navigate, "ui/my_click")
///     .without(UiSound::Edge);
/// ```
///
/// To design a click in code instead of using a file, see
/// [`Ui::set_sound_data`](super::Ui::set_sound_data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiSounds {
    /// Played when a widget opens.
    pub open: Option<String>,
    /// Played when the player closes a widget.
    pub close: Option<String>,
    /// Played when moving between items.
    pub navigate: Option<String>,
    /// Played when wrapping around in a menu (falls back to `navigate`).
    pub wrap: Option<String>,
    /// Played when a button is pressed.
    pub activate: Option<String>,
    /// Played at the first or last item.
    pub edge: Option<String>,
    /// Played when a value changes.
    pub change: Option<String>,
    /// Played for each typed or deleted character.
    pub typing: Option<String>,
}

impl Default for UiSounds {
    fn default() -> Self {
        Self::builtin()
    }
}

impl UiSounds {
    /// Generated sounds that need no files. Typing is silent.
    pub fn builtin() -> Self {
        let mut sounds = Self::none();
        for kind in UiSound::ALL {
            if kind != UiSound::Typing {
                sounds.set(kind, Some(format!("ui:builtin:{}", kind.name())));
            }
        }
        sounds
    }

    /// No sounds at all.
    pub fn none() -> Self {
        Self {
            open: None,
            close: None,
            navigate: None,
            wrap: None,
            activate: None,
            edge: None,
            change: None,
            typing: None,
        }
    }

    /// Loads sounds by convention from a folder: `open`, `close`, `navigate`, `wrap`,
    /// `activate`, `edge`, `change` and `typing` (any supported extension) inside `folder`.
    /// Missing files fall back to silence with a warning on first use; use
    /// [`UiSounds::without`] for slots you do not have a file for.
    pub fn from_folder(folder: &str) -> Self {
        let mut sounds = Self::none();
        for kind in UiSound::ALL {
            sounds.set(kind, Some(format!("{}/{}", folder.trim_end_matches('/'), kind.name())));
        }
        sounds
    }

    /// Returns a copy that plays sound `name` for `kind`.
    pub fn with(mut self, kind: UiSound, name: impl Into<String>) -> Self {
        self.set(kind, Some(name.into()));
        self
    }

    /// Returns a copy that is silent for `kind`.
    pub fn without(mut self, kind: UiSound) -> Self {
        self.set(kind, None);
        self
    }

    /// Changes the sound for `kind` (`None` = silent).
    pub fn set(&mut self, kind: UiSound, name: Option<String>) {
        *self.slot(kind) = name;
    }

    fn slot(&mut self, kind: UiSound) -> &mut Option<String> {
        match kind {
            UiSound::Open => &mut self.open,
            UiSound::Close => &mut self.close,
            UiSound::Navigate => &mut self.navigate,
            UiSound::Wrap => &mut self.wrap,
            UiSound::Activate => &mut self.activate,
            UiSound::Edge => &mut self.edge,
            UiSound::Change => &mut self.change,
            UiSound::Typing => &mut self.typing,
        }
    }

    /// The sound name for a moment, if any.
    pub fn get(&self, kind: UiSound) -> Option<&str> {
        match kind {
            UiSound::Open => self.open.as_deref(),
            UiSound::Close => self.close.as_deref(),
            UiSound::Navigate => self.navigate.as_deref(),
            UiSound::Wrap => self.wrap.as_deref().or(self.navigate.as_deref()),
            UiSound::Activate => self.activate.as_deref(),
            UiSound::Edge => self.edge.as_deref(),
            UiSound::Change => self.change.as_deref(),
            UiSound::Typing => self.typing.as_deref(),
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
    let synth = match kind {
        "navigate" => Synth::sine(1400.0).length(0.035).decay(120.0).volume(0.35),
        "wrap" => Synth::sine(1400.0).length(0.03).decay(120.0).volume(0.3).then(Synth::sine(1900.0).length(0.04).decay(120.0).volume(0.3)),
        "change" => Synth::sine(1000.0).length(0.04).decay(90.0).volume(0.35),
        "activate" => Synth::sine(700.0).length(0.05).decay(40.0).volume(0.35).then(Synth::sine(1050.0).length(0.07).decay(40.0).volume(0.35)),
        "edge" => Synth::sine(180.0).length(0.08).decay(45.0).volume(0.5),
        "open" => Synth::sine(500.0).sweep_to(980.0).length(0.12).decay(15.0).volume(0.25),
        "close" => Synth::sine(950.0).sweep_to(590.0).length(0.12).decay(15.0).volume(0.25),
        "typing" => Synth::noise().length(0.015).decay(250.0).volume(0.2),
        _ => return,
    };
    audio.register(name, synth);
}
