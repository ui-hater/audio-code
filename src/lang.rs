//! Translations: play the same game in several languages. Available as `ctx.lang`.
//!
//! Put one text file per language into the `lang` folder next to the game, named by the
//! language code (`en.txt`, `de.txt`, `fr.txt`, ...). Each line is `key = text`:
//!
//! ```text
//! # Lines starting with # are comments.
//! language = Deutsch
//! intro = Willkommen! Drücke Leertaste zum Springen.
//! score = Du hast {points} Punkte.
//! coins.one = 1 Münze
//! coins.other = {n} Münzen
//! ```
//!
//! Then speak keys instead of text:
//!
//! ```no_run
//! # use audio_code::prelude::*;
//! # fn f(ctx: &mut Context, points: u32) {
//! ctx.speak(ctx.tr("intro"));
//! ctx.speak(ctx.tr_with("score", &[("points", &points)]));
//! ctx.speak(ctx.lang.plural("coins", 3)); // "3 Münzen"
//! # }
//! ```
//!
//! * The game starts in the language the player chose last time ([`Context::set_language`](crate::Context::set_language)
//!   saves it), else in [`Engine::language`](crate::Engine::language), else in the system
//!   language if there is a file for it, else in the fallback language (English).
//! * A key missing in the current language comes from the fallback language. A key missing
//!   everywhere is spoken as the key itself and reported once on the console, so a
//!   forgotten translation never crashes the game.
//! * `{name}` is replaced by the argument with that name. `\n` in a text is a line break.
//! * The special key `language` is the language's own name, for a language menu
//!   ([`Lang::name`]).
//! * Texts can also be compiled into the game: `ctx.lang.add_text("de", include_str!("de.txt"))`.
//!
//! The engine's own announcements (menus, dialogs, [`Context::describe`](crate::Context::describe), vehicle speed) are
//! translated as well. English and German are built in; other languages, or different
//! wording, go into your files under the keys listed in `docs/translations.md`.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Display;
use std::path::Path;

use glam::Vec3;

use crate::math::{angle_difference, bearing, clock_position};
use crate::net::Reason;
use crate::vehicle::Vehicle;

/// The translation tables and the current language. Reach it through `ctx.lang`.
#[derive(Debug, Clone)]
pub struct Lang {
    current: String,
    fallback: String,
    tables: BTreeMap<String, HashMap<String, String>>,
    /// Keys already reported as missing, so each is reported only once.
    missing: RefCell<HashSet<String>>,
}

impl Default for Lang {
    fn default() -> Self {
        Self::new()
    }
}

impl Lang {
    /// Empty tables, English as current and fallback language.
    pub fn new() -> Self {
        Self {
            current: "en".into(),
            fallback: "en".into(),
            tables: BTreeMap::new(),
            missing: RefCell::default(),
        }
    }

    /// Loads every `<code>.txt` in `dir`. Returns how many languages were loaded. A missing
    /// folder is fine and loads nothing.
    pub fn load_dir(&mut self, dir: impl AsRef<Path>) -> usize {
        let Ok(entries) = std::fs::read_dir(dir.as_ref()) else {
            return 0;
        };
        let mut count = 0;
        for path in entries.flatten().map(|e| e.path()) {
            if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("txt")) {
                continue;
            }
            let Some(code) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    self.add_text(code, &text);
                    count += 1;
                }
                Err(e) => eprintln!("audio-code: could not read {}: {e}", path.display()),
            }
        }
        count
    }

    /// Adds the lines of a translation file (`key = text`) to a language. Later keys
    /// replace earlier ones. Handy with `include_str!` to ship texts inside the executable.
    pub fn add_text(&mut self, code: &str, text: &str) {
        let table = self.tables.entry(normalize(code)).or_default();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim_start_matches('\u{feff}').trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match line.split_once('=') {
                Some((key, value)) => {
                    table.insert(key.trim().to_string(), value.trim().replace("\\n", "\n"));
                }
                None => eprintln!("audio-code: language {code}, line {}: expected `key = text`", number + 1),
            }
        }
    }

    /// Adds or replaces one text.
    pub fn insert(&mut self, code: &str, key: impl Into<String>, text: impl Into<String>) {
        self.tables.entry(normalize(code)).or_default().insert(key.into(), text.into());
    }

    /// Switches the language. `"de-AT"` falls back to `"de"`. Returns `false` (and keeps the
    /// current language) if no texts exist for it. English and German always exist, because
    /// the engine's own texts are built in. To remember the choice, use
    /// [`Context::set_language`](crate::Context::set_language)(crate::Context::set_language) instead.
    pub fn set(&mut self, code: &str) -> bool {
        match self.resolve(code) {
            Some(code) => {
                self.current = code;
                true
            }
            None => {
                eprintln!("audio-code: no translation for language \"{code}\"");
                false
            }
        }
    }

    /// The current language code, e.g. `"de"`.
    pub fn current(&self) -> &str {
        &self.current
    }

    /// The language used for keys missing in the current one (default `"en"`).
    pub fn fallback(&self) -> &str {
        &self.fallback
    }

    /// Changes the fallback language.
    pub fn set_fallback(&mut self, code: &str) {
        self.fallback = normalize(code);
    }

    /// The languages with loaded texts, sorted by code. Built in engine texts do not count,
    /// so this lists exactly the languages your game was translated to.
    pub fn languages(&self) -> Vec<String> {
        self.tables.keys().cloned().collect()
    }

    /// The name of a language as written in its file (key `language`), else the code.
    pub fn name(&self, code: &str) -> String {
        let code = normalize(code);
        self.tables.get(&code).and_then(|t| t.get("language")).cloned().unwrap_or(code)
    }

    /// Whether `key` has a text in the current or the fallback language.
    pub fn has(&self, key: &str) -> bool {
        self.lookup(key).is_some()
    }

    /// The text for `key` in the current language. See the [module docs](self) for what
    /// happens when it is missing.
    pub fn get(&self, key: &str) -> String {
        match self.lookup(key) {
            Some(text) => text.to_string(),
            None => {
                if self.missing.borrow_mut().insert(key.to_string()) {
                    eprintln!("audio-code: no text for \"{key}\" in language \"{}\"", self.current);
                }
                key.to_string()
            }
        }
    }

    /// The text for `key` with `{name}` placeholders filled in.
    ///
    /// ```no_run
    /// # use audio_code::prelude::*;
    /// # fn f(ctx: &mut Context) {
    /// // score = {player} has {points} points
    /// let text = ctx.lang.format("score", &[("player", &"Anna"), ("points", &12)]);
    /// # }
    /// ```
    pub fn format(&self, key: &str, args: &[(&str, &dyn Display)]) -> String {
        fill(self.get(key), args)
    }

    /// Picks `key.one` when `n` is 1 (or -1) and `key.other` otherwise, and fills in `{n}`.
    /// Without those keys, `key` itself is used.
    pub fn plural(&self, key: &str, n: i64) -> String {
        self.plural_with(key, n, &[])
    }

    /// Like [`Lang::plural`] with more placeholders.
    pub fn plural_with(&self, key: &str, n: i64, args: &[(&str, &dyn Display)]) -> String {
        let form = format!("{key}.{}", if n.abs() == 1 { "one" } else { "other" });
        let text = if self.has(&form) { self.get(&form) } else { self.get(key) };
        fill(fill(text, &[("n", &n)]), args)
    }

    /// The player's system language (e.g. `"de"`), if the system tells.
    pub fn system() -> Option<String> {
        sys_locale::get_locale().map(|l| normalize(&l)).filter(|l| !l.is_empty())
    }

    /// Like [`math::direction_name`](crate::math::direction_name) in the current language.
    pub fn direction_name(&self, degrees: f32) -> String {
        const KEYS: [&str; 8] = [
            "direction.north",
            "direction.north_east",
            "direction.east",
            "direction.south_east",
            "direction.south",
            "direction.south_west",
            "direction.west",
            "direction.north_west",
        ];
        let i = ((crate::math::normalize_angle(degrees) + 22.5) / 45.0) as usize % 8;
        self.get(KEYS[i])
    }

    /// Like [`math::describe_position`](crate::math::describe_position) in the current
    /// language, e.g. `"3 Uhr, 12 Meter"`.
    pub fn describe_position(&self, from: Vec3, yaw: f32, target: Vec3) -> String {
        let d = target - from;
        let distance = self.plural("unit.meters", d.length().round() as i64);
        if d.x.hypot(d.y) < 0.5 && d.z.abs() >= 0.5 {
            let key = if d.z > 0.0 { "describe.directly_above" } else { "describe.directly_below" };
            return self.format(key, &[("distance", &distance)]);
        }
        let hour = clock_position(angle_difference(yaw, bearing(from, target)));
        let mut text = format!("{}, {distance}", self.format("describe.clock", &[("hour", &hour)]));
        let height = d.z.round() as i64;
        if height != 0 {
            let key = if height > 0 { "describe.above" } else { "describe.below" };
            let meters = self.plural("unit.meters", height.abs());
            text.push_str(", ");
            text.push_str(&self.format(key, &[("distance", &meters)]));
        }
        text
    }

    /// Like [`Vehicle::describe_speed`] in the current language.
    pub fn describe_speed(&self, vehicle: &Vehicle) -> String {
        let speed = self.plural("unit.kmh", vehicle.speed_kmh().abs().round() as i64);
        if vehicle.is_reversing() { self.format("vehicle.reversing", &[("speed", &speed)]) } else { speed }
    }

    /// Why a network connection ended, as a sentence in the current language, e.g.
    /// `"The server is full"`.
    ///
    /// ```no_run
    /// # use audio_code::prelude::*;
    /// # fn f(ctx: &mut Context) {
    /// for (_, reason) in ctx.net.left() {
    ///     ctx.speak(ctx.lang.describe_reason(&reason));
    /// }
    /// # }
    /// ```
    pub fn describe_reason(&self, reason: &Reason) -> String {
        match reason {
            Reason::Left => self.get("net.left"),
            Reason::Closed => self.get("net.closed"),
            Reason::Kicked(text) if text.is_empty() => self.get("net.kicked"),
            Reason::Kicked(text) => self.format("net.kicked_reason", &[("reason", text)]),
            Reason::ServerFull => self.get("net.server_full"),
            Reason::WrongGame(game) => self.format("net.wrong_game", &[("game", game)]),
            Reason::WrongVersion(version) => self.format("net.wrong_version", &[("version", version)]),
            Reason::TimedOut => self.get("net.timed_out"),
            Reason::Failed(error) => self.format("net.failed", &[("error", error)]),
            Reason::Lost(error) => self.format("net.lost", &[("error", error)]),
        }
    }

    fn lookup(&self, key: &str) -> Option<&str> {
        let table = |code: &str| self.tables.get(code).and_then(|t| t.get(key)).map(String::as_str);
        table(&self.current)
            .or_else(|| table(&self.fallback))
            .or_else(|| builtin(&self.current, key))
            .or_else(|| builtin(&self.fallback, key))
            .or_else(|| builtin("en", key))
    }

    /// The code under which texts for `code` exist: the code itself or its main language.
    pub(crate) fn resolve(&self, code: &str) -> Option<String> {
        let code = normalize(code);
        let main = code.split('-').next().unwrap_or_default().to_string();
        let exists = |c: &str| self.tables.contains_key(c) || BUILTIN.contains(&c);
        [code, main].into_iter().find(|c| exists(c))
    }

    /// Like [`Lang::resolve`], but only for languages with loaded texts.
    pub(crate) fn resolve_loaded(&self, code: &str) -> Option<String> {
        self.resolve(code).filter(|c| self.tables.contains_key(c))
    }
}

/// `"de_DE.UTF-8"` → `"de-de"`.
fn normalize(code: &str) -> String {
    let code = code.split(['.', '@']).next().unwrap_or_default();
    code.trim().replace('_', "-").to_lowercase()
}

fn fill(mut text: String, args: &[(&str, &dyn Display)]) -> String {
    for (name, value) in args {
        let pattern = format!("{{{name}}}");
        if text.contains(&pattern) {
            text = text.replace(&pattern, &value.to_string());
        }
    }
    text
}

/// Languages the engine's own texts are built in for.
const BUILTIN: [&str; 2] = ["en", "de"];

/// The engine's own texts.
fn builtin(code: &str, key: &str) -> Option<&'static str> {
    let text = match code {
        "en" => match key {
            "ui.checked" => "checked",
            "ui.not_checked" => "not checked",
            "ui.blank" => "blank",
            "ui.empty" => "empty",
            "ui.edit" => "{label}, edit, {value}",
            "ui.characters.one" => "1 character",
            "ui.characters.other" => "{n} characters",
            "ui.position" => "{index} of {count}",
            "ui.yes" => "Yes",
            "ui.no" => "No",
            "ui.star" => "star",
            "char.space" => "space",
            "char.dot" => "dot",
            "char.comma" => "comma",
            "char.exclamation" => "exclamation",
            "char.question" => "question",
            "char.dash" => "dash",
            "char.underscore" => "underscore",
            "char.colon" => "colon",
            "char.semicolon" => "semicolon",
            "char.apostrophe" => "apostrophe",
            "char.quote" => "quote",
            "char.slash" => "slash",
            "char.backslash" => "backslash",
            "char.at" => "at",
            "char.number" => "number",
            "char.left_paren" => "left paren",
            "char.right_paren" => "right paren",
            "direction.north" => "north",
            "direction.north_east" => "north east",
            "direction.east" => "east",
            "direction.south_east" => "south east",
            "direction.south" => "south",
            "direction.south_west" => "south west",
            "direction.west" => "west",
            "direction.north_west" => "north west",
            "describe.clock" => "{hour} o'clock",
            "describe.above" => "{distance} above",
            "describe.below" => "{distance} below",
            "describe.directly_above" => "directly above, {distance}",
            "describe.directly_below" => "directly below, {distance}",
            "unit.meters.one" => "1 meter",
            "unit.meters.other" => "{n} meters",
            "unit.kmh.one" => "1 kilometer per hour",
            "unit.kmh.other" => "{n} kilometers per hour",
            "vehicle.reversing" => "reversing, {speed}",
            "net.left" => "Left the game",
            "net.closed" => "The server was closed",
            "net.kicked" => "Kicked from the server",
            "net.kicked_reason" => "Kicked from the server: {reason}",
            "net.server_full" => "The server is full",
            "net.wrong_game" => "This server runs another game: {game}",
            "net.wrong_version" => "Wrong game version. The server has version {version}",
            "net.timed_out" => "The connection timed out",
            "net.failed" => "Could not connect: {error}",
            "net.lost" => "Connection lost: {error}",
            _ => return None,
        },
        "de" => match key {
            "ui.checked" => "aktiviert",
            "ui.not_checked" => "nicht aktiviert",
            "ui.blank" => "leer",
            "ui.empty" => "leer",
            "ui.edit" => "{label}, Eingabefeld, {value}",
            "ui.characters.one" => "1 Zeichen",
            "ui.characters.other" => "{n} Zeichen",
            "ui.position" => "{index} von {count}",
            "ui.yes" => "Ja",
            "ui.no" => "Nein",
            "ui.star" => "Stern",
            "char.space" => "Leerzeichen",
            "char.dot" => "Punkt",
            "char.comma" => "Komma",
            "char.exclamation" => "Ausrufezeichen",
            "char.question" => "Fragezeichen",
            "char.dash" => "Bindestrich",
            "char.underscore" => "Unterstrich",
            "char.colon" => "Doppelpunkt",
            "char.semicolon" => "Semikolon",
            "char.apostrophe" => "Apostroph",
            "char.quote" => "Anführungszeichen",
            "char.slash" => "Schrägstrich",
            "char.backslash" => "Backslash",
            "char.at" => "at",
            "char.number" => "Raute",
            "char.left_paren" => "Klammer auf",
            "char.right_paren" => "Klammer zu",
            "direction.north" => "Norden",
            "direction.north_east" => "Nordosten",
            "direction.east" => "Osten",
            "direction.south_east" => "Südosten",
            "direction.south" => "Süden",
            "direction.south_west" => "Südwesten",
            "direction.west" => "Westen",
            "direction.north_west" => "Nordwesten",
            "describe.clock" => "{hour} Uhr",
            "describe.above" => "{distance} darüber",
            "describe.below" => "{distance} darunter",
            "describe.directly_above" => "direkt darüber, {distance}",
            "describe.directly_below" => "direkt darunter, {distance}",
            "unit.meters.one" => "1 Meter",
            "unit.meters.other" => "{n} Meter",
            "unit.kmh.one" => "1 Kilometer pro Stunde",
            "unit.kmh.other" => "{n} Kilometer pro Stunde",
            "vehicle.reversing" => "rückwärts, {speed}",
            "net.left" => "Hat das Spiel verlassen",
            "net.closed" => "Der Server wurde geschlossen",
            "net.kicked" => "Vom Server geworfen",
            "net.kicked_reason" => "Vom Server geworfen: {reason}",
            "net.server_full" => "Der Server ist voll",
            "net.wrong_game" => "Auf diesem Server läuft ein anderes Spiel: {game}",
            "net.wrong_version" => "Falsche Spielversion. Der Server hat Version {version}",
            "net.timed_out" => "Zeitüberschreitung der Verbindung",
            "net.failed" => "Verbindung fehlgeschlagen: {error}",
            "net.lost" => "Verbindung verloren: {error}",
            _ => return None,
        },
        _ => return None,
    };
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lang() -> Lang {
        let mut lang = Lang::new();
        lang.add_text("en", "# comment\nlanguage = English\nhello = Hello, {name}!\ncoins.one = 1 coin\ncoins.other = {n} coins\nonly_en = fallback\n");
        lang.add_text("de", "\u{feff}language = Deutsch\nhello = Hallo, {name}!\nlines = a\\nb\n");
        lang
    }

    #[test]
    fn looks_up_formats_and_falls_back() {
        let mut lang = lang();
        assert_eq!(lang.format("hello", &[("name", &"Anna")]), "Hello, Anna!");
        assert!(lang.set("de_AT.UTF-8"));
        assert_eq!(lang.current(), "de");
        assert_eq!(lang.format("hello", &[("name", &"Anna")]), "Hallo, Anna!");
        assert_eq!(lang.get("only_en"), "fallback");
        assert_eq!(lang.get("lines"), "a\nb");
        assert_eq!(lang.get("nothing"), "nothing");
        assert!(!lang.set("fr"));
        assert_eq!(lang.current(), "de");
    }

    #[test]
    fn plurals_and_names() {
        let lang = lang();
        assert_eq!(lang.plural("coins", 1), "1 coin");
        assert_eq!(lang.plural("coins", 4), "4 coins");
        assert_eq!(lang.plural("hello", 4), "Hello, {name}!");
        assert_eq!(lang.languages(), ["de", "en"]);
        assert_eq!(lang.name("de"), "Deutsch");
        assert_eq!(lang.name("fr"), "fr");
    }

    #[test]
    fn engine_texts_are_built_in() {
        let mut lang = Lang::new();
        assert!(lang.set("de"));
        assert_eq!(lang.get("ui.checked"), "aktiviert");
        assert_eq!(lang.describe_position(Vec3::ZERO, 0.0, Vec3::new(5.0, 0.0, 0.0)), "3 Uhr, 5 Meter");
        assert_eq!(lang.describe_position(Vec3::ZERO, 0.0, Vec3::new(0.0, 0.0, -1.0)), "direkt darunter, 1 Meter");
        assert_eq!(lang.describe_reason(&Reason::ServerFull), "Der Server ist voll");
        assert_eq!(lang.describe_reason(&Reason::Kicked("Spam".into())), "Vom Server geworfen: Spam");
        assert_eq!(lang.direction_name(180.0), "Süden");
        lang.insert("de", "ui.checked", "an");
        assert_eq!(lang.get("ui.checked"), "an", "game texts win over built in ones");
        assert!(lang.languages() == ["de"]);
    }
}
