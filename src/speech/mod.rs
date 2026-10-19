//! Screen reader and text-to-speech output. Available as `ctx.speech`.
//!
//! The engine tries these outputs in order and uses the first one that works:
//!
//! 1. **[prism](https://github.com/ethindp/prism)** (feature `prism`). It talks to the running
//!    screen reader (NVDA, JAWS, VoiceOver, Orca, ...) or falls back to the system voice
//!    (SAPI / OneCore, AVSpeech, speech-dispatcher, ...).
//! 2. **NVDA controller client** (Windows, feature `nvda`). Loads `nvdaControllerClient.dll`
//!    at runtime and speaks through NVDA. See [`Engine::nvda_dll`](crate::Engine::nvda_dll).
//! 3. **VoiceOver or the system voice** (macOS, feature `macos`). Speaks through VoiceOver
//!    announcements when VoiceOver is on, otherwise through `AVSpeechSynthesizer`.
//! 4. **The console.** Everything is printed, so the game always keeps working.
//!
//! Speech runs on its own thread, so speaking never blocks your game loop.

#[cfg(all(target_os = "macos", feature = "macos"))]
mod macos;
#[cfg(all(windows, feature = "nvda"))]
mod nvda;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

const HISTORY_LEN: usize = 100;

// Only prism uses every command (NVDA ignores rate and volume, macOS ignores braille).
#[cfg_attr(not(feature = "prism"), allow(dead_code))]
enum Command {
    Speak { text: String, interrupt: bool },
    Braille(String),
    Stop,
    Rate(f32),
    Volume(f32),
}

/// State shared between [`Speech`] and its output thread.
#[derive(Debug, Default)]
struct Shared {
    backend: Mutex<Option<String>>,
    echo: AtomicBool,
}

impl Shared {
    #[cfg_attr(not(any(feature = "prism", all(windows, feature = "nvda"), all(target_os = "macos", feature = "macos"))), allow(dead_code))]
    fn set_backend(&self, name: String) {
        *self.backend.lock().unwrap_or_else(|e| e.into_inner()) = Some(name);
    }
}

/// Speech and braille output. Reach it through `ctx.speech`.
#[derive(Debug)]
pub struct Speech {
    tx: Option<Sender<Command>>,
    shared: Arc<Shared>,
    muted: bool,
    history: Vec<String>,
}

impl Speech {
    pub(crate) fn new(echo: bool, nvda_dll: Option<PathBuf>) -> Self {
        let shared = Arc::new(Shared::default());
        shared.echo.store(echo, Ordering::Relaxed);
        let tx = spawn(shared.clone(), nvda_dll);
        Self { tx, shared, muted: false, history: Vec::new() }
    }

    /// Speech that only prints to the console. Useful for tests and headless tools.
    pub fn console() -> Self {
        let shared = Arc::new(Shared::default());
        shared.echo.store(true, Ordering::Relaxed);
        Self { tx: None, shared, muted: false, history: Vec::new() }
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

    /// Also print everything spoken to the console. Handy while developing.
    pub fn set_echo(&mut self, echo: bool) {
        self.shared.echo.store(echo, Ordering::Relaxed);
    }

    /// Mutes speech output (messages still go to the history).
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Name of the active output, e.g. "NVDA", "SAPI", "NVDA (controller client)",
    /// "VoiceOver" or "AVSpeechSynthesizer".
    /// `None` while starting up or when printing to the console.
    pub fn backend_name(&self) -> Option<String> {
        self.shared.backend.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// Starts the output thread, or returns `None` if no output is compiled in.
#[cfg_attr(not(all(windows, feature = "nvda")), allow(unused_variables))]
fn spawn(shared: Arc<Shared>, nvda_dll: Option<PathBuf>) -> Option<Sender<Command>> {
    if !cfg!(any(feature = "prism", all(windows, feature = "nvda"), all(target_os = "macos", feature = "macos"))) {
        return None;
    }
    let (tx, rx) = channel::<Command>();
    let spawned = std::thread::Builder::new().name("audio-code speech".into()).spawn(move || {
        #[cfg_attr(not(any(feature = "prism", all(windows, feature = "nvda"), all(target_os = "macos", feature = "macos"))), allow(unused_mut))]
        let mut reasons: Vec<String> = Vec::new();

        #[cfg(feature = "prism")]
        match run_prism(&rx, &shared) {
            Ok(()) => return,
            Err(e) => reasons.push(format!("prism: {e}")),
        }

        #[cfg(all(windows, feature = "nvda"))]
        match nvda::run(&rx, &shared, nvda_dll.as_deref()) {
            Ok(()) => return,
            Err(e) => reasons.push(format!("NVDA controller client: {e}")),
        }

        #[cfg(all(target_os = "macos", feature = "macos"))]
        match macos::run(&rx, &shared) {
            Ok(()) => return,
            Err(e) => reasons.push(format!("macOS speech: {e}")),
        }

        eprintln!("audio-code: no speech output found ({}); printing to console", reasons.join("; "));
        // Dropping the receiver makes Speech switch to console output.
        drop(rx);
    });
    spawned.ok().map(|_| tx)
}

/// Runs prism until the channel closes. Returns an error if prism has no usable output.
#[cfg(feature = "prism")]
fn run_prism(rx: &Receiver<Command>, shared: &Shared) -> Result<(), String> {
    let prism = prismer::Prism::new().map_err(|e| format!("{e:?}"))?;
    let backend = prism.create_best().map_err(|e| format!("{e:?}"))?;
    shared.set_backend(backend.name());
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
        };
    }
    Ok(())
}

#[cfg(not(feature = "prism"))]
#[allow(dead_code)]
fn run_prism(_: &Receiver<Command>, _: &Shared) -> Result<(), String> {
    Err("not compiled in".into())
}
