//! Audio menus.

use super::dialog::TextInput;
use super::{Status, UiContext, UiEvent, UiSound, UiSounds, Value, Widget, format_number};
use crate::lang::Lang;
use crate::input::Key;

/// What kind of item a [`MenuItem`] is.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemKind {
    /// Pressing Enter emits [`UiEvent::Activated`].
    Button,
    /// On/off toggle (Enter, Space, Left or Right).
    Checkbox(bool),
    /// Number adjusted with Left/Right (one step) and Page Up/Down (ten steps).
    Slider {
        /// Current value.
        value: f32,
        /// Lowest value.
        min: f32,
        /// Highest value.
        max: f32,
        /// Change per key press.
        step: f32,
    },
    /// One of several options, cycled with Left/Right or Enter.
    Choice {
        /// Option texts.
        options: Vec<String>,
        /// Selected index.
        selected: usize,
    },
    /// Editable text; Enter opens a text input.
    Text {
        /// Current text.
        value: String,
        /// Speak stars instead of the content.
        password: bool,
    },
    /// Information only, cannot be activated.
    Label,
}

/// One entry of a [`Menu`].
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    /// Identifier used in events.
    pub id: String,
    /// Spoken label.
    pub label: String,
    /// Kind and value.
    pub kind: ItemKind,
}

impl MenuItem {
    /// The current value, `None` for buttons and labels.
    pub fn value(&self) -> Option<Value> {
        match &self.kind {
            ItemKind::Checkbox(b) => Some(Value::Bool(*b)),
            ItemKind::Slider { value, .. } => Some(Value::Number(*value)),
            ItemKind::Choice { options, selected } => {
                Some(Value::Choice(*selected, options.get(*selected).cloned().unwrap_or_default()))
            }
            ItemKind::Text { value, .. } => Some(Value::Text(value.clone())),
            ItemKind::Button | ItemKind::Label => None,
        }
    }

    /// What a screen reader would say for this item, in English.
    pub fn describe(&self) -> String {
        self.describe_in(&Lang::new())
    }

    /// What a screen reader would say for this item, in the current language of `lang`.
    pub fn describe_in(&self, lang: &Lang) -> String {
        match &self.kind {
            ItemKind::Button | ItemKind::Label => self.label.clone(),
            ItemKind::Checkbox(b) => format!("{}, {}", self.label, lang.get(if *b { "ui.checked" } else { "ui.not_checked" })),
            ItemKind::Slider { value, .. } => format!("{}, {}", self.label, format_number(*value)),
            ItemKind::Choice { options, selected } => {
                format!("{}, {}", self.label, options.get(*selected).map_or("", String::as_str))
            }
            ItemKind::Text { value, password } => {
                let shown = if value.is_empty() {
                    lang.get("ui.blank")
                } else if *password {
                    lang.plural("ui.characters", value.chars().count() as i64)
                } else {
                    value.clone()
                };
                lang.format("ui.edit", &[("label", &self.label), ("value", &shown)])
            }
        }
    }
}

/// A vertical list of items navigated with the arrow keys.
///
/// | Key | Action |
/// |-----|--------|
/// | Up / Down | previous / next item |
/// | Home / End | first / last item |
/// | Enter / Space | press button, toggle checkbox, next choice, edit text |
/// | Left / Right | adjust slider or choice, toggle checkbox |
/// | Page Up / Page Down | adjust slider by ten steps |
/// | Letters | jump to the next item starting with that letter |
/// | Escape | close (if [`Menu::closable`]) |
///
/// ```no_run
/// # use audio_code::prelude::*;
/// let menu = Menu::new("options", "Options")
///     .slider("volume", "Volume", 80.0, 0.0, 100.0, 5.0)
///     .choice("difficulty", "Difficulty", &["Easy", "Normal", "Hard"], 1)
///     .checkbox("subtitles", "Speak hints", true)
///     .text_field("name", "Player name", "Hero")
///     .button("back", "Back")
///     .music("menu_theme");
/// ```
#[derive(Debug, Clone)]
pub struct Menu {
    id: String,
    title: String,
    items: Vec<MenuItem>,
    index: usize,
    wrap: bool,
    closable: bool,
    positions: bool,
    close_on_activate: bool,
    sounds: Option<UiSounds>,
    sound_overrides: Vec<(UiSound, Option<String>)>,
    music: Option<String>,
    editing: Option<usize>,
}

impl Menu {
    /// Creates an empty menu. `title` is spoken when it opens (may be empty).
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            items: Vec::new(),
            index: 0,
            wrap: false,
            closable: true,
            positions: false,
            close_on_activate: false,
            sounds: None,
            sound_overrides: Vec::new(),
            music: None,
            editing: None,
        }
    }

    /// Adds any item.
    pub fn item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }

    fn push(self, id: impl Into<String>, label: impl Into<String>, kind: ItemKind) -> Self {
        self.item(MenuItem { id: id.into(), label: label.into(), kind })
    }

    /// Adds a button.
    pub fn button(self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.push(id, label, ItemKind::Button)
    }

    /// Adds a checkbox.
    pub fn checkbox(self, id: impl Into<String>, label: impl Into<String>, checked: bool) -> Self {
        self.push(id, label, ItemKind::Checkbox(checked))
    }

    /// Adds a slider. `value` is the start value, `min` and `max` the range, and `step`
    /// how much Left and Right change it (Page Up and Page Down change it by ten steps).
    /// `slider("volume", "Volume", 80.0, 0.0, 100.0, 5.0)` starts at 80 and goes from 0 to
    /// 100 in steps of 5. Read the value with `Value::as_f32`.
    pub fn slider(self, id: impl Into<String>, label: impl Into<String>, value: f32, min: f32, max: f32, step: f32) -> Self {
        let value = value.clamp(min, max);
        self.push(id, label, ItemKind::Slider { value, min, max, step: step.abs().max(f32::EPSILON) })
    }

    /// Adds a choice between several options. `selected` is the index of the option chosen
    /// at the start (0 = the first one).
    pub fn choice(self, id: impl Into<String>, label: impl Into<String>, options: &[&str], selected: usize) -> Self {
        let options: Vec<String> = options.iter().map(|s| s.to_string()).collect();
        let selected = selected.min(options.len().saturating_sub(1));
        self.push(id, label, ItemKind::Choice { options, selected })
    }

    /// Adds an editable text field.
    pub fn text_field(self, id: impl Into<String>, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.push(id, label, ItemKind::Text { value: value.into(), password: false })
    }

    /// Adds a password field (content is never spoken).
    pub fn password_field(self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.push(id, label, ItemKind::Text { value: String::new(), password: true })
    }

    /// Adds a line of information that cannot be activated.
    pub fn label(self, text: impl Into<String>) -> Self {
        let text = text.into();
        self.push("", text, ItemKind::Label)
    }

    /// Jump from the last to the first item and back (default `false`).
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// Allow closing with Escape (default `true`). A main menu often sets this to `false`.
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// Speak "2 of 5" after each item (default `false`).
    pub fn positions(mut self, positions: bool) -> Self {
        self.positions = positions;
        self
    }

    /// Close the menu automatically after a button was pressed (default `false`).
    pub fn close_on_activate(mut self, close: bool) -> Self {
        self.close_on_activate = close;
        self
    }

    /// Use these sounds instead of the global ones for this menu.
    pub fn sounds(mut self, sounds: UiSounds) -> Self {
        self.sounds = Some(sounds);
        self
    }

    /// Changes one sound for this menu only, e.g. a special click for the main menu.
    /// `None` makes it silent. All other sounds stay as they are.
    pub fn sound(mut self, kind: UiSound, name: Option<&str>) -> Self {
        self.sound_overrides.push((kind, name.map(String::from)));
        self
    }

    /// Plays the music `name` in a loop while this menu is open (a file or registered sound).
    /// Submenus and dialogs on top keep it playing unless they have music of their own.
    /// When the menu closes, the music that played before comes back, or the music fades out.
    pub fn music(mut self, name: impl Into<String>) -> Self {
        self.music = Some(name.into());
        self
    }

    fn play(&self, ui: &mut UiContext<'_>, kind: UiSound) {
        match self.sound_overrides.iter().rev().find(|(k, _)| *k == kind) {
            Some((_, Some(name))) => ui.play_named(name),
            Some((_, None)) => {}
            None => ui.play(kind, self.sounds.as_ref()),
        }
    }

    /// Start with the item `id` focused.
    pub fn focus(mut self, id: &str) -> Self {
        if let Some(i) = self.items.iter().position(|it| it.id == id) {
            self.index = i;
        }
        self
    }

    /// The items of this menu.
    pub fn items(&self) -> &[MenuItem] {
        &self.items
    }

    /// The focused item.
    pub fn current(&self) -> Option<&MenuItem> {
        self.items.get(self.index)
    }

    fn announce(&self, ui: &mut UiContext<'_>, with_title: bool) {
        let mut text = String::new();
        if with_title && !self.title.is_empty() {
            text.push_str(&self.title);
            text.push_str(". ");
        }
        match self.current() {
            Some(item) => {
                text.push_str(&item.describe_in(ui.lang));
                if self.positions {
                    let (index, count) = (self.index + 1, self.items.len());
                    text.push_str(", ");
                    text.push_str(&ui.lang.format("ui.position", &[("index", &index), ("count", &count)]));
                }
            }
            None => text.push_str(&ui.lang.get("ui.empty")),
        }
        ui.speech.speak(text);
    }

    fn speak_value(&self, ui: &mut UiContext<'_>) {
        if let Some(item) = self.current() {
            match item.value() {
                Some(Value::Bool(b)) => ui.speech.speak(ui.lang.get(if b { "ui.checked" } else { "ui.not_checked" })),
                Some(v) => ui.speech.speak(v.to_text()),
                None => {}
            }
        }
    }

    fn move_to(&mut self, index: usize, ui: &mut UiContext<'_>) {
        self.move_with_sound(index, UiSound::Navigate, ui);
    }

    fn move_with_sound(&mut self, index: usize, sound: UiSound, ui: &mut UiContext<'_>) {
        if index == self.index {
            self.play(ui, UiSound::Edge);
        } else {
            self.play(ui, sound);
            self.index = index;
        }
        self.announce(ui, false);
    }

    fn changed(&mut self, ui: &mut UiContext<'_>) {
        self.play(ui, UiSound::Change);
        self.speak_value(ui);
        if let Some(item) = self.current()
            && let Some(value) = item.value()
        {
            let event = UiEvent::Changed { menu: self.id.clone(), item: item.id.clone(), value };
            ui.emit(event);
        }
    }

    /// Adjusts the focused item by `steps` (negative = left). Returns whether anything changed.
    fn adjust(&mut self, steps: i32) -> bool {
        let Some(item) = self.items.get_mut(self.index) else {
            return false;
        };
        match &mut item.kind {
            ItemKind::Checkbox(b) => {
                *b = !*b;
                true
            }
            ItemKind::Slider { value, min, max, step } => {
                let new = (*value + *step * steps as f32).clamp(*min, *max);
                // Snap to the step grid to avoid 0.30000000004.
                let new = ((new - *min) / *step).round() * *step + *min;
                let new = new.clamp(*min, *max);
                let changed = new != *value;
                *value = new;
                changed
            }
            ItemKind::Choice { options, selected } => {
                if options.is_empty() {
                    return false;
                }
                let n = options.len() as i32;
                let new = (*selected as i32 + steps).rem_euclid(n) as usize;
                let changed = new != *selected;
                *selected = new;
                changed
            }
            _ => false,
        }
    }

    fn jump_to_letter(&mut self, c: char, ui: &mut UiContext<'_>) {
        let n = self.items.len();
        let c = c.to_lowercase().next().unwrap_or(c);
        let found = (1..=n).map(|off| (self.index + off) % n).find(|&i| {
            self.items[i].label.chars().next().and_then(|f| f.to_lowercase().next()) == Some(c)
        });
        match found {
            Some(i) if i != self.index => {
                self.index = i;
                self.play(ui, UiSound::Navigate);
                self.announce(ui, false);
            }
            Some(_) => self.announce(ui, false),
            None => self.play(ui, UiSound::Edge),
        }
    }
}

impl Widget for Menu {
    fn id(&self) -> &str {
        &self.id
    }

    fn open(&mut self, ui: &mut UiContext<'_>) {
        self.index = self.index.min(self.items.len().saturating_sub(1));
        self.play(ui, UiSound::Open);
        self.announce(ui, true);
    }

    fn resume(&mut self, value: Option<Value>, ui: &mut UiContext<'_>) {
        if let (Some(i), Some(Value::Text(text))) = (self.editing.take(), value)
            && let Some(item) = self.items.get_mut(i)
            && let ItemKind::Text { value, .. } = &mut item.kind
        {
            *value = text.clone();
            let event = UiEvent::Changed { menu: self.id.clone(), item: item.id.clone(), value: Value::Text(text) };
            ui.emit(event);
        }
        self.announce(ui, false);
    }

    fn update(&mut self, ui: &mut UiContext<'_>) -> Status {
        if self.items.is_empty() {
            if self.closable && ui.key(Key::Escape) {
                ui.emit(UiEvent::Closed { id: self.id.clone() });
                return Status::Close;
            }
            return Status::Open;
        }
        let last = self.items.len() - 1;

        if ui.key(Key::Up) {
            if self.index > 0 {
                self.move_to(self.index - 1, ui);
            } else if self.wrap && last > 0 {
                self.move_with_sound(last, UiSound::Wrap, ui);
            } else {
                self.move_to(0, ui);
            }
        } else if ui.key(Key::Down) || ui.key(Key::Tab) {
            if self.index < last {
                self.move_to(self.index + 1, ui);
            } else if self.wrap && last > 0 {
                self.move_with_sound(0, UiSound::Wrap, ui);
            } else {
                self.move_to(last, ui);
            }
        } else if ui.key(Key::Home) {
            self.move_to(0, ui);
        } else if ui.key(Key::End) {
            self.move_to(last, ui);
        } else if ui.key(Key::Left) || ui.key(Key::Right) || ui.key(Key::PageUp) || ui.key(Key::PageDown) {
            let steps: i32 = if ui.key(Key::Left) {
                -1
            } else if ui.key(Key::Right) {
                1
            } else if ui.key(Key::PageUp) {
                10
            } else {
                -10
            };
            let is_slider = matches!(self.current().map(|i| &i.kind), Some(ItemKind::Slider { .. }));
            let is_paging = steps.abs() == 10;
            if is_paging && !is_slider {
                let to = if steps > 0 { 0 } else { last };
                self.move_to(to, ui);
            } else if self.adjust(steps) {
                self.changed(ui);
            } else if is_slider || matches!(self.current().map(|i| &i.kind), Some(ItemKind::Choice { .. })) {
                self.play(ui, UiSound::Edge);
                self.speak_value(ui);
            }
        } else if ui.key(Key::Enter) || ui.key(Key::NumpadEnter) || ui.input.pressed(Key::Space) {
            let item = &self.items[self.index];
            match &item.kind {
                ItemKind::Button => {
                    self.play(ui, UiSound::Activate);
                    ui.emit(UiEvent::Activated { menu: self.id.clone(), item: item.id.clone() });
                    if self.close_on_activate {
                        return Status::Close;
                    }
                }
                ItemKind::Checkbox(_) | ItemKind::Choice { .. } => {
                    if self.adjust(1) {
                        self.changed(ui);
                    }
                }
                ItemKind::Text { value, password } => {
                    self.play(ui, UiSound::Activate);
                    let mut input = TextInput::new(format!("{}/{}", self.id, item.id), item.label.clone())
                        .initial(value.clone())
                        .child();
                    if *password {
                        input = input.password();
                    }
                    self.editing = Some(self.index);
                    return Status::Push(Box::new(input));
                }
                ItemKind::Slider { .. } | ItemKind::Label => self.announce(ui, false),
            }
        } else if ui.key(Key::Escape) && self.closable {
            self.play(ui, UiSound::Close);
            ui.emit(UiEvent::Closed { id: self.id.clone() });
            return Status::Close;
        } else if let Some(c) = ui.input.text().chars().find(|c| c.is_alphanumeric()) {
            self.jump_to_letter(c, ui);
        }
        Status::Open
    }

    fn value(&self, item: &str) -> Option<Value> {
        self.items.iter().find(|i| i.id == item).and_then(MenuItem::value)
    }

    fn music(&self) -> Option<&str> {
        self.music.as_deref()
    }
}
