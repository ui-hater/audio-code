//! Bell Catcher: bells fall somewhere in front of you. Listen where one rings, step under it
//! and catch it before it lands. The faster you are, the faster they fall.
//!
//! * Left / Right: step one lane
//! * S: score and lives
//! * Escape: back to the menu
//!
//! The menu lets players pick a speech voice and rate; both are saved with the best score.
//! All sounds are generated with the synth, so no files are needed.
//! Run with `cargo run --example bell_catcher` and use headphones.

use audio_code::prelude::*;

/// Lanes go from -LANES to LANES, 0 is the middle.
const LANES: i32 = 3;
const LANE_WIDTH: f32 = 1.5;
const LIVES: u32 = 3;
/// Seconds a bell needs to land, at the start and at most speed.
const FIRST_FALL: f32 = 3.0;
const FASTEST_FALL: f32 = 1.0;

fn main() -> audio_code::Result<()> {
    Engine::new("Bell Catcher").run_with(|ctx| {
        ctx.audio.register("bell", Synth::sine(1568.0).with(Synth::sine(3136.0).volume(0.3)).length(0.35).decay(9.0).volume(0.6));
        ctx.audio.register("step", Synth::noise().length(0.05).decay(80.0).volume(0.5));
        ctx.audio.register("edge", Synth::sine(110.0).length(0.12).decay(30.0).volume(0.8));
        ctx.audio.register(
            "catch",
            Synth::sine(784.0).length(0.08).then(Synth::sine(1047.0).length(0.08)).then(Synth::sine(1568.0).length(0.2).decay(12.0)).volume(0.4),
        );
        ctx.audio.register("miss", Synth::saw(220.0).sweep_to(80.0).length(0.5).decay(5.0).volume(0.35));

        // Restore the speech settings from the last session.
        if let Some(voice) = ctx.storage.get::<String>("voice") {
            ctx.speech.set_voice(voice);
        }
        let rate: f32 = ctx.storage.get_or("rate", 50.0);
        if rate != 50.0 {
            ctx.speech.set_rate(rate / 100.0);
        }
        Title
    })
}

// ------------------------------------------------------------------------- menu

struct Title;

fn title_menu(ctx: &Context) -> Menu {
    let voice = ctx.speech.voice().map_or("default".to_string(), |v| v.name.clone());
    let best: u32 = ctx.storage.get_or("best", 0);
    Menu::new("title", "Bell Catcher")
        .button("play", "Play")
        .button("voice", format!("Voice: {voice}"))
        .slider("rate", "Speech rate", ctx.storage.get_or("rate", 50.0), 0.0, 100.0, 5.0)
        .button("help", "How to play")
        .label(format!("Best score: {best}"))
        .button("quit", "Quit")
        .closable(false)
        .wrap(true)
}

/// One button per voice, with the current one focused. `None` if the output has no voices.
fn voice_menu(ctx: &Context) -> Option<Menu> {
    let voices = ctx.speech.voices();
    if voices.is_empty() {
        return None;
    }
    let mut menu = Menu::new("voices", "Choose a voice").button("default", "Default voice");
    for (i, voice) in voices.iter().enumerate() {
        menu = menu.button(format!("voice{i}"), voice.to_string());
    }
    if let Some(i) = ctx.speech.voice().and_then(|current| voices.iter().position(|v| v == current)) {
        menu = menu.focus(&format!("voice{i}"));
    }
    Some(menu.button("back", "Back"))
}

/// Reopens the title menu so its labels show the new values, focused on `item`.
fn refresh_title(ctx: &mut Context, item: &str) {
    ctx.ui.close_id("title");
    let menu = title_menu(ctx).focus(item);
    ctx.ui.open(menu);
}

impl Scene for Title {
    fn start(&mut self, ctx: &mut Context) {
        let menu = title_menu(ctx);
        ctx.ui.open(menu);
    }

    fn on_ui(&mut self, event: &UiEvent, ctx: &mut Context) {
        match event {
            UiEvent::Activated { menu, item } if menu == "title" => match item.as_str() {
                "play" => {
                    ctx.ui.close_all();
                    ctx.switch_scene(Game::default());
                }
                "voice" => match voice_menu(ctx) {
                    Some(menu) => ctx.ui.open(menu),
                    None => ctx.ui.message(
                        "no voices",
                        "Your screen reader speaks with its own voice. Change it in the screen reader's settings.",
                    ),
                },
                "help" => ctx.ui.message(
                    "help",
                    "Bells fall in one of seven lanes in front of you. Listen where a bell rings, \
                     step under it with left and right, and catch it before it lands. \
                     The bell rings faster and higher the closer it gets. You have three lives. \
                     Press S for your score, Escape for the menu.",
                ),
                "quit" => ctx.quit(),
                _ => {}
            },
            UiEvent::Activated { menu, item } if menu == "voices" => {
                if item == "back" {
                    ctx.ui.close();
                    return;
                }
                if item == "default" {
                    ctx.speech.reset_voice();
                    ctx.storage.remove("voice");
                } else if let Some(voice) =
                    item.strip_prefix("voice").and_then(|i| i.parse::<usize>().ok()).and_then(|i| ctx.speech.voices().get(i).cloned())
                    && ctx.speech.set_voice(&voice.id)
                {
                    ctx.storage.set("voice", &voice.id);
                }
                ctx.ui.close();
                refresh_title(ctx, "voice");
                let name = ctx.speech.voice().map_or("Default voice".to_string(), |v| v.name.clone());
                ctx.speech.speak_queued(format!("{name}. This is how I sound now."));
            }
            UiEvent::Changed { menu, item, value } if menu == "title" && item == "rate" => {
                ctx.speech.set_rate(value.as_f32() / 100.0);
                ctx.storage.set("rate", value.as_f32());
            }
            _ => {}
        }
    }

    fn update(&mut self, _ctx: &mut Context) {}
}

// ------------------------------------------------------------------------- game

struct Game {
    lane: i32,
    bell_lane: i32,
    /// Seconds a bell needs to land; shrinks with every catch.
    fall: f32,
    /// Seconds until the current bell lands. Negative while waiting for the next bell.
    left: f32,
    next_ring: f32,
    score: u32,
    lives: u32,
    over: bool,
}

impl Default for Game {
    fn default() -> Self {
        Self { lane: 0, bell_lane: 0, fall: FIRST_FALL, left: 0.0, next_ring: 0.0, score: 0, lives: LIVES, over: false }
    }
}

impl Game {
    fn drop_bell(&mut self) {
        self.bell_lane = random::int(-LANES as i64, LANES as i64) as i32;
        self.left = self.fall;
        self.next_ring = 0.0;
    }

    /// Where the bell is, seen from the player: lanes to the side, a little in front.
    fn bell_position(&self) -> Vec3 {
        vec3((self.bell_lane - self.lane) as f32 * LANE_WIDTH, 2.0, 0.0)
    }

    fn ring(&mut self, ctx: &mut Context) {
        // 0.0 when the bell starts falling, 1.0 when it lands.
        let progress = 1.0 - self.left / self.fall;
        let position = self.bell_position();
        ctx.audio.sound("bell").at(position).pitch(1.0 + progress * 0.6).play();
        self.next_ring = 0.12 + 0.45 * (1.0 - progress);
    }

    fn land(&mut self, ctx: &mut Context) {
        if self.lane == self.bell_lane {
            self.score += 1;
            self.fall = (self.fall * 0.93).max(FASTEST_FALL);
            ctx.play("catch");
            if self.score.is_multiple_of(5) {
                ctx.speak(format!("{} bells!", self.score));
            }
        } else {
            self.lives -= 1;
            ctx.play("miss");
            match self.lives {
                0 => return self.game_over(ctx),
                1 => ctx.speak("Missed! Last life."),
                n => ctx.speak(format!("Missed! {n} lives left.")),
            }
        }
        // A short break before the next bell.
        self.left = -0.8;
    }

    fn game_over(&mut self, ctx: &mut Context) {
        self.over = true;
        let best: u32 = ctx.storage.get_or("best", 0);
        let text = if self.score > best {
            ctx.storage.set("best", self.score);
            format!("Game over. {} bells, a new best score!", self.score)
        } else {
            format!("Game over. {} bells. Your best is {best}.", self.score)
        };
        ctx.ui.message("over", text);
    }
}

impl Scene for Game {
    fn start(&mut self, ctx: &mut Context) {
        ctx.speak("Catch the bells! Left and right to move.");
        // Give the player a moment to listen to the first bell.
        self.left = -1.5;
    }

    fn update(&mut self, ctx: &mut Context) {
        if self.over {
            if ctx.ui.closed("over") {
                ctx.switch_scene(Title);
            }
            return;
        }
        if ctx.pressed(Key::Escape) {
            ctx.switch_scene(Title);
            return;
        }
        if ctx.pressed(Key::S) {
            ctx.speak(format!("{} bells, {} lives", self.score, self.lives));
        }
        for (key, step) in [(Key::Left, -1), (Key::Right, 1)] {
            if ctx.input.repeating(key, 0.3, 0.15) {
                let lane = (self.lane + step).clamp(-LANES, LANES);
                if lane == self.lane {
                    ctx.play("edge");
                } else {
                    self.lane = lane;
                    ctx.audio.sound("step").at(vec3(step as f32 * 0.5, 0.0, 0.0)).random_pitch(0.9, 1.1).play();
                }
            }
        }

        let dt = ctx.time.delta();
        if self.left < 0.0 {
            self.left += dt;
            if self.left >= 0.0 {
                self.drop_bell();
            }
            return;
        }
        self.left -= dt;
        self.next_ring -= dt;
        if self.left <= 0.0 {
            self.land(ctx);
        } else if self.next_ring <= 0.0 {
            self.ring(ctx);
        }
    }
}

