//! A whole game in one closure: press space as fast as you can for ten seconds.
//! The high score is saved between runs.
//!
//! Run with `cargo run --example clicker`.

use audio_code::prelude::*;

fn main() -> audio_code::Result<()> {
    let mut score = 0;

    audio_code::run("Clicker", move |ctx| {
        if ctx.once("intro") {
            let best: u32 = ctx.storage.get_or("best", 0);
            ctx.speak(format!("Press space as often as you can in ten seconds. Your best is {best}. Go!"));
            ctx.time.after("end", 10.0);
        }

        if ctx.time.is_scheduled("end") && ctx.pressed(Key::Space) {
            score += 1;
            ctx.audio.play_tone(300.0 + score as f32 * 15.0, 0.05);
        }

        if ctx.time.fired("end") {
            let best: u32 = ctx.storage.get_or("best", 0);
            if score > best {
                ctx.storage.set("best", score);
                ctx.ui.message("result", format!("{score} presses. New record!"));
            } else {
                ctx.ui.message("result", format!("{score} presses. Your best is {best}."));
            }
        }

        if ctx.ui.closed("result") || ctx.pressed(Key::Escape) {
            ctx.quit();
        }
    })
}
