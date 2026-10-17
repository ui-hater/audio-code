//! The smallest possible game: speech, a beep and quitting.
//!
//! Run with `cargo run --example hello`.

use audio_code::prelude::*;

struct Hello;

impl Scene for Hello {
    fn start(&mut self, ctx: &mut Context) {
        ctx.speech.speak("Hello! Press space to beep, F1 to repeat, escape to quit.");
    }

    fn update(&mut self, ctx: &mut Context) {
        if ctx.input.pressed(Key::Space) {
            // A pitch that rises over time, so every beep sounds different.
            let pitch = 300.0 + (ctx.time.elapsed() as f32 * 50.0) % 900.0;
            ctx.audio.play_tone(pitch.round(), 0.15);
        }
        if ctx.input.pressed(Key::F1) {
            ctx.speech.repeat_last();
        }
        if ctx.input.pressed(Key::Escape) {
            ctx.quit();
        }
    }
}

fn main() -> audio_code::Result<()> {
    Engine::new("Hello audio-code").echo_speech(true).run(Hello)
}
