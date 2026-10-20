//! Screen reader and text-to-speech output. Available as `ctx.speech`.
//!
//! The engine tries these outputs in order and uses the first one that works:
//!
//! 1. **[prism](https://github.com/ethindp/prism)** (feature `prism`, on by default). It talks
//!    to the running screen reader (NVDA, JAWS, VoiceOver, Orca, ...) or falls back to the
//!    system voice (SAPI / OneCore, AVSpeech, speech-dispatcher, ...).
//! 2. **VoiceOver or the system voice** (macOS, feature `macos`). Speaks through VoiceOver
//!    announcements when VoiceOver is on, otherwise through `AVSpeechSynthesizer`.
//! 3. **The console.** Everything is printed, so the game always keeps working.
//!
//! Speech runs on its own thread, so speaking never blocks your game loop.
//!
//! [`Speech::voices`] lists the voices of the system voice output and [`Speech::set_voice`]
//! picks one. Screen readers keep the voice the player chose in the screen reader.

#[cfg(all(target_os = "macos", feature = "macos"))]
mod macos;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

const HISTORY_LEN: usize = 100;
/// How long [`Speech::voices`] waits for the output to start.
const VOICES_TIMEOUT: Duration = Duration::from_secs(3);

// Only prism uses every command (macOS ignores braille).
#[cfg_attr(not(feature = "prism"), allow(dead_code))]
enum Command {
    Speak { text: String, interrupt: bool },
    Braille(String),
    Stop,
    Rate(f32),
    Volume(f32),
    /// A [`Voice::id`] from the list the output published, or `None` for the default voice.
    Voice(Option<String>),
}

/// A voice that [`Speech::set_voice`] can switch to. Get the list with [`Speech::voices`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voice {
    /// Identifies the voice for [`Speech::set_voice`]. Its format depends on the output, for
    /// example `"com.apple.voice.compact.de-DE.Anna"` on macOS.
    pub id: String,
    /// Display name, e.g. "Anna". Show this to players.
    pub name: String,
    /// Language tag such as "de-DE" or "en-US", if the output reports one.
    pub language: Option<String>,
}

impl std::fmt::Display for Voice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.language {
            Some(language) => write!(f, "{} ({language})", self.name),
            None => f.write_str(&self.name),
        }
    }
}

/// State shared between [`Speech`] and its output thread.
#[derive(Debug, Default)]
struct Shared {
    backend: Mutex<Option<String>>,
    echo: AtomicBool,
    /// `None` until the output has started and listed its voices.
    voices: Mutex<Option<Vec<Voice>>>,
    voices_ready: Condvar,
}

impl Shared {
    #[cfg_attr(not(any(feature = "prism", all(target_os = "macos", feature = "macos"))), allow(dead_code))]
    fn set_backend(&self, name: String) {
        *self.backend.lock().unwrap_or_else(|e| e.into_inner()) = Some(name);
    }

    /// Publishes the voice list (empty if the output has no voices to choose from).
    fn set_voices(&self, voices: Vec<Voice>) {
        *self.voices.lock().unwrap_or_else(|e| e.into_inner()) = Some(voices);
        self.voices_ready.notify_all();
    }
}

/// Speech and braille output. Reach it through `ctx.speech`.
#[derive(Debug)]
pub struct Speech {
    tx: Option<Sender<Command>>,
    shared: Arc<Shared>,
    muted: bool,
    history: Vec<String>,
    voice: Option<Voice>,
}

impl Speech {
    pub(crate) fn new(echo: bool) -> Self {
        let shared = Arc::new(Shared::default());
        shared.echo.store(echo, Ordering::Relaxed);
        let tx = spawn(shared.clone());
        if tx.is_none() {
            shared.set_voices(Vec::new());
        }
        Self { tx, shared, muted: false, history: Vec::new(), voice: None }
    }

    /// Speech that only prints to the console. Useful for tests and headless tools.
    pub fn console() -> Self {
        let shared = Arc::new(Shared::default());
        shared.echo.store(true, Ordering::Relaxed);
        shared.set_voices(Vec::new());
        Self { tx: None, shared, muted: false, history: Vec::new(), voice: None }
    }

    fn echo(&self) -> bool {
        self.shared.echo.load(Ordering::Relaxed)
    }

    fn send(&mut self, cmd: Command) -> bool {
        match &self.tx {
            Some(tx) => {
                if tx.send(cmd).is_err() {
                    // The speech thread gave up (no output found); fall back to the console.
                    self.tx = None;
                    self.shared.echo.store(true, Ordering::Relaxed);
                    false
                } else {
                    true
                }
            }
            None => false,
        }
    }

    fn say(&mut self, text: &str, interrupt: bool) {
        if text.is_empty() {
            return;
        }
        if self.history.len() >= HISTORY_LEN {
            self.history.remove(0);
        }
        self.history.push(text.to_string());
        if self.muted {
            return;
        }
        let sent = self.send(Command::Speak { text: text.to_string(), interrupt });
        if self.echo() || !sent {
            println!("{text}");
        }
    }

    /// Speaks text, interrupting whatever is being said. This is what you want most of the time.
    pub fn speak(&mut self, text: impl AsRef<str>) {
        self.say(text.as_ref(), true);
    }

    /// Speaks text after the current speech has finished.
    pub fn speak_queued(&mut self, text: impl AsRef<str>) {
        self.say(text.as_ref(), false);
    }

    /// Shows text on a braille display only.
    pub fn braille(&mut self, text: impl AsRef<str>) {
        self.send(Command::Braille(text.as_ref().to_string()));
    }

    /// Silences speech immediately.
    pub fn stop(&mut self) {
        self.send(Command::Stop);
    }

    /// Speaks the last message again. Bind this to a key so players can catch up.
    pub fn repeat_last(&mut self) {
        if let Some(last) = self.history.last().cloned() {
            self.history.pop();
            self.speak(last);
        }
    }

    /// The most recently spoken message.
    pub fn last(&self) -> Option<&str> {
        self.history.last().map(String::as_str)
    }

    /// Recently spoken messages, oldest first (up to 100).
    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Sets the speech rate if the output supports it, from 0.0 (slowest) to 1.0
    /// (fastest), passed on to the voice as is.
    /// Screen readers usually ignore this and keep the user's own settings.
    pub fn set_rate(&mut self, rate: f32) {
        self.send(Command::Rate(rate.clamp(0.0, 1.0)));
    }

    /// Sets the speech volume if the output supports it (0.0 to 1.0).
    pub fn set_volume(&mut self, volume: f32) {
        self.send(Command::Volume(volume.clamp(0.0, 1.0)));
    }

    /// The voices of the system voice output, sorted by language and name, for example to
    /// build a voice menu. Empty when the output has no choice of voices: screen readers
    /// (NVDA, VoiceOver, ...) speak with the voice the player set up there, and the console has
    /// none.
    ///
    /// Right after the game starts this waits until the output is ready (at most a few
    /// seconds); after that it returns immediately.
    ///
    /// ```no_run
    /// # use audio_code::prelude::*;
    /// # fn f(ctx: &mut Context) {
    /// for voice in ctx.speech.voices() {
    ///     println!("{} {}", voice.name, voice.language.as_deref().unwrap_or("?"));
    /// }
    /// # }
    /// ```
    pub fn voices(&self) -> Vec<Voice> {
        let voices = self.shared.voices.lock().unwrap_or_else(|e| e.into_inner());
        let (voices, _) = self
            .shared
            .voices_ready
            .wait_timeout_while(voices, VOICES_TIMEOUT, |v| v.is_none())
            .unwrap_or_else(|e| e.into_inner());
        voices.clone().unwrap_or_default()
    }

    /// Switches the system voice. `voice` is a [`Voice::id`] or a voice name (any case),
    /// e.g. `"Anna"`. Returns `false` and keeps the current voice if no such voice exists.
    /// Screen readers ignore this, see [`Speech::voices`].
    pub fn set_voice(&mut self, voice: impl AsRef<str>) -> bool {
        let wanted = voice.as_ref();
        let voices = self.voices();
        let found = voices
            .iter()
            .find(|v| v.id == wanted)
            .or_else(|| voices.iter().find(|v| v.name.eq_ignore_ascii_case(wanted)))
            .cloned();
        match found {
            Some(voice) => {
                self.send(Command::Voice(Some(voice.id.clone())));
                self.voice = Some(voice);
                true
            }
            None => {
                eprintln!("audio-code: no speech voice called \"{wanted}\"");
                false
            }
        }
    }

    /// Goes back to the default voice, the one the game started with.
    pub fn reset_voice(&mut self) {
        if self.voice.take().is_some() {
            self.send(Command::Voice(None));
        }
    }

    /// The voice chosen with [`Speech::set_voice`], or `None` for the default voice.
    pub fn voice(&self) -> Option<&Voice> {
        self.voice.as_ref()
    }

    /// Also print everything spoken to the console. Handy while developing.
    pub fn set_echo(&mut self, echo: bool) {
        self.shared.echo.store(echo, Ordering::Relaxed);
    }

    /// Mutes speech output (messages still go to the history).
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Name of the active output, e.g. "NVDA", "SAPI", "VoiceOver" or "AVSpeechSynthesizer".
    /// `None` while starting up or when printing to the console.
    pub fn backend_name(&self) -> Option<String> {
        self.shared.backend.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// Starts the output thread, or returns `None` if no output is compiled in.
fn spawn(shared: Arc<Shared>) -> Option<Sender<Command>> {
    if !cfg!(any(feature = "prism", all(target_os = "macos", feature = "macos"))) {
        return None;
    }
    let (tx, rx) = channel::<Command>();
    let spawned = std::thread::Builder::new().name("audio-code speech".into()).spawn(move || {
        #[cfg_attr(not(any(feature = "prism", all(target_os = "macos", feature = "macos"))), allow(unused_mut))]
        let mut reasons: Vec<String> = Vec::new();

        #[cfg(feature = "prism")]
        match run_prism(&rx, &shared) {
            Ok(()) => return,
            Err(e) => reasons.push(format!("prism: {e}")),
        }

        #[cfg(all(target_os = "macos", feature = "macos"))]
        match macos::run(&rx, &shared) {
            Ok(()) => return,
            Err(e) => reasons.push(format!("macOS speech: {e}")),
        }

        eprintln!("audio-code: no speech output found ({}); printing to console", reasons.join("; "));
        shared.set_voices(Vec::new());
        // Dropping the receiver makes Speech switch to console output.
        drop(rx);
    });
    spawned.ok().map(|_| tx)
}

/// Sorts voices by language, then name, so a voice menu groups languages together.
#[cfg_attr(not(any(feature = "prism", all(target_os = "macos", feature = "macos"))), allow(dead_code))]
fn sort_voices(voices: &mut [Voice]) {
    voices.sort_by(|a, b| a.language.cmp(&b.language).then_with(|| a.name.cmp(&b.name)));
}

/// Runs prism until the channel closes. Returns an error if prism has no usable output.
#[cfg(feature = "prism")]
fn run_prism(rx: &Receiver<Command>, shared: &Shared) -> Result<(), String> {
    // Declared before the backend, so the backend is dropped first.
    let prism = prism::Context::new().map_err(|e| e.to_string())?;
    let mut backend = prism.acquire_best().map_err(|e| e.to_string())?;
    shared.set_backend(backend.name().unwrap_or_else(|_| "prism".into()));
    let count = backend.voice_count().unwrap_or(0);
    let mut voices: Vec<Voice> = (0..count)
        .filter_map(|i| {
            let name = backend.voice_name(i).ok()?;
            let language = backend.voice_language(i).ok().filter(|l| !l.is_empty());
            Some(Voice { id: i.to_string(), name, language })
        })
        .collect();
    sort_voices(&mut voices);
    shared.set_voices(voices);
    let default_voice = backend.voice().ok();
    for cmd in rx {
        // Errors such as "not implemented" for one feature are not worth crashing for.
        let _ = match cmd {
            Command::Speak { text, interrupt } => {
                backend.output(&text, interrupt).or_else(|_| backend.speak(&text, interrupt))
            }
            Command::Braille(text) => backend.braille(&text),
            Command::Stop => backend.stop(),
            Command::Rate(r) => backend.set_rate(r),
            Command::Volume(v) => backend.set_volume(v),
            Command::Voice(id) => match id.map_or(default_voice, |id| id.parse().ok()) {
                Some(index) => backend.set_voice(index),
                None => Ok(()),
            },
        };
    }
    Ok(())
}

#[cfg(not(feature = "prism"))]
#[allow(dead_code)]
fn run_prism(_: &Receiver<Command>, _: &Shared) -> Result<(), String> {
    Err("not compiled in".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_has_no_voices() {
        let mut speech = Speech::console();
        assert!(speech.voices().is_empty());
        assert!(!speech.set_voice("Anna"));
        assert_eq!(speech.voice(), None);
        speech.reset_voice();
    }

    #[test]
    fn voices_sort_by_language_then_name() {
        let voice = |name: &str, language: &str| Voice { id: name.into(), name: name.into(), language: Some(language.into()) };
        let mut voices = vec![voice("Zoe", "de-DE"), voice("Alex", "en-US"), voice("Anna", "de-DE")];
        sort_voices(&mut voices);
        let names: Vec<&str> = voices.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["Anna", "Zoe", "Alex"]);
        assert_eq!(voices[0].to_string(), "Anna (de-DE)");
    }
}
