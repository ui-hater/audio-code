//! Speech on macOS without extra libraries: VoiceOver through AppleScript, or the system voice
//! through the `say` command.
//!
//! VoiceOver only accepts AppleScript when "Allow VoiceOver to be controlled with AppleScript"
//! is checked in VoiceOver Utility. Without it, the system voice is used instead.

use std::collections::VecDeque;
use std::path::Path;
use std::process::{Child, Command as Process, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use super::{Command, Shared};

const SAY: &str = "/usr/bin/say";
const OSASCRIPT: &str = "/usr/bin/osascript";
const VOICEOVER: &str = "VoiceOver (AppleScript)";
const SYSTEM_VOICE: &str = "macOS system voice (say)";
/// How often the `say` process is checked so queued messages start on time.
const POLL: Duration = Duration::from_millis(30);
/// How long a "VoiceOver is (not) running" answer is trusted.
const VOICEOVER_CHECK: Duration = Duration::from_secs(2);

/// The system voice, one `say` process per message.
#[derive(Default)]
struct Say {
    current: Option<Child>,
    queue: VecDeque<String>,
    /// Words per minute, `None` for the user's default.
    rate: Option<u32>,
    volume: Option<f32>,
}

impl Say {
    fn speak(&mut self, text: String, interrupt: bool) {
        if interrupt {
            self.stop();
        }
        self.queue.push_back(text);
        self.poll();
    }

    fn stop(&mut self) {
        self.queue.clear();
        if let Some(mut child) = self.current.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Starts the next queued message once the current one has finished.
    fn poll(&mut self) {
        if let Some(child) = &mut self.current {
            if matches!(child.try_wait(), Ok(None)) {
                return;
            }
            self.current = None;
        }
        while let Some(text) = self.queue.pop_front() {
            let mut cmd = Process::new(SAY);
            if let Some(rate) = self.rate {
                cmd.arg("-r").arg(rate.to_string());
            }
            // `say` has no volume option, but understands this embedded command.
            let text = match self.volume {
                Some(volume) => format!("[[volm {volume:.2}]] {text}"),
                None => text,
            };
            cmd.arg("--").arg(text).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
            if let Ok(child) = cmd.spawn() {
                self.current = Some(child);
                return;
            }
        }
    }

    fn busy(&self) -> bool {
        self.current.is_some() || !self.queue.is_empty()
    }
}

/// Maps the engine's 0.0..=1.0 rate to words per minute; 0.5 is a normal speaking rate.
fn words_per_minute(rate: f32) -> u32 {
    let wpm = if rate <= 0.5 { 80.0 + rate * 200.0 } else { 180.0 + (rate - 0.5) * 640.0 };
    wpm.round() as u32
}

/// VoiceOver, controlled through `osascript`.
struct VoiceOver {
    running: bool,
    checked: Option<Instant>,
    /// Cleared when VoiceOver refuses AppleScript, so the system voice takes over.
    scriptable: bool,
}

impl VoiceOver {
    fn new() -> Self {
        Self { running: false, checked: None, scriptable: true }
    }

    fn active(&mut self) -> bool {
        if !self.scriptable {
            return false;
        }
        if self.checked.is_none_or(|t| t.elapsed() > VOICEOVER_CHECK) {
            self.running = Process::new("/usr/bin/pgrep")
                .args(["-x", "VoiceOver"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            self.checked = Some(Instant::now());
        }
        self.running
    }

    /// Speaks (interrupting VoiceOver's current speech). Returns false if VoiceOver refused.
    fn output(&mut self, text: &str) -> bool {
        // The text is passed as an argument, so it needs no AppleScript escaping.
        let ok = Process::new(OSASCRIPT)
            .args([
                "-e",
                "on run argv",
                "-e",
                "tell application id \"com.apple.VoiceOver\" to output (item 1 of argv)",
                "-e",
                "end run",
                "--",
                text,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            self.scriptable = false;
            eprintln!(
                "audio-code: VoiceOver refused AppleScript; using the system voice. To speak through \
                 VoiceOver, check \"Allow VoiceOver to be controlled with AppleScript\" in VoiceOver Utility."
            );
        }
        ok
    }
}

/// Runs the macOS output until the channel closes. Fails if `say` is missing.
pub(super) fn run(rx: &Receiver<Command>, shared: &Shared) -> Result<(), String> {
    if !Path::new(SAY).is_file() {
        return Err(format!("{SAY} not found"));
    }
    let mut voiceover = VoiceOver::new();
    let mut say = Say::default();
    let mut backend = if voiceover.active() { VOICEOVER } else { SYSTEM_VOICE };
    shared.set_backend(backend.into());
    loop {
        let cmd = if say.busy() {
            match rx.recv_timeout(POLL) {
                Ok(cmd) => cmd,
                Err(RecvTimeoutError::Timeout) => {
                    say.poll();
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => break,
            }
        };
        let name = match cmd {
            Command::Speak { text, interrupt } => {
                if voiceover.active() && voiceover.output(&text) {
                    say.stop();
                    VOICEOVER
                } else {
                    say.speak(text, interrupt);
                    SYSTEM_VOICE
                }
            }
            Command::Stop => {
                say.stop();
                continue;
            }
            Command::Rate(rate) => {
                say.rate = Some(words_per_minute(rate));
                continue;
            }
            Command::Volume(volume) => {
                say.volume = Some(volume);
                continue;
            }
            // Neither VoiceOver's AppleScript nor `say` can write to a braille display.
            Command::Braille(_) => continue,
        };
        if name != backend {
            backend = name;
            shared.set_backend(name.into());
        }
    }
    say.stop();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_mapping() {
        assert_eq!(words_per_minute(0.0), 80);
        assert_eq!(words_per_minute(0.5), 180);
        assert_eq!(words_per_minute(1.0), 500);
    }
}
