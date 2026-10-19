//! Simple saving: high scores, settings, progress. Available as `ctx.storage`.
//!
//! Values are stored as text in a small `key = value` file that is loaded when the game
//! starts and written when it quits (or when you call [`Storage::save`]).
//!
//! ```no_run
//! # use audio_code::prelude::*;
//! # fn f(ctx: &mut Context, score: u32) {
//! let best: u32 = ctx.storage.get_or("highscore", 0);
//! if score > best {
//!     ctx.storage.set("highscore", score);
//!     ctx.speak("New high score!");
//! }
//! let volume: f32 = ctx.storage.get_or("volume", 0.8);
//! # }
//! ```
//!
//! The file is `<game title>.save` in the working directory (in a macOS app bundle:
//! `~/Library/Application Support/<game title>/`) unless set with
//! [`Engine::save_file`](crate::Engine::save_file). Anything that implements `Display` and
//! `FromStr` can be stored: numbers, `bool`, `String`, ...
//!
//! ## Encryption (optional)
//!
//! By default the file is plain text, so players can read and edit it. With
//! [`Engine::encrypt_save`](crate::Engine::encrypt_save) (or [`Storage::open_encrypted`]) the
//! file is encrypted with XChaCha20-Poly1305, using a key made from a secret word you choose:
//!
//! ```no_run
//! # use audio_code::prelude::*;
//! # struct Game;
//! # impl Scene for Game { fn update(&mut self, _: &mut Context) {} }
//! fn main() -> audio_code::Result<()> {
//!     Engine::new("My game").encrypt_save("my secret word").run(Game)
//! }
//! ```
//!
//! * Nothing else changes: `ctx.storage.get` / `set` work as before.
//! * The file cannot be read, and any change to it is detected. A file that cannot be
//!   decrypted (wrong secret, edited or damaged) is not overwritten: it is renamed to
//!   `<file>.broken`, the game starts with empty storage, and [`Storage::load_error`] says why.
//! * An existing plain text save is read once and written encrypted on the next save, so
//!   turning encryption on keeps the players' progress.
//! * The secret is compiled into your game, so a determined player can dig it out of the
//!   executable. Encryption stops casual cheating and accidental edits; it is not a safe place
//!   for real secrets such as passwords.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// Start of an encrypted save file. Also authenticated, so it cannot be swapped.
const MAGIC: &[u8] = b"AUDIOCODE-SAVE-ENC1\n";
const NONCE_LEN: usize = 24;

/// Key-value storage that survives restarts, optionally encrypted.
#[derive(Default)]
pub struct Storage {
    path: Option<PathBuf>,
    values: BTreeMap<String, String>,
    dirty: bool,
    key: Option<[u8; 32]>,
    load_error: Option<String>,
}

impl std::fmt::Debug for Storage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the key.
        f.debug_struct("Storage")
            .field("path", &self.path)
            .field("values", &self.values.len())
            .field("encrypted", &self.key.is_some())
            .field("load_error", &self.load_error)
            .finish()
    }
}

impl Storage {
    /// Storage that lives only in memory and is never written.
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Loads storage from a plain text file at `path`. A missing file simply gives empty
    /// storage. An encrypted file cannot be read without its secret: it is renamed to
    /// `<file>.broken` and [`Storage::load_error`] explains why.
    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self::load(path.into(), None)
    }

    /// Loads storage from an encrypted file at `path`, using `secret` to make the key. A
    /// missing file gives empty storage. A plain text file is read and written encrypted on
    /// the next save. A file that cannot be decrypted (wrong secret, edited or damaged) is
    /// renamed to `<file>.broken`, and [`Storage::load_error`] explains why.
    pub fn open_encrypted(path: impl Into<PathBuf>, secret: &str) -> Self {
        Self::load(path.into(), Some(derive_key(secret)))
    }

    fn load(path: PathBuf, key: Option<[u8; 32]>) -> Self {
        let mut storage = Self { path: Some(path.clone()), key, ..Self::default() };
        let Ok(bytes) = std::fs::read(&path) else {
            return storage;
        };
        let text = match (bytes.strip_prefix(MAGIC), key) {
            (Some(sealed), Some(key)) => decrypt(&key, sealed),
            (Some(_), None) => Err("the save file is encrypted, but no secret was given".to_string()),
            (None, _) => {
                // Plain text: with a key it is rewritten encrypted on the next save.
                storage.dirty = key.is_some();
                String::from_utf8(bytes).map_err(|_| "the save file is not valid text".to_string())
            }
        };
        match text {
            Ok(text) => storage.parse(&text),
            Err(reason) => storage.keep_broken(reason),
        }
        storage
    }

    fn parse(&mut self, text: &str) {
        for line in text.lines() {
            if let Some((key, value)) = line.split_once('=') {
                self.values.insert(key.trim().to_string(), unescape(value.trim_start()));
            }
        }
    }

    /// Moves an unreadable file out of the way, so saving does not destroy it.
    fn keep_broken(&mut self, reason: String) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let mut backup = path.clone().into_os_string();
        backup.push(".broken");
        let message = match std::fs::rename(&path, &backup) {
            Ok(()) => format!("{reason}; it was kept as {}", PathBuf::from(&backup).display()),
            Err(_) => {
                // Could not move it: never write over it.
                self.path = None;
                format!("{reason}; saving is disabled so the file is not overwritten")
            }
        };
        eprintln!("audio-code: could not load {}: {message}", path.display());
        self.load_error = Some(message);
    }

    /// Why the save file could not be loaded, if it could not: wrong secret, edited or
    /// damaged file. The unreadable file was kept as `<file>.broken`. `None` when loading
    /// worked or there was no file.
    ///
    /// ```no_run
    /// # use audio_code::prelude::*;
    /// # fn f(ctx: &mut Context) {
    /// if ctx.storage.load_error().is_some() {
    ///     ctx.speak("Your save file is damaged. Starting a new game.");
    /// }
    /// # }
    /// ```
    pub fn load_error(&self) -> Option<&str> {
        self.load_error.as_deref()
    }

    /// Whether the file is written encrypted.
    pub fn is_encrypted(&self) -> bool {
        self.key.is_some()
    }

    /// Turns encryption on with a secret (`Some`) or off (`None`). The file is rewritten in
    /// the new form on the next save.
    pub fn set_encryption(&mut self, secret: Option<&str>) {
        self.key = secret.map(derive_key);
        self.dirty = true;
    }

    /// The file this storage is saved to, if any.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Reads a value. Returns `None` if it is missing or cannot be parsed as `T`.
    pub fn get<T: FromStr>(&self, key: &str) -> Option<T> {
        self.values.get(key).and_then(|v| v.parse().ok())
    }

    /// Reads a value, or `default` if it is missing or invalid.
    pub fn get_or<T: FromStr>(&self, key: &str, default: T) -> T {
        self.get(key).unwrap_or(default)
    }

    /// Stores a value. It is written to disk when the game quits or [`Storage::save`] is called.
    ///
    /// # Panics
    /// If `key` contains `=` or a line break.
    pub fn set(&mut self, key: &str, value: impl ToString) {
        assert!(!key.contains(['=', '\n', '\r']), "storage keys must not contain '=' or line breaks");
        let value = value.to_string();
        if self.values.get(key) != Some(&value) {
            self.values.insert(key.trim().to_string(), value);
            self.dirty = true;
        }
    }

    /// Adds `amount` to a number (missing counts as 0) and returns the new value.
    /// Handy for statistics: `ctx.storage.add("games_played", 1)`.
    pub fn add(&mut self, key: &str, amount: i64) -> i64 {
        let value = self.get_or::<i64>(key, 0) + amount;
        self.set(key, value);
        value
    }

    /// Removes a value.
    pub fn remove(&mut self, key: &str) {
        if self.values.remove(key).is_some() {
            self.dirty = true;
        }
    }

    /// Whether a value exists.
    pub fn contains(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// All keys in alphabetical order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }

    /// Removes everything.
    pub fn clear(&mut self) {
        if !self.values.is_empty() {
            self.values.clear();
            self.dirty = true;
        }
    }

    /// Writes changes to disk now. Does nothing for in-memory storage or without changes.
    pub fn save(&mut self) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if !self.dirty {
            return Ok(());
        }
        let mut text = String::new();
        for (key, value) in &self.values {
            text.push_str(key);
            text.push_str(" = ");
            text.push_str(&escape(value));
            text.push('\n');
        }
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let bytes = match &self.key {
            Some(key) => encrypt(key, text.as_bytes())?,
            None => text.into_bytes(),
        };
        std::fs::write(path, bytes)?;
        self.dirty = false;
        Ok(())
    }
}

/// Makes a 256 bit key from the secret word.
fn derive_key(secret: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"audio-code save key v1\0");
    hash.update(secret.as_bytes());
    hash.finalize().into()
}

/// `MAGIC`, a random nonce, then the ciphertext with its authentication tag.
fn encrypt(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;
    let cipher = XChaCha20Poly1305::new(&Key::from(*key));
    let sealed = cipher
        .encrypt(&XNonce::from(nonce), Payload { msg: plain, aad: MAGIC })
        .map_err(|_| Error::Io(std::io::Error::other("could not encrypt the save file")))?;
    let mut out = Vec::with_capacity(MAGIC.len() + NONCE_LEN + sealed.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// Decrypts what follows `MAGIC` in a save file.
fn decrypt(key: &[u8; 32], sealed: &[u8]) -> std::result::Result<String, String> {
    let damaged = || "the save file was changed, is damaged, or was saved with another secret".to_string();
    if sealed.len() < NONCE_LEN {
        return Err(damaged());
    }
    let (nonce, data) = sealed.split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| damaged())?;
    let cipher = XChaCha20Poly1305::new(&Key::from(*key));
    let plain = cipher.decrypt(&XNonce::from(nonce), Payload { msg: data, aad: MAGIC }).map_err(|_| damaged())?;
    String::from_utf8(plain).map_err(|_| damaged())
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "\\r")
}

fn unescape(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Turns a game title into a safe file name.
pub(crate) fn default_file(title: &str) -> PathBuf {
    let name: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let name = if name.trim_matches('_').is_empty() { "game".to_string() } else { name };
    let file = PathBuf::from(format!("{name}.save"));
    // Inside a macOS app bundle the working directory is `/`, which is not writable.
    match crate::bundle::data_dir(&name) {
        Some(dir) => dir.join(file),
        None => file,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join(format!("audio-code-storage-{}", std::process::id()));
        let path = dir.join("test.save");
        let mut s = Storage::open(&path);
        s.set("score", 42);
        s.set("name", "Line one\nback\\slash = ok");
        s.set("on", true);
        assert_eq!(s.add("plays", 2), 2);
        s.save().unwrap();

        let s = Storage::open(&path);
        assert_eq!(s.get::<u32>("score"), Some(42));
        assert_eq!(s.get::<String>("name").as_deref(), Some("Line one\nback\\slash = ok"));
        assert!(s.get_or("on", false));
        assert_eq!(s.get_or("missing", 7), 7);
        assert_eq!(s.get::<u32>("name"), None);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn encrypted_round_trip_and_tampering() {
        let dir = std::env::temp_dir().join(format!("audio-code-encrypted-{}", std::process::id()));
        let path = dir.join("game.save");
        let mut s = Storage::open_encrypted(&path, "secret");
        s.set("score", 42);
        s.set("name", "Hero = best");
        s.save().unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(MAGIC));
        assert!(!String::from_utf8_lossy(&bytes).contains("score"), "not readable as text");

        let s = Storage::open_encrypted(&path, "secret");
        assert_eq!(s.get::<u32>("score"), Some(42));
        assert_eq!(s.get::<String>("name").as_deref(), Some("Hero = best"));
        assert!(s.load_error().is_none());
        assert!(s.is_encrypted());

        // Wrong secret: the file is kept as .broken and not overwritten.
        let wrong = Storage::open_encrypted(&path, "other");
        assert!(wrong.load_error().is_some());
        assert_eq!(wrong.get::<u32>("score"), None);
        let broken = dir.join("game.save.broken");
        assert!(broken.is_file());
        assert!(!path.exists());
        let kept = Storage::open_encrypted(&broken, "secret");
        assert_eq!(kept.get::<u32>("score"), Some(42), "the kept file is intact");
        std::fs::remove_file(&broken).unwrap();

        // Tampering: flip one byte of the ciphertext.
        let mut s = Storage::open_encrypted(&path, "secret");
        s.set("score", 42);
        s.save().unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        std::fs::write(&path, &bytes).unwrap();
        let s = Storage::open_encrypted(&path, "secret");
        assert!(s.load_error().unwrap().contains("changed"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn plain_save_is_migrated_and_encrypted_needs_secret() {
        let dir = std::env::temp_dir().join(format!("audio-code-migrate-{}", std::process::id()));
        let path = dir.join("game.save");
        let mut plain = Storage::open(&path);
        plain.set("level", 3);
        plain.save().unwrap();

        let mut s = Storage::open_encrypted(&path, "secret");
        assert_eq!(s.get::<u32>("level"), Some(3), "old plain save still loads");
        s.save().unwrap();
        assert!(std::fs::read(&path).unwrap().starts_with(MAGIC), "written encrypted");

        let s = Storage::open(&path);
        assert!(s.load_error().unwrap().contains("no secret"));
        assert!(dir.join("game.save.broken").is_file());
        assert!(!format!("{s:?}").contains("key"));

        let mut off = Storage::open_encrypted(dir.join("game.save.broken"), "secret");
        off.set_encryption(None);
        off.save().unwrap();
        let text = std::fs::read_to_string(dir.join("game.save.broken")).unwrap();
        assert!(text.contains("level = 3"), "turned off: plain text again");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn file_names() {
        assert_eq!(default_file("My Game: 2"), PathBuf::from("My_Game__2.save"));
        assert_eq!(default_file("?!"), PathBuf::from("game.save"));
    }
}
