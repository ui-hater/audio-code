# wxDragon integration

[wxDragon](https://crates.io/crates/wxdragon) provides Rust bindings to wxWidgets, which
uses native, screen reader friendly controls. audio-code can run a game inside a wxWidgets
window. That window can also hold native dialogs and controls, which is useful for launchers,
level editors and settings windows.

Enable the feature:

```toml
audio-code = { git = "...", features = ["wxdragon"] }
wxdragon = "0.9"   # only needed if you use wx widgets yourself
```

wxDragon builds wxWidgets the first time, which takes a while. See its README for the
requirements (CMake, a C++ compiler, and Ninja on Windows).

## The one-liner

Replace `run` with `run_wx`:

```rust
fn main() -> audio_code::Result<()> {
    Engine::new("My game").run_wx(MainMenu)
}
```

`run_wx_with(|ctx| ...)` is the counterpart of `run_with`. The rest of the engine works the
same: speech, sound, the virtual UI and physics.

## Embedding into your own wx application

`GameHost::attach` adds a game to an existing `Frame`. The game gets a panel that receives
the keyboard and a timer that runs the game loop.

```rust
use audio_code::prelude::*;
use audio_code::wx::GameHost;
use wxdragon::prelude::*;

fn main() {
    let _ = wxdragon::main(|_| {
        let frame = Frame::builder().with_title("Editor").with_size(Size::new(800, 600)).build();
        let host = GameHost::attach(&frame, Engine::new("Editor"), MyScene::default());

        let save = Button::builder(&frame).with_label("Save level").build();
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        sizer.add(host.panel(), 1, SizerFlag::Expand, 0);
        sizer.add(&save, 0, SizerFlag::Expand | SizerFlag::All, 8);
        frame.set_sizer(sizer, true);

        // Native controls can reach into the game.
        let game = host.clone();
        save.on_click(move |_| {
            game.with_context(|ctx| ctx.speech.speak("Level saved"));
            game.focus();   // give the keyboard back to the game
        });

        frame.show(true);
        host.focus();
    });
}
```

- `host.panel()` is the game panel. Put it into your sizers.
- `host.focus()` moves keyboard focus to the game.
- `host.with_context(|ctx| ...)` runs code with the game's `Context`. It returns `None` if
  it is called while the game itself is running a callback.
- When the game calls `ctx.quit()`, the frame closes. When the frame closes, all scenes get
  their `end` call.

## How keys reach the game

The host listens to the frame's *char hook*, which sees every key before wxWidgets uses it
for dialog navigation. Arrows, Tab, Enter and Escape therefore reach the game even when the
panel contains other controls. Typed characters arrive through normal character events, so
text input follows the keyboard layout.
