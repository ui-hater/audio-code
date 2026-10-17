//! Screen reader and text-to-speech output through [prism](https://github.com/ethindp/prism).
//! Available as `ctx.speech`.
//!
//! Prism automatically talks to the running screen reader (NVDA, JAWS, VoiceOver, Orca, ...)
//! and falls back to the system voice (SAPI / OneCore, AVSpeech, speech-dispatcher, ...).
//! Speech runs on its own thread, so speaking never blocks your game loop.
//!
//! When the crate is compiled without the `prism` feature, or prism cannot find any output,
//! everything is printed to the console instead.

use std::sync::mpsc::Sender;
#[cfg(feature = "prism")]
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

const HISTORY_LEN: usize = 100;

#[cfg_attr(not(feature = "prism"), allow(dead_code))]
enum Command {
    Speak { text: String, interrupt: bool },
    Braille(String),
    Stop,
    Rate(f32),
    Volume(f32),
}

/// Speech and braille output. Reach it through `ctx.speech`.
#[derive(Debug)]
pub struct Speech {
    tx: Option<Sender<Command>>,
    backend: Arc<Mutex<Option<String>>>,
    echo: bool,
    muted: bool,
    history: Vec<String>,
}

impl Speech {
    pub(crate) fn new(echo: bool) -> Self {
        let backend = Arc::new(Mutex::new(None));
        let tx = Self::spawn(backend.clone());
        Self { tx, backend, echo, muted: false, history: Vec::new() }
    }

    /// Speech that only prints to the console. Useful for tests and headless tools.
    pub fn console() -> Self {
        Self { tx: None, backend: Arc::default(), echo: true, muted: false, history: Vec::new() }
    }

    #[cfg(feature = "prism")]
    fn spawn(name: Arc<Mutex<Option<String>>>) -> Option<Sender<Command>> {
        let (tx, rx) = channel::<Command>();
        let spawned = std::thread::Builder::new().name("audio-code speech".into()).spawn(move || {
            let prism = match prismer::Prism::new() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("audio-code: speech unavailable ({e:?}); printing to console");
                    return;
                }
            };
            let backend = match prism.create_best() {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("audio-code: no speech backend found ({e:?}); printing to console");
                    return;
                }
            };
            *name.lock().unwrap_or_else(|e| e.into_inner()) = Some(backend.name());
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
        });
        spawned.ok().map(|_| tx)
    }

    #[cfg(not(feature = "prism"))]
    fn spawn(_: Arc<Mutex<Option<String>>>) -> Option<Sender<Command>> {
        None
    }

    fn send(&mut self, cmd: Command) -> bool {
        match &self.tx {
            Some(tx) => {
                if tx.send(cmd).is_err() {
                    // The speech thread gave up (no backend); fall back to the console.
                    self.tx = None;
                    self.echo = true;
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
        if self.echo || !sent {
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

    /// Sets the speech rate if the backend supports it (0.0 slowest to 1.0 fastest).
    /// Screen readers usually ignore this and keep the user's own settings.
    pub fn set_rate(&mut self, rate: f32) {
        self.send(Command::Rate(rate.clamp(0.0, 1.0)));
    }

    /// Sets the speech volume if the backend supports it (0.0 to 1.0).
    pub fn set_volume(&mut self, volume: f32) {
        self.send(Command::Volume(volume.clamp(0.0, 1.0)));
    }

    /// Also print everything spoken to the console. Handy while developing.
    pub fn set_echo(&mut self, echo: bool) {
        self.echo = echo;
    }

    /// Mutes speech output (messages still go to the history).
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Name of the active speech backend, e.g. "NVDA" or "SAPI". `None` while starting up
    /// or when printing to the console.
    pub fn backend_name(&self) -> Option<String> {
        self.backend.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}
