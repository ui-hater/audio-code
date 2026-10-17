//! Running a game inside a wxWidgets frame with native controls next to it.
//!
//! Run with `cargo run --example wx_game --features wxdragon`.

use audio_code::prelude::*;
use audio_code::wx::GameHost;
use wxdragon::prelude::*;

struct Counter {
    count: u32,
}

impl Scene for Counter {
    fn start(&mut self, ctx: &mut Context) {
        ctx.speech.speak("Press space to count, escape to quit. The window also has a native button.");
    }

    fn update(&mut self, ctx: &mut Context) {
        if ctx.input.pressed(Key::Space) {
            self.count += 1;
            ctx.audio.play_tone(440.0 + self.count as f32 * 20.0, 0.1);
            ctx.speech.speak(self.count.to_string());
        }
        if ctx.input.pressed(Key::Escape) {
            ctx.quit();
        }
    }
}

fn main() {
    // The one-liner: exactly like Engine::run, but inside a native wxWidgets frame.
    //     Engine::new("Counter").run_wx(Counter { count: 0 })
    //
    // Below is the long form, which embeds the game into your own wxDragon UI.
    let _ = wxdragon::main(|_| {
        let frame = Frame::builder().with_title("Counter").with_size(Size::new(400, 300)).build();
        let host = GameHost::attach(&frame, Engine::new("Counter"), Counter { count: 0 });

        let button = Button::builder(&frame).with_label("Say hello from wxWidgets").build();
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        sizer.add(host.panel(), 1, SizerFlag::Expand, 0);
        sizer.add(&button, 0, SizerFlag::Expand | SizerFlag::All, 8);
        frame.set_sizer(sizer, true);

        let game = host.clone();
        button.on_click(move |_| {
            game.with_context(|ctx| ctx.speech.speak("Hello from a native button"));
            game.focus();
        });

        frame.show(true);
        host.focus();
    });
}
