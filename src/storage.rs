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
//! The file is `<game title>.save` in the working directory unless set with
//! [`Engine::save_file`](crate::Engine::save_file). Anything that implements `Display` and
//! `FromStr` can be stored: numbers, `bool`, `String`, ...

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::error::Result;

/// Key-value storage that survives restarts.
#[derive(Debug, Default)]
pub struct Storage {
    path: Option<PathBuf>,
    values: BTreeMap<String, String>,
    dirty: bool,
}

impl Storage {
    /// Storage that lives only in memory and is never written.
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Loads storage from `path`. A missing file simply gives empty storage.
    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut storage = Self { path: Some(path.clone()), ..Self::default() };
        if let Ok(text) = std::fs::read_to_string(&path) {
            for line in text.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    storage.values.insert(key.trim().to_string(), unescape(value.trim_start()));
                }
            }
        }
        storage
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
        std::fs::write(path, text)?;
        self.dirty = false;
        Ok(())
    }
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
    PathBuf::from(format!("{name}.save"))
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
    fn file_names() {
        assert_eq!(default_file("My Game: 2"), PathBuf::from("My_Game__2.save"));
        assert_eq!(default_file("?!"), PathBuf::from("game.save"));
    }
}
