//! Message boxes, yes/no questions and text input.

use super::{Status, UiContext, UiEvent, UiSound, Value, Widget};
use crate::input::Key;

/// A message the player reads and dismisses with Enter, Space or Escape.
/// Up/Down/Home/End review the text line by line.
#[derive(Debug, Clone)]
pub struct Message {
    id: String,
    text: String,
    lines: Vec<String>,
    line: usize,
}

impl Message {
    /// Creates a message.
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let lines = text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
        Self { id: id.into(), text, lines, line: 0 }
    }
}

impl Widget for Message {
    fn id(&self) -> &str {
        &self.id
    }

    fn open(&mut self, ui: &mut UiContext<'_>) {
        ui.play(UiSound::Open, None);
        ui.speech.speak(&self.text);
    }

    fn update(&mut self, ui: &mut UiContext<'_>) -> Status {
        let last = self.lines.len().saturating_sub(1);
        let target = if ui.key(Key::Up) {
            Some(self.line.saturating_sub(1))
        } else if ui.key(Key::Down) {
            Some((self.line + 1).min(last))
        } else if ui.key(Key::Home) {
            Some(0)
        } else if ui.key(Key::End) {
            Some(last)
        } else {
            None
        };
        if let Some(t) = target {
            ui.play(if t == self.line { UiSound::Edge } else { UiSound::Navigate }, None);
            self.line = t;
            if let Some(line) = self.lines.get(t) {
                ui.speech.speak(line);
            }
        }
        if ui.key(Key::Enter) || ui.key(Key::NumpadEnter) || ui.input.pressed(Key::Space) || ui.key(Key::Escape) {
            ui.play(UiSound::Close, None);
            ui.emit(UiEvent::Closed { id: self.id.clone() });
            return Status::Close;
        }
        Status::Open
    }
}

/// A yes/no question. Arrows switch the answer, Enter confirms, Escape answers no.
/// The first letters of the answers work as shortcuts.
#[derive(Debug, Clone)]
pub struct Confirm {
    id: String,
    question: String,
    /// Answer texts; `None` uses `ui.yes` / `ui.no` of the current language.
    yes: Option<String>,
    no: Option<String>,
    selected_yes: bool,
}

impl Confirm {
    /// Creates a question with "Yes" and "No" answers (in the current language, see
    /// [`crate::lang`]), "Yes" focused.
    pub fn new(id: impl Into<String>, question: impl Into<String>) -> Self {
        Self { id: id.into(), question: question.into(), yes: None, no: None, selected_yes: true }
    }

    /// Changes the answer texts, e.g. `.labels("Ja", "Nein")`.
    pub fn labels(mut self, yes: impl Into<String>, no: impl Into<String>) -> Self {
        self.yes = Some(yes.into());
        self.no = Some(no.into());
        self
    }

    /// Focus "No" initially (for dangerous actions).
    pub fn default_no(mut self) -> Self {
        self.selected_yes = false;
        self
    }

    fn label(&self, yes: bool, ui: &UiContext<'_>) -> String {
        let (custom, key) = if yes { (&self.yes, "ui.yes") } else { (&self.no, "ui.no") };
        custom.clone().unwrap_or_else(|| ui.lang.get(key))
    }

    fn current(&self, ui: &UiContext<'_>) -> String {
        self.label(self.selected_yes, ui)
    }

    fn answer(&self, yes: bool, ui: &mut UiContext<'_>) -> Status {
        ui.play(if yes { UiSound::Activate } else { UiSound::Close }, None);
        ui.emit(UiEvent::Confirmed { id: self.id.clone(), yes });
        Status::Close
    }
}

fn first_lower(s: &str) -> Option<char> {
    s.chars().next().and_then(|c| c.to_lowercase().next())
}

impl Widget for Confirm {
    fn id(&self) -> &str {
        &self.id
    }

    fn open(&mut self, ui: &mut UiContext<'_>) {
        ui.play(UiSound::Open, None);
        let answer = self.current(ui);
        ui.speech.speak(format!("{} {answer}", self.question));
    }

    fn update(&mut self, ui: &mut UiContext<'_>) -> Status {
        if ui.key(Key::Up) || ui.key(Key::Down) || ui.key(Key::Left) || ui.key(Key::Right) || ui.key(Key::Tab) {
            self.selected_yes = !self.selected_yes;
            ui.play(UiSound::Navigate, None);
            let text = self.current(ui);
            ui.speech.speak(text);
        } else if ui.key(Key::Enter) || ui.key(Key::NumpadEnter) || ui.input.pressed(Key::Space) {
            return self.answer(self.selected_yes, ui);
        } else if ui.key(Key::Escape) {
            return self.answer(false, ui);
        } else if let Some(c) = ui.input.text().chars().next().and_then(|c| c.to_lowercase().next()) {
            if Some(c) == first_lower(&self.label(true, ui)) {
                return self.answer(true, ui);
            }
            if Some(c) == first_lower(&self.label(false, ui)) {
                return self.answer(false, ui);
            }
        }
        Status::Open
    }
}

/// A single line text field.
///
/// | Key | Action |
/// |-----|--------|
/// | typing | inserts and speaks the character |
/// | Left / Right | moves the cursor and speaks the character |
/// | Control + Left / Right | moves by word |
/// | Home / End | start / end |
/// | Backspace / Delete | deletes and speaks the character |
/// | Up / Down | speaks the whole text |
/// | Enter | submits ([`UiEvent::TextSubmitted`]) |
/// | Escape | cancels ([`UiEvent::Closed`]) |
#[derive(Debug, Clone)]
pub struct TextInput {
    id: String,
    prompt: String,
    text: Vec<char>,
    cursor: usize,
    password: bool,
    max_length: Option<usize>,
    numeric: bool,
    allow_empty: bool,
    child: bool,
}

impl TextInput {
    /// Creates an empty text field with a spoken prompt.
    pub fn new(id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            text: Vec::new(),
            cursor: 0,
            password: false,
            max_length: None,
            numeric: false,
            allow_empty: true,
            child: false,
        }
    }

    /// Pre-fills the field; the cursor is placed at the end.
    pub fn initial(mut self, text: impl Into<String>) -> Self {
        self.text = text.into().chars().collect();
        self.cursor = self.text.len();
        self
    }

    /// Speaks "star" instead of the typed characters.
    pub fn password(mut self) -> Self {
        self.password = true;
        self
    }

    /// Limits the number of characters.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_length = Some(max);
        self
    }

    /// Accepts only digits, minus and decimal point.
    pub fn numeric(mut self) -> Self {
        self.numeric = true;
        self
    }

    /// Refuses to submit an empty text (default: empty is allowed).
    pub fn required(mut self) -> Self {
        self.allow_empty = false;
        self
    }

    /// Hands the result to the widget below instead of emitting events (used by menus).
    pub(crate) fn child(mut self) -> Self {
        self.child = true;
        self
    }

    fn content(&self) -> String {
        self.text.iter().collect()
    }

    fn speak_char(&self, c: Option<char>, ui: &mut UiContext<'_>) {
        let text = match c {
            None => ui.lang.get("ui.blank"),
            Some(_) if self.password => ui.lang.get("ui.star"),
            Some(c) => char_name(c, ui),
        };
        ui.speech.speak(text);
    }

    fn speak_all(&self, ui: &mut UiContext<'_>) {
        let text = if self.text.is_empty() {
            ui.lang.get("ui.blank")
        } else if self.password {
            ui.lang.plural("ui.characters", self.text.len() as i64)
        } else {
            self.content()
        };
        ui.speech.speak(text);
    }

    fn word_left(&self) -> usize {
        let mut i = self.cursor;
        while i > 0 && self.text[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !self.text[i - 1].is_whitespace() {
            i -= 1;
        }
        i
    }

    fn word_right(&self) -> usize {
        let mut i = self.cursor;
        while i < self.text.len() && !self.text[i].is_whitespace() {
            i += 1;
        }
        while i < self.text.len() && self.text[i].is_whitespace() {
            i += 1;
        }
        i
    }
}

/// Spoken name of a character, so punctuation is not silently skipped.
fn char_name(c: char, ui: &UiContext<'_>) -> String {
    let key = match c {
        ' ' => "char.space",
        '.' => "char.dot",
        ',' => "char.comma",
        '!' => "char.exclamation",
        '?' => "char.question",
        '-' => "char.dash",
        '_' => "char.underscore",
        ':' => "char.colon",
        ';' => "char.semicolon",
        '\'' => "char.apostrophe",
        '"' => "char.quote",
        '/' => "char.slash",
        '\\' => "char.backslash",
        '@' => "char.at",
        '#' => "char.number",
        '(' => "char.left_paren",
        ')' => "char.right_paren",
        _ => return c.to_string(),
    };
    ui.lang.get(key)
}

impl Widget for TextInput {
    fn id(&self) -> &str {
        &self.id
    }

    fn open(&mut self, ui: &mut UiContext<'_>) {
        ui.play(UiSound::Open, None);
        let content = if self.text.is_empty() {
            ui.lang.get("ui.blank")
        } else if self.password {
            ui.lang.plural("ui.characters", self.text.len() as i64)
        } else {
            self.content()
        };
        let text = ui.lang.format("ui.edit", &[("label", &self.prompt), ("value", &content)]);
        ui.speech.speak(text);
    }

    fn update(&mut self, ui: &mut UiContext<'_>) -> Status {
        let word = ui.input.modifiers().control;
        if ui.key(Key::Enter) || ui.key(Key::NumpadEnter) {
            if self.text.is_empty() && !self.allow_empty {
                ui.play(UiSound::Edge, None);
                let blank = ui.lang.get("ui.blank");
                ui.speech.speak(blank);
                return Status::Open;
            }
            ui.play(UiSound::Activate, None);
            let text = self.content();
            if self.child {
                return Status::CloseWith(Value::Text(text));
            }
            ui.emit(UiEvent::TextSubmitted { id: self.id.clone(), text });
            return Status::Close;
        }
        if ui.key(Key::Escape) {
            ui.play(UiSound::Close, None);
            if !self.child {
                ui.emit(UiEvent::Closed { id: self.id.clone() });
            }
            return Status::Close;
        }
        if ui.key(Key::Left) {
            if self.cursor == 0 {
                ui.play(UiSound::Edge, None);
            } else {
                self.cursor = if word { self.word_left() } else { self.cursor - 1 };
            }
            self.speak_char(self.text.get(self.cursor).copied(), ui);
        } else if ui.key(Key::Right) {
            if self.cursor >= self.text.len() {
                ui.play(UiSound::Edge, None);
            } else {
                self.cursor = if word { self.word_right() } else { self.cursor + 1 };
            }
            self.speak_char(self.text.get(self.cursor).copied(), ui);
        } else if ui.key(Key::Home) {
            self.cursor = 0;
            self.speak_char(self.text.first().copied(), ui);
        } else if ui.key(Key::End) {
            self.cursor = self.text.len();
            self.speak_char(None, ui);
        } else if ui.key(Key::Up) || ui.key(Key::Down) {
            self.speak_all(ui);
        } else if ui.key(Key::Backspace) {
            if self.cursor == 0 {
                ui.play(UiSound::Edge, None);
            } else {
                self.cursor -= 1;
                let removed = self.text.remove(self.cursor);
                ui.play(UiSound::Typing, None);
                self.speak_char(Some(removed), ui);
            }
        } else if ui.key(Key::Delete) {
            if self.cursor < self.text.len() {
                self.text.remove(self.cursor);
                ui.play(UiSound::Typing, None);
                self.speak_char(self.text.get(self.cursor).copied(), ui);
            } else {
                ui.play(UiSound::Edge, None);
            }
        }

        let typed: Vec<char> = ui.input.text().chars().collect();
        let mut last = None;
        for c in typed {
            if self.numeric && !(c.is_ascii_digit() || c == '-' || c == '.' || c == ',') {
                continue;
            }
            if self.max_length.is_some_and(|m| self.text.len() >= m) {
                ui.play(UiSound::Edge, None);
                break;
            }
            self.text.insert(self.cursor, c);
            self.cursor += 1;
            last = Some(c);
        }
        if last.is_some() {
            ui.play(UiSound::Typing, None);
            self.speak_char(last, ui);
        }
        Status::Open
    }

    fn value(&self, _item: &str) -> Option<Value> {
        Some(Value::Text(self.content()))
    }
}
