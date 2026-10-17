//! # audio-code
//!
//! A simple, high level game engine for **audio games**: games played by ear, without
//! graphics. Everything a blind or visually impaired player needs is built in:
//!
//! * **Speech** through the player's screen reader or system voice ([`speech`], via prism)
//! * **Sound**: one-line playback, 3D positioned sounds, music with cross fades ([`audio`])
//! * **Virtual UI**: spoken menus, sliders, checkboxes, text fields and dialogs ([`ui`])
//! * **Physics**: walls, floors, gravity, triggers and ray casts ([`physics`])
//! * **Time**: frame timing, a scheduler, timers, cooldowns and intervals ([`time`])
//! * **Input**: pressed / held / released, custom key repeat, typed text ([`input`])
//! * Optional **wxWidgets** integration through wxDragon (`audio_code::wx`, feature `wxdragon`)
//!
//! ## A complete game in 30 lines
//!
//! ```no_run
//! use audio_code::prelude::*;
//!
//! struct Game {
//!     x: f32,
//! }
//!
//! impl Scene for Game {
//!     fn start(&mut self, ctx: &mut Context) {
//!         ctx.speech.speak("Walk with left and right. Find the beacon!");
//!         ctx.audio.play_looped_at("beacon", vec3(10.0, 0.0, 0.0));
//!     }
//!
//!     fn update(&mut self, ctx: &mut Context) {
//!         if ctx.input.repeating(Key::Left, 0.25, 0.25) {
//!             self.x -= 1.0;
//!         }
//!         if ctx.input.repeating(Key::Right, 0.25, 0.25) {
//!             self.x += 1.0;
//!         }
//!         ctx.audio.set_listener(vec3(self.x, 0.0, 0.0), 0.0);
//!         if self.x == 10.0 {
//!             ctx.speech.speak("You found it!");
//!             ctx.quit();
//!         }
//!     }
//! }
//!
//! fn main() -> audio_code::Result<()> {
//!     Engine::new("Beacon").run(Game { x: 0.0 })
//! }
//! ```
//!
//! Start with [`Engine`], [`Scene`] and [`Context`], then read the module docs. The `docs`
//! folder of the repository contains a longer guide.

#![warn(missing_docs)]

pub mod audio;
mod engine;
mod error;
pub mod input;
pub mod math;
pub mod physics;
pub mod speech;
pub mod time;
pub mod ui;
mod window;
#[cfg(feature = "wxdragon")]
pub mod wx;

pub use engine::{Context, Engine, Runner, Scene};
pub use error::{Error, Result};

/// Everything you normally need: `use audio_code::prelude::*;`
pub mod prelude {
    pub use crate::audio::{Audio, Group, Sound, SoundData};
    pub use crate::engine::{Context, Engine, Scene};
    pub use crate::input::{Input, Key, Modifiers};
    pub use crate::math::{self, Vec2, Vec3, random, vec2, vec3};
    pub use crate::physics::{Body, BodyId, BodyKind, PhysicsEvent, Shape, World};
    pub use crate::speech::Speech;
    pub use crate::time::{Cooldown, Interval, Time, Timer};
    pub use crate::ui::{Confirm, Menu, Message, TextInput, Ui, UiEvent, UiSounds, Value, Widget};
}
