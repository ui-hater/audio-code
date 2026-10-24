# Virtual UI

Spoken menus, sliders, checkboxes, text fields and dialogs: `ctx.ui` (type `Ui`). Nothing
is drawn. Every widget announces itself through speech, plays short interface sounds and
reads the keyboard directly.

- Widgets form a stack. The top one gets the keyboard; closing it returns to the one
  below, which announces itself again. Sub menus are just another `open`.
- While any widget is open, game input (`ctx.input`, `ctx.pressed`, ...) is blocked.
- A widget opened with `open` announces itself on the next frame.
- Results arrive as `UiEvent`s that exist for one frame. Read them in `update` with the
  helpers (`clicked`, `changed`, `confirmed`, `submitted`, `closed`) or in `Scene::on_ui`.
- Ids are free strings. Events name the menu id and the item id.

```rust
use audio_code::prelude::*;   // Ui, Menu, Message, Confirm, TextInput, Value, UiEvent, UiSound, UiSounds, Widget
use audio_code::ui::{ItemKind, MenuItem, Status, UiContext};   // for custom items and widgets
```

## Opening and closing

### `ctx.ui.open(widget: impl Widget)`

Opens a widget on top of the stack: a `Menu`, `Message`, `Confirm`, `TextInput` or your own.

```rust
ctx.ui.open(
    Menu::new("main", "Main menu")
        .button("play", "Play")
        .button("options", "Options")
        .button("quit", "Quit")
        .closable(false),
);
```

### `ctx.ui.open_boxed(widget: Box<dyn Widget>)`

Opens an already boxed widget, for example one chosen at runtime.

```rust
let widget: Box<dyn Widget> = if ctx.once("first") {
    Box::new(Message::new("hello", "Welcome"))
} else {
    Box::new(Message::new("back", "Welcome back"))
};
ctx.ui.open_boxed(widget);
```

### `ctx.ui.message(id: impl Into<String>, text: impl Into<String>)`

Shows a message. Enter, Space or Escape closes it and emits `UiEvent::Closed`. Multi line
text can be reviewed line by line with Up, Down, Home and End. Shortcut for
`open(Message::new(id, text))`.

```rust
ctx.ui.message("help", "Arrows walk.\nSpace jumps.\nEscape opens the menu.");
if ctx.ui.closed("help") {
    ctx.speak("Good luck!");
}
```

### `ctx.ui.confirm(id: impl Into<String>, question: impl Into<String>)`

Asks a yes/no question. The answer arrives as `UiEvent::Confirmed`, read it with
`confirmed`. Shortcut for `open(Confirm::new(id, question))`.

```rust
ctx.ui.confirm("quit", "Really quit?");
if ctx.ui.confirmed("quit") == Some(true) {
    ctx.quit();
}
```

### `ctx.ui.input(id: impl Into<String>, prompt: impl Into<String>)`

Asks the player to type text. The result arrives as `UiEvent::TextSubmitted`, read it with
`submitted`. Escape cancels with `UiEvent::Closed`. Shortcut for
`open(TextInput::new(id, prompt))`.

```rust
ctx.ui.input("name", "Enter your name");
if let Some(name) = ctx.ui.submitted("name") {
    let text = format!("Hello, {name}");
    ctx.speak(text);
}
```

### `ctx.ui.close()`

Closes the top widget, without sound or event.

```rust
if ctx.ui.clicked("main", "play") {
    ctx.ui.close();
}
```

### `ctx.ui.close_id(id: &str)`

Closes the widget with this id and everything above it.

```rust
ctx.ui.close_id("options");
```

### `ctx.ui.close_all()`

Closes every widget.

```rust
ctx.ui.close_all();
```

### `ctx.ui.is_open() -> bool`

Whether any widget is open.

```rust
if !ctx.ui.is_open() && ctx.pressed(Key::Escape) {
    ctx.ui.open(Menu::new("pause", "Paused").button("resume", "Resume").button("quit", "Quit"));
}
```

### `ctx.ui.contains(id: &str) -> bool`

Whether a widget with this id is anywhere on the stack.

```rust
if !ctx.ui.contains("main") {
    ctx.ui.open(Menu::new("main", "Main menu").button("play", "Play"));
}
```

### `ctx.ui.top() -> Option<&str>`

Id of the widget that has the keyboard.

```rust
let in_options = ctx.ui.top() == Some("options");
```

### `ctx.ui.value(menu: &str, item: &str) -> Option<Value>`

Current value of a menu item while the menu is open (any position in the stack).

```rust
if let Some(v) = ctx.ui.value("options", "volume") {
    let volume = v.as_f32() / 100.0;
}
```

## Reading events

All helpers look at the events of the current frame only.

### `ctx.ui.clicked(menu: &str, item: &str) -> bool`

Whether button `item` in `menu` was activated this frame.

```rust
if ctx.ui.clicked("main", "quit") {
    ctx.quit();
}
```

### `ctx.ui.activated(menu: &str) -> Option<&str>`

Id of the button activated in `menu` this frame. Handy with `match`.

```rust
match ctx.ui.activated("main") {
    Some("play") => ctx.ui.close(),
    Some("quit") => ctx.quit(),
    _ => {}
}
```

### `ctx.ui.changed(menu: &str, item: &str) -> Option<&Value>`

New value of a checkbox, slider, choice or text field if it changed this frame.

```rust
if let Some(v) = ctx.ui.changed("options", "volume") {
    let volume = v.as_f32() / 100.0;
    ctx.audio.set_master_volume(volume);
}
```

### `ctx.ui.closed(id: &str) -> bool`

Whether the widget `id` was closed by the player this frame (Escape in a menu or text
input, any close key on a message). `ctx.ui.close()` does not count.

```rust
if ctx.ui.closed("options") {
    let _ = ctx.storage.save();
}
```

### `ctx.ui.confirmed(id: &str) -> Option<bool>`

The answer of confirm dialog `id` if given this frame: `Some(true)` yes, `Some(false)` no
(also Escape).

```rust
match ctx.ui.confirmed("delete_save") {
    Some(true) => ctx.storage.clear(),
    Some(false) => ctx.speak("Kept"),
    None => {}
}
```

### `ctx.ui.submitted(id: &str) -> Option<&str>`

Text submitted with Enter in text input `id` this frame.

```rust
if let Some(code) = ctx.ui.submitted("cheat") {
    if code == "godmode" {
        ctx.speak("Cheat on");
    }
}
```

### `ctx.ui.events() -> &[UiEvent]`

All UI events of this frame.

```rust
for event in ctx.ui.events() {
    println!("{event:?}");
}
```

## UiEvent

```rust
pub enum UiEvent {
    Activated { menu: String, item: String },               // a menu button was pressed
    Changed { menu: String, item: String, value: Value },   // checkbox, slider, choice or text field changed
    Closed { id: String },                                  // the player closed a widget
    Confirmed { id: String, yes: bool },                    // a Confirm was answered
    TextSubmitted { id: String, text: String },             // Enter in a TextInput
}
```

```rust
for event in ctx.ui.events().to_vec() {
    if let UiEvent::Changed { item, value, .. } = event {
        let text = format!("{item} is now {}", value.to_text());
        ctx.speech.speak_queued(text);
    }
}
```

## Value

The value of a menu item or event.

```rust
pub enum Value {
    Bool(bool),              // checkbox
    Number(f32),             // slider
    Choice(usize, String),   // choice: index and text
    Text(String),            // text field
}
```

### `value.as_bool() -> bool`

`Bool` as is; numbers are `true` when not zero, choices when the index is not 0, text when
not empty.

```rust
if let Some(v) = ctx.ui.changed("options", "music") {
    let on = v.as_bool();
    ctx.audio.set_music_volume(if on { 1.0 } else { 0.0 });
}
```

### `value.as_f32() -> f32`

A number: sliders their value, choices their index, `bool` 0 or 1, text parsed (0 if not a
number).

```rust
let v = Value::Number(80.0);
assert_eq!(v.as_f32(), 80.0);
```

### `value.as_index() -> usize`

A choice index (or the number rounded, at least 0).

```rust
let difficulty = ctx.ui.value("options", "difficulty").map_or(1, |v| v.as_index());
```

### `value.as_str() -> &str`

Choice or text content; `"on"` / `"off"` for `bool`; `""` for numbers.

```rust
let v = Value::Choice(2, "Hard".into());
assert_eq!(v.as_str(), "Hard");
```

### `value.to_text() -> String`

The value formatted for speaking (numbers without needless decimals: `80`, `0.25`).

```rust
assert_eq!(Value::Number(80.0).to_text(), "80");
```

## Menu

A vertical list of items, navigated with the keyboard:

| Key | Action |
|-----|--------|
| Up / Down (Tab = Down) | previous / next item |
| Home / End | first / last item |
| Enter / Space | press button, toggle checkbox, next choice, edit text field |
| Left / Right | adjust slider or choice, toggle checkbox |
| Page Up / Page Down | slider ±10 steps; on other items first / last item |
| Letters and digits | jump to the next item starting with that character |
| Escape | close (if closable), emits `UiEvent::Closed` |

When it opens it speaks `"<title>. <item>"`, then each focused item, such as
`"Volume, 80"`, `"Music, checked"`, `"Name, edit, Hero"`.

### `Menu::new(id: impl Into<String>, title: impl Into<String>) -> Menu`

An empty menu. `title` is spoken when it opens (may be `""`).

```rust
let menu = Menu::new("main", "Main menu");
```

### `menu.button(id: impl Into<String>, label: impl Into<String>) -> Menu`

Adds a button. Enter or Space emits `UiEvent::Activated`.

```rust
let menu = Menu::new("main", "Main menu").button("play", "Play").button("quit", "Quit");
```

### `menu.checkbox(id: impl Into<String>, label: impl Into<String>, checked: bool) -> Menu`

Adds an on/off toggle. Value: `Value::Bool`.

```rust
let menu = Menu::new("options", "Options").checkbox("music", "Music", true);
```

### `menu.slider(id, label, value: f32, min: f32, max: f32, step: f32) -> Menu`

Adds a number slider (`id` and `label` are `impl Into<String>`). Starts at `value`
(clamped), goes from `min` to `max`, Left/Right change it by `step`, Page Up/Down by ten
steps. Value: `Value::Number`.

```rust
let menu = Menu::new("options", "Options").slider("volume", "Volume", 80.0, 0.0, 100.0, 5.0);
```

### `menu.choice(id, label, options: &[&str], selected: usize) -> Menu`

Adds a choice between options, cycled with Left/Right or Enter. `selected` is the start
index (0 = first). Value: `Value::Choice(index, text)`.

```rust
let menu = Menu::new("options", "Options").choice("difficulty", "Difficulty", &["Easy", "Normal", "Hard"], 1);
```

### `menu.text_field(id, label, value: impl Into<String>) -> Menu`

Adds an editable text field with a start value. Enter opens a text input; Enter there
saves and emits `UiEvent::Changed` with `Value::Text`, Escape cancels.

```rust
let menu = Menu::new("profile", "Profile").text_field("name", "Player name", "Hero");
```

### `menu.password_field(id, label) -> Menu`

Adds an empty text field whose content is never spoken (only its length).

```rust
let menu = Menu::new("login", "Login").text_field("user", "User", "").password_field("pass", "Password");
```

### `menu.label(text: impl Into<String>) -> Menu`

Adds a line of information that cannot be activated (id `""`).

```rust
let best = ctx.storage.get_or("highscore", 0);
let menu = Menu::new("main", "Main menu").label(format!("High score {best}")).button("play", "Play");
```

### `menu.item(item: MenuItem) -> Menu`

Adds any item, built by hand.

```rust
use audio_code::ui::{ItemKind, MenuItem};
let menu = Menu::new("m", "").item(MenuItem { id: "ok".into(), label: "OK".into(), kind: ItemKind::Button });
```

### `menu.wrap(wrap: bool) -> Menu`

Jump from the last item to the first and back. Default `false` (stops at the edge with the
edge sound).

```rust
let menu = Menu::new("main", "Main menu").button("a", "A").button("b", "B").wrap(true);
```

### `menu.closable(closable: bool) -> Menu`

Whether Escape closes the menu. Default `true`. A main menu usually sets `false`.

```rust
let menu = Menu::new("main", "Main menu").button("quit", "Quit").closable(false);
```

### `menu.positions(positions: bool) -> Menu`

Speak `"2 of 5"` after each item. Default `false`.

```rust
let menu = Menu::new("levels", "Choose a level").button("1", "Forest").button("2", "Cave").positions(true);
```

### `menu.close_on_activate(close: bool) -> Menu`

Close the menu automatically after a button was pressed. Default `false`. The
`Activated` event is still emitted.

```rust
let menu = Menu::new("pick", "Pick a weapon").button("sword", "Sword").button("bow", "Bow").close_on_activate(true);
```

### `menu.focus(id: &str) -> Menu`

Starts with the item `id` focused. Call it after adding the items.

```rust
let menu = Menu::new("pause", "Paused").button("resume", "Resume").button("quit", "Quit").focus("resume");
```

### `menu.music(name: impl Into<String>) -> Menu`

Plays music in a loop while the menu is open. Submenus and dialogs on top keep it playing
unless they have their own music. When the menu closes, the music from before comes back,
or the music fades out.

```rust
let menu = Menu::new("main", "Main menu").button("play", "Play").music("music/menu");
```

### `menu.sounds(sounds: UiSounds) -> Menu`

Uses this sound set instead of the global one for this menu.

```rust
let menu = Menu::new("shop", "Shop").button("buy", "Buy").sounds(UiSounds::from_folder("ui/shop"));
```

### `menu.sound(kind: UiSound, name: Option<&str>) -> Menu`

Changes one sound for this menu only; `None` silences it. Other sounds stay.

```rust
let menu = Menu::new("main", "Main menu").button("play", "Play").sound(UiSound::Activate, Some("ui/start"));
```

### `menu.items() -> &[MenuItem]`

The items of the menu.

```rust
let menu = Menu::new("m", "").button("a", "A").button("b", "B");
assert_eq!(menu.items().len(), 2);
```

### `menu.current() -> Option<&MenuItem>`

The focused item.

```rust
let menu = Menu::new("m", "").button("a", "A");
assert_eq!(menu.current().map(|i| i.id.as_str()), Some("a"));
```

## MenuItem and ItemKind

```rust
pub struct MenuItem {
    pub id: String,       // used in events
    pub label: String,    // spoken
    pub kind: ItemKind,
}

pub enum ItemKind {
    Button,
    Checkbox(bool),
    Slider { value: f32, min: f32, max: f32, step: f32 },
    Choice { options: Vec<String>, selected: usize },
    Text { value: String, password: bool },
    Label,
}
```

### `item.value() -> Option<Value>`

The item's value, `None` for buttons and labels.

```rust
use audio_code::ui::{ItemKind, MenuItem};
let item = MenuItem { id: "music".into(), label: "Music".into(), kind: ItemKind::Checkbox(true) };
assert_eq!(item.value(), Some(Value::Bool(true)));
```

### `item.describe() -> String`

What is spoken for the item in English, for example `"Music, checked"`. Menus use
`describe_in` with the current language.

```rust
use audio_code::ui::{ItemKind, MenuItem};
let item = MenuItem { id: "music".into(), label: "Music".into(), kind: ItemKind::Checkbox(true) };
assert_eq!(item.describe(), "Music, checked");
```

### `item.describe_in(lang: &Lang) -> String`

What is spoken for the item in the current language of `lang`.

```rust
use audio_code::ui::{ItemKind, MenuItem};
let item = MenuItem { id: "music".into(), label: "Musik".into(), kind: ItemKind::Checkbox(true) };
let mut lang = Lang::new();
lang.set("de");
assert_eq!(item.describe_in(&lang), "Musik, aktiviert");
```

## Message

A text the player reads and dismisses. Enter, Space or Escape close it
(`UiEvent::Closed`); Up, Down, Home, End read it line by line.

### `Message::new(id: impl Into<String>, text: impl Into<String>) -> Message`

```rust
ctx.ui.open(Message::new("story", "Long ago...\nThe king vanished.\nYou must find him."));
```

## Confirm

A yes/no question. Arrows or Tab switch the answer, Enter or Space confirm, Escape answers
no, and the first letter of an answer picks it directly. Emits `UiEvent::Confirmed`.

### `Confirm::new(id: impl Into<String>, question: impl Into<String>) -> Confirm`

A question with the answers "Yes" and "No" in the current language (`ui.yes` / `ui.no`,
"Ja" / "Nein" in German), "Yes" focused.

```rust
ctx.ui.open(Confirm::new("restart", "Restart the level?"));
```

### `confirm.labels(yes: impl Into<String>, no: impl Into<String>) -> Confirm`

Changes the answer texts. Without it, the answers follow the current language.

```rust
ctx.ui.open(Confirm::new("quit", "Really quit?").labels("Sure", "Not yet"));
```

### `confirm.default_no() -> Confirm`

Focuses "No" first, for dangerous actions.

```rust
ctx.ui.open(Confirm::new("delete", "Delete your save?").default_no());
```

## TextInput

A single line text field.

| Key | Action |
|-----|--------|
| typing | inserts and speaks the character |
| Left / Right | moves the cursor and speaks the character |
| Control + Left / Right | moves by word |
| Home / End | start / end |
| Backspace / Delete | deletes and speaks the character |
| Up / Down | speaks the whole text |
| Enter | submits (`UiEvent::TextSubmitted`) |
| Escape | cancels (`UiEvent::Closed`) |

### `TextInput::new(id: impl Into<String>, prompt: impl Into<String>) -> TextInput`

An empty field with a spoken prompt.

```rust
ctx.ui.open(TextInput::new("name", "Your name"));
```

### `input.initial(text: impl Into<String>) -> TextInput`

Pre-fills the field, cursor at the end.

```rust
ctx.ui.open(TextInput::new("name", "Your name").initial("Hero"));
```

### `input.password() -> TextInput`

Speaks "star" instead of the typed characters.

```rust
ctx.ui.open(TextInput::new("pin", "PIN").password());
```

### `input.max_length(max: usize) -> TextInput`

Limits the number of characters.

```rust
ctx.ui.open(TextInput::new("initials", "Your initials").max_length(3));
```

### `input.numeric() -> TextInput`

Accepts only digits, minus and decimal point.

```rust
ctx.ui.open(TextInput::new("seed", "Seed").numeric());
if let Some(text) = ctx.ui.submitted("seed") {
    let seed: u64 = text.parse().unwrap_or(0);
}
```

### `input.required() -> TextInput`

Refuses to submit an empty text. By default empty is allowed.

```rust
ctx.ui.open(TextInput::new("name", "Your name").required());
```

## Interface sounds

Widgets play a sound at eight moments (`UiSound`). Each moment maps to a sound name (a
file or a registered sound) in a `UiSounds` set, or to silence. The default set is
`UiSounds::builtin()`: small generated clicks that need no files (typing is silent). UI
sounds play in `Group::Ui`.

### `UiSound`

```rust
pub enum UiSound {
    Open,       // a widget opened
    Close,      // the player closed a widget
    Navigate,   // focus moved to another item
    Wrap,       // focus wrapped around (falls back to Navigate)
    Activate,   // a button was pressed
    Edge,       // focus hit the first or last item
    Change,     // a value changed
    Typing,     // a character was typed or deleted (silent by default)
}
```

`UiSound::ALL` lists all eight. `kind.name()` gives the lower case name (`"navigate"`).

```rust
for kind in UiSound::ALL {
    println!("{}", kind.name());
}
```

### `ctx.ui.set_sound(kind: UiSound, name: Option<&str>)`

Uses the sound `name` for one moment in all widgets; `None` silences it.

```rust
ctx.ui.set_sound(UiSound::Navigate, Some("ui/click"));   // sounds/ui/click.ogg
ctx.ui.set_sound(UiSound::Edge, None);
```

### `ctx.ui.set_sound_data(kind: UiSound, data: impl Into<SoundData>)`

Uses a sound made in code, typically a `Synth`, for one moment.

```rust
ctx.ui.set_sound_data(UiSound::Navigate, Synth::triangle(1800.0).length(0.02).decay(150.0));
ctx.ui.set_sound_data(UiSound::Typing, Synth::noise().length(0.01).volume(0.2));
```

### `ctx.ui.set_sounds(sounds: UiSounds)`

Replaces the whole global sound set.

```rust
ctx.ui.set_sounds(UiSounds::from_folder("ui"));
ctx.ui.set_sounds(UiSounds::none());   // silent UI
```

### `ctx.ui.sounds() -> &UiSounds`

The global sound set.

```rust
let navigate = ctx.ui.sounds().get(UiSound::Navigate).map(String::from);
```

### `UiSounds::builtin() -> UiSounds`

The generated default sounds (`UiSounds::default()` is the same).

```rust
let sounds = UiSounds::builtin();
```

### `UiSounds::none() -> UiSounds`

No sounds at all.

```rust
let silent = UiSounds::none();
```

### `UiSounds::from_folder(folder: &str) -> UiSounds`

Sounds named after the moments inside `folder`: `open`, `close`, `navigate`, `wrap`,
`activate`, `edge`, `change`, `typing` (any supported extension, inside the sound folder).
Missing files warn once and stay silent; use `without` for moments you have no file for.

```rust
// sounds/ui/open.ogg, sounds/ui/navigate.wav, ...
let sounds = UiSounds::from_folder("ui").without(UiSound::Typing);
```

### `sounds.with(kind: UiSound, name: impl Into<String>) -> UiSounds`

A copy that plays `name` for `kind`.

```rust
let sounds = UiSounds::builtin().with(UiSound::Navigate, "ui/my_click");
```

### `sounds.without(kind: UiSound) -> UiSounds`

A copy that is silent for `kind`.

```rust
let sounds = UiSounds::builtin().without(UiSound::Edge);
```

### `sounds.set(kind: UiSound, name: Option<String>)`

Changes one moment in place.

```rust
let mut sounds = UiSounds::builtin();
sounds.set(UiSound::Open, Some("ui/whoosh".into()));
```

### `sounds.get(kind: UiSound) -> Option<&str>`

The sound name for a moment (`Wrap` falls back to `Navigate`).

```rust
let sounds = UiSounds::builtin();
assert!(sounds.get(UiSound::Typing).is_none());
```

`UiSounds` also has one public `Option<String>` field per moment: `open`, `close`,
`navigate`, `wrap`, `activate`, `edge`, `change`, `typing`.

## Custom widgets

Implement `Widget` to make your own UI element. It gets a `UiContext` with unblocked input,
speech and audio, and returns a `Status` from `update`.

```rust
pub trait Widget: 'static {
    fn id(&self) -> &str;
    fn open(&mut self, ui: &mut UiContext<'_>);
    fn update(&mut self, ui: &mut UiContext<'_>) -> Status;
    fn resume(&mut self, value: Option<Value>, ui: &mut UiContext<'_>) { self.open(ui) }
    fn value(&self, item: &str) -> Option<Value> { None }
    fn music(&self) -> Option<&str> { None }
}

pub enum Status {
    Open,                      // stay open (default)
    Close,                     // close this widget
    CloseWith(Value),          // close and hand a value to the widget below (its resume)
    Push(Box<dyn Widget>),     // open another widget on top
}
```

| Method | Called |
|--------|--------|
| `id` | for events and `ctx.ui.contains` / `close_id` / `value` |
| `open` | once, on the first frame on top of the stack: announce yourself |
| `update` | every frame while on top |
| `resume` | when on top again after the widget above closed; `value` is what it passed with `CloseWith` (default: calls `open`) |
| `value` | by `ctx.ui.value(id, item)` |
| `music` | each frame: music to play while this widget is on the stack |

`UiContext` fields and methods:

| Item | Meaning |
|------|---------|
| `ui.input: RawInput` | keyboard, never blocked |
| `ui.speech: &mut Speech` | speech output |
| `ui.audio: &mut Audio` | audio output |
| `ui.lang: &Lang` | translations for texts the widget speaks itself ([lang.md](lang.md)) |
| `ui.key(key) -> bool` | key went down or repeats |
| `ui.emit(event: UiEvent)` | reports an event to the game |
| `ui.play(sound: UiSound, custom: Option<&UiSounds>)` | plays an interface sound from `custom` or the global set |
| `ui.play_named(name: &str)` | plays any sound in the UI group |

```rust
use audio_code::ui::{Status, UiContext};

/// Counts up with Right, down with Left, Enter submits the number.
struct Counter {
    value: i32,
}

impl Widget for Counter {
    fn id(&self) -> &str {
        "counter"
    }

    fn open(&mut self, ui: &mut UiContext<'_>) {
        ui.play(UiSound::Open, None);
        ui.speech.speak(format!("Counter, {}", self.value));
    }

    fn update(&mut self, ui: &mut UiContext<'_>) -> Status {
        if ui.key(Key::Right) {
            self.value += 1;
            ui.play(UiSound::Change, None);
            ui.speech.speak(self.value.to_string());
        } else if ui.key(Key::Left) {
            self.value -= 1;
            ui.play(UiSound::Change, None);
            ui.speech.speak(self.value.to_string());
        } else if ui.key(Key::Enter) {
            ui.emit(UiEvent::TextSubmitted { id: "counter".into(), text: self.value.to_string() });
            return Status::Close;
        } else if ui.key(Key::Escape) {
            ui.play(UiSound::Close, None);
            return Status::Close;
        }
        Status::Open
    }
}

ctx.ui.open(Counter { value: 0 });
```
