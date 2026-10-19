//! Speech through NVDA's controller client DLL, loaded at runtime.
//!
//! The DLL is part of NVDA's "controllerClient" download and is licensed under the LGPL 2.1.
//! It is not bundled with this crate: ship it next to your game's executable.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;

use super::{Command, Shared};

type NoArgs = unsafe extern "system" fn() -> u32;
type TextArg = unsafe extern "system" fn(*const u16) -> u32;

/// File names the DLL has had over the years, newest first.
#[cfg(target_pointer_width = "64")]
const NAMES: [&str; 2] = ["nvdaControllerClient.dll", "nvdaControllerClient64.dll"];
#[cfg(not(target_pointer_width = "64"))]
const NAMES: [&str; 2] = ["nvdaControllerClient.dll", "nvdaControllerClient32.dll"];

struct Client {
    // Keeps the DLL loaded while the function pointers below are in use.
    _lib: libloading::Library,
    test_if_running: NoArgs,
    speak_text: TextArg,
    cancel_speech: NoArgs,
    braille_message: TextArg,
}

impl Client {
    fn load(custom: Option<&Path>) -> Result<Self, String> {
        let mut errors = Vec::new();
        for path in candidates(custom) {
            // SAFETY: loading the controller client only sets up an RPC binding to NVDA.
            match unsafe { libloading::Library::new(&path) } {
                Ok(lib) => return Self::from_library(lib).map_err(|e| format!("{}: {e}", path.display())),
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
        Err(if custom.is_some() { errors.join(", ") } else { format!("{} not found", NAMES[0]) })
    }

    fn from_library(lib: libloading::Library) -> Result<Self, libloading::Error> {
        // SAFETY: the signatures match nvdaController.h; all functions return error_status_t.
        unsafe {
            let test_if_running = *lib.get::<NoArgs>("nvdaController_testIfRunning")?;
            let speak_text = *lib.get::<TextArg>("nvdaController_speakText")?;
            let cancel_speech = *lib.get::<NoArgs>("nvdaController_cancelSpeech")?;
            let braille_message = *lib.get::<TextArg>("nvdaController_brailleMessage")?;
            Ok(Self { _lib: lib, test_if_running, speak_text, cancel_speech, braille_message })
        }
    }

    fn is_running(&self) -> bool {
        // SAFETY: valid function pointer from the loaded DLL, no arguments.
        unsafe { (self.test_if_running)() == 0 }
    }

    fn speak(&self, text: &str, interrupt: bool) {
        let wide = wide(text);
        // SAFETY: `wide` is a NUL terminated UTF-16 string that outlives the calls.
        unsafe {
            if interrupt {
                (self.cancel_speech)();
            }
            (self.speak_text)(wide.as_ptr());
        }
    }

    fn braille(&self, text: &str) {
        let wide = wide(text);
        // SAFETY: as in `speak`.
        unsafe {
            (self.braille_message)(wide.as_ptr());
        }
    }

    fn cancel(&self) {
        // SAFETY: valid function pointer from the loaded DLL, no arguments.
        unsafe {
            (self.cancel_speech)();
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Where to look for the DLL: the configured path (file or folder), next to the executable,
/// the working directory, and finally the normal Windows DLL search path.
fn candidates(custom: Option<&Path>) -> Vec<PathBuf> {
    if let Some(custom) = custom {
        if custom.is_dir() {
            return NAMES.iter().map(|n| custom.join(n)).collect();
        }
        return vec![custom.to_path_buf()];
    }
    let mut dirs = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf)) {
        dirs.push(dir);
    }
    if let Ok(dir) = std::env::current_dir() {
        dirs.push(dir);
    }
    let mut paths: Vec<PathBuf> =
        dirs.iter().flat_map(|d| NAMES.iter().map(move |n| d.join(n))).filter(|p| p.is_file()).collect();
    paths.extend(NAMES.iter().map(PathBuf::from));
    paths
}

/// Runs the NVDA output until the channel closes. Fails if the DLL cannot be loaded.
/// If NVDA is not running when a message arrives, the message is printed instead.
pub(super) fn run(rx: &Receiver<Command>, shared: &Shared, dll: Option<&Path>) -> Result<(), String> {
    let client = Client::load(dll)?;
    shared.set_backend("NVDA (controller client)".into());
    // NVDA speaks with the voice the player chose in NVDA.
    shared.set_voices(Vec::new());
    for cmd in rx {
        let running = client.is_running();
        match cmd {
            Command::Speak { text, interrupt } if running => client.speak(&text, interrupt),
            Command::Speak { text, .. } => {
                if !shared.echo.load(Ordering::Relaxed) {
                    println!("{text}");
                }
            }
            Command::Braille(text) if running => client.braille(&text),
            Command::Stop if running => client.cancel(),
            // NVDA keeps the user's own rate and volume.
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_dll_is_an_error() {
        let err = Client::load(Some(Path::new("does/not/exist.dll"))).err().unwrap();
        assert!(err.contains("exist.dll"));
    }

    #[test]
    fn utf16_is_terminated() {
        assert_eq!(wide("ä"), vec![0xE4, 0]);
    }
}
