//! Speech on macOS through Apple's own APIs:
//!
//! - **VoiceOver**: when it is on, messages are posted as accessibility announcements
//!   (`NSAccessibilityAnnouncementRequestedNotification`), so players hear their own VoiceOver
//!   voice and settings, and the text also reaches a braille display.
//! - **System voice**: otherwise `AVSpeechSynthesizer` speaks, using the voice, rate and volume
//!   from the system's Spoken Content settings unless the game sets its own.
//!
//! AppKit must be used from the main thread, so the speech thread only forwards commands to
//! the main dispatch queue. The window's event loop (winit or wxWidgets) runs that queue
//! between frames; all state lives in a thread local there.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, NSObjectProtocol};
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSAccessibilityAnnouncementKey, NSAccessibilityAnnouncementRequestedNotification,
    NSAccessibilityPostNotificationWithUserInfo, NSAccessibilityPriorityKey, NSAccessibilityPriorityLevel,
    NSApplication, NSWorkspace,
};
use objc2_avf_audio::{AVSpeechBoundary, AVSpeechSynthesizer, AVSpeechUtterance};
use objc2_foundation::{NSDictionary, NSNumber, NSString};

use super::{Command, Shared};

const VOICEOVER: &str = "VoiceOver";
const SYSTEM_VOICE: &str = "AVSpeechSynthesizer";

thread_local! {
    /// The output, created on first use. Only ever touched on the main thread.
    static OUTPUT: RefCell<Option<Output>> = const { RefCell::new(None) };
}

struct Output {
    synth: Retained<AVSpeechSynthesizer>,
    /// Set by the game; `None` keeps the user's Spoken Content settings.
    rate: Option<f32>,
    volume: Option<f32>,
    backend: &'static str,
}

impl Output {
    fn new() -> Self {
        // SAFETY: plain allocation and init of an AVFoundation object.
        let synth = unsafe { AVSpeechSynthesizer::new() };
        Self { synth, rate: None, volume: None, backend: "" }
    }

    fn handle(&mut self, mtm: MainThreadMarker, cmd: Command, shared: &Shared) {
        match cmd {
            Command::Speak { text, interrupt } => {
                if voiceover_running() {
                    // Do not talk over VoiceOver if it was just turned on.
                    self.stop();
                    announce(mtm, &text, interrupt);
                    self.set_backend(VOICEOVER, shared);
                } else {
                    self.speak(&text, interrupt);
                    self.set_backend(SYSTEM_VOICE, shared);
                }
            }
            Command::Stop => self.stop(),
            // VoiceOver keeps the user's own rate and volume, like NVDA.
            Command::Rate(rate) => self.rate = Some(rate),
            Command::Volume(volume) => self.volume = Some(volume),
            // Announcements are always spoken as well, so there is no braille-only output.
            Command::Braille(_) => {}
        }
    }

    fn set_backend(&mut self, name: &'static str, shared: &Shared) {
        if self.backend != name {
            self.backend = name;
            shared.set_backend(name.into());
        }
    }

    fn speak(&self, text: &str, interrupt: bool) {
        // SAFETY: the synthesizer and utterance are valid objects used on the main thread, and
        // the optional selector is only called after checking that it exists.
        unsafe {
            if interrupt {
                self.synth.stopSpeakingAtBoundary(AVSpeechBoundary::Immediate);
            }
            let utterance = AVSpeechUtterance::speechUtteranceWithString(&NSString::from_str(text));
            match self.rate {
                // AVSpeech rates go from 0.0 to 1.0 with 0.5 as the default, just like ours.
                Some(rate) => utterance.setRate(rate),
                // macOS 14+: use the voice and rate from System Settings > Accessibility >
                // Spoken Content instead of the generic defaults.
                None if utterance.respondsToSelector(sel!(setPrefersAssistiveTechnologySettings:)) => {
                    utterance.setPrefersAssistiveTechnologySettings(true);
                }
                None => {}
            }
            if let Some(volume) = self.volume {
                utterance.setVolume(volume);
            }
            // The synthesizer queues utterances itself, so `speak_queued` needs no extra work.
            self.synth.speakUtterance(&utterance);
        }
    }

    fn stop(&self) {
        // SAFETY: valid synthesizer, main thread.
        unsafe {
            self.synth.stopSpeakingAtBoundary(AVSpeechBoundary::Immediate);
        }
    }
}

fn voiceover_running() -> bool {
    NSWorkspace::sharedWorkspace().isVoiceOverEnabled()
}

/// Asks VoiceOver to speak `text`. High priority interrupts the current speech, medium waits
/// for it (the same mapping browsers use for assertive and polite live regions).
fn announce(mtm: MainThreadMarker, text: &str, interrupt: bool) {
    let priority = if interrupt { NSAccessibilityPriorityLevel::High } else { NSAccessibilityPriorityLevel::Medium };
    let text = NSString::from_str(text);
    let priority = NSNumber::new_isize(priority.0);
    // SAFETY: the keys and the notification name are constant strings exported by AppKit.
    let (announcement_key, priority_key, notification) =
        unsafe { (NSAccessibilityAnnouncementKey, NSAccessibilityPriorityKey, NSAccessibilityAnnouncementRequestedNotification) };
    let objects: [&AnyObject; 2] = [text.as_ref(), priority.as_ref()];
    let info = NSDictionary::from_slices(&[announcement_key, priority_key], &objects);
    // VoiceOver ignores announcements from inactive windows, so post them on the key window
    // when there is one, and on the application otherwise.
    let app = NSApplication::sharedApplication(mtm);
    let window = app.keyWindow();
    let element: &AnyObject = match &window {
        Some(window) => window.as_ref(),
        None => app.as_ref(),
    };
    // SAFETY: valid element, notification name and user info with the documented keys.
    unsafe { NSAccessibilityPostNotificationWithUserInfo(element, notification, Some(&info)) };
}

/// Runs a command on the main thread.
fn on_main(cmd: Command, shared: &Arc<Shared>) {
    let shared = shared.clone();
    DispatchQueue::main().exec_async(move || {
        let Some(mtm) = MainThreadMarker::new() else { return };
        OUTPUT.with_borrow_mut(|output| output.get_or_insert_with(Output::new).handle(mtm, cmd, &shared));
    });
}

/// Forwards commands to the main thread until the channel closes. Fails if AVSpeechSynthesizer
/// is missing (macOS before 10.14).
pub(super) fn run(rx: &Receiver<Command>, shared: &Arc<Shared>) -> Result<(), String> {
    if AnyClass::get(c"AVSpeechSynthesizer").is_none() {
        return Err("AVSpeechSynthesizer is not available (needs macOS 10.14 or newer)".into());
    }
    // Creates the synthesizer early, so the first message is not delayed.
    on_main(Command::Stop, shared);
    for cmd in rx {
        on_main(cmd, shared);
    }
    on_main(Command::Stop, shared);
    Ok(())
}
