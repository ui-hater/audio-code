# Virtual UI

`ctx.ui` provides a user interface that is spoken instead of drawn. Each element announces
itself through speech, plays short interface sounds, and is controlled from the keyboard.

Widgets live on a **stack**. The top widget receives the keyboard. When it closes, the
widget below speaks its current item again. This makes sub menus easy.

While any widget is open, `ctx.input` reports no keys to your game. This prevents the
Enter that picks a menu item from also making your character jump. Use `ctx.input.raw()`
if you need to see the keys anyway.

## Menus

```rust
ctx.ui.open(
    Menu::new("options", "Options")
        .slider("volume", "Volume", 80.0, 0.0, 100.0, 5.0)
        .choice("difficulty", "Difficulty", &["Easy", "Normal", "Hard"], 1)
        .checkbox("hints", "Speak hints", true)
        .text_field("name", "Player name", "Hero")
        .password_field("password", "Password")
        .label("Changes are saved automatically")
        .button("back", "Back"),
);
```

| Key                 | Action                                                   |
|---------------------|----------------------------------------------------------|
| Up / Down / Tab     | previous / next item                                     |
| Home / End          | first / last item                                        |
| Enter / Space       | press button, toggle checkbox, next choice, edit text    |
| Left / Right        | slider minus/plus one step, previous/next choice, toggle |
| Page Up / Page Down | slider plus/minus ten steps                              |
| letters             | jump to the next item starting with that letter          |
| Escape              | close (unless `.closable(false)`)                        |

Options:

```rust
Menu::new("main", "Main menu")
    .wrap(true)               // jump from last to first item
    .closable(false)          // Escape does nothing (good for main menus)
    .positions(false)         // do not say "2 of 5"
    .close_on_activate(true)  // close after a button is pressed
    .focus("continue")        // start on this item
    .sounds(UiSounds::from_folder("ui/fantasy"));
```

The announcement sounds like `Options. Volume, 80, 1 of 7`.

## Reacting to the UI

There are two ways, and you can mix them.

**Polling in `update`:**

```rust
if ctx.ui.clicked("main", "play") { /* ... */ }
if let Some(item) = ctx.ui.activated("main") { /* any button */ }
if let Some(v) = ctx.ui.changed("options", "volume") {
    ctx.audio.set_master_volume(v.as_f32() / 100.0);
}
if ctx.ui.closed("options") { /* player pressed Escape */ }
let current = ctx.ui.value("options", "hints");  // while the menu is open
```

**The `on_ui` callback of your scene:**

```rust
fn on_ui(&mut self, event: &UiEvent, ctx: &mut Context) {
    match event {
        UiEvent::Activated { menu, item } if menu == "main" && item == "quit" => ctx.quit(),
        UiEvent::Changed { item, value, .. } if item == "volume" => {
            ctx.audio.set_master_volume(value.as_f32() / 100.0)
        }
        _ => {}
    }
}
```

`Value` has `as_bool()`, `as_f32()`, `as_index()`, `as_str()` and `to_text()`.

## Messages

```rust
ctx.ui.message("intro", "Welcome to the dungeon.\nFind the key.\nAvoid the rats.");
if ctx.ui.closed("intro") { /* ... */ }
```

The message is spoken in full. Up/Down/Home/End go through it line by line, and Enter,
Space or Escape close it.

## Yes / no questions

```rust
ctx.ui.confirm("quit", "Do you really want to quit?");
// or with options:
ctx.ui.open(Confirm::new("delete", "Delete save game?").labels("Ja", "Nein").default_no());

match ctx.ui.confirmed("quit") {
    Some(true) => ctx.quit(),
    Some(false) => {}  // also the result of Escape
    None => {}         // not answered this frame
}
```

Arrow keys switch the answer, and Enter confirms it. The first letter of an answer (here
"y" / "n") answers right away.

## Text input

```rust
ctx.ui.input("name", "Enter your name");
// or with options:
ctx.ui.open(TextInput::new("code", "Enter the code").numeric().max_length(4).required());

if let Some(text) = ctx.ui.submitted("name") { /* ... */ }
```

| Key                    | Action                               |
|------------------------|--------------------------------------|
| typing                 | inserts and speaks the character     |
| Left / Right           | moves the cursor, speaks the character |
| Control + Left / Right | moves by word                        |
| Home / End             | start / end                          |
| Backspace / Delete     | deletes, speaks what was deleted     |
| Up / Down              | speaks the whole text                |
| Enter                  | submit                               |
| Escape                 | cancel (`closed` event)              |

`.password()` speaks "star" instead of the characters.

## Interface sounds

By default the UI uses small generated clicks, so no files are needed. To replace them:

```rust
ctx.ui.set_sounds(UiSounds::from_folder("ui"));   // sounds/ui/navigate.ogg, ...
ctx.ui.set_sounds(UiSounds::none());              // silence
ctx.ui.set_sounds(UiSounds { activate: Some("click".into()), ..UiSounds::builtin() });
```

The sound slots are `open`, `close`, `navigate`, `activate`, `edge` (first/last item reached)
and `change` (value changed). UI sounds play in `Group::Ui`, which has its own volume.

## Managing the stack

```rust
ctx.ui.close();            // close the top widget silently
ctx.ui.close_id("pause");  // close "pause" and everything above it
ctx.ui.close_all();
ctx.ui.is_open();
ctx.ui.contains("options");
ctx.ui.top();              // Some("options")
```

## Custom widgets

Implement [`Widget`] to make your own element, for example an inventory grid or a key
binding recorder:

```rust
use audio_code::ui::{Status, UiContext, UiEvent, UiSound, Widget};

struct KeyRecorder;

impl Widget for KeyRecorder {
    fn id(&self) -> &str { "recorder" }

    fn open(&mut self, ui: &mut UiContext<'_>) {
        ui.play(UiSound::Open, None);
        ui.speech.speak("Press the key you want to use.");
    }

    fn update(&mut self, ui: &mut UiContext<'_>) -> Status {
        if let Some(key) = ui.input.keys_pressed().first() {
            ui.speech.speak(format!("{} selected", key.name()));
            ui.emit(UiEvent::Closed { id: "recorder".into() });
            return Status::Close;
        }
        Status::Open
    }
}

ctx.ui.open(KeyRecorder);
```

`Status::Push(Box::new(other_widget))` opens a child widget, and
`Status::CloseWith(value)` closes the widget and passes `value` to the widget below in
`Widget::resume`.

[`Widget`]: ../src/ui/mod.rs
