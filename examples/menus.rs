//! Main menu, options sub menu, name input and a quit confirmation.
//! Shows how to design your own click sounds with the synth.
//!
//! Run with `cargo run --example menus`.

use audio_code::prelude::*;

/// Settings shared between scenes through `ctx.insert` / `ctx.get`.
struct Settings {
    volume: f32,
    difficulty: usize,
    name: String,
}

struct MainMenu;

fn main_menu() -> Menu {
    Menu::new("main", "Main menu")
        .button("play", "Play")
        .button("options", "Options")
        .button("name", "Change name")
        .button("quit", "Quit")
        .closable(false)
        .wrap(true)
}

fn options_menu(s: &Settings) -> Menu {
    Menu::new("options", "Options")
        .slider("volume", "Volume", s.volume * 100.0, 0.0, 100.0, 10.0)
        .choice("difficulty", "Difficulty", &["Easy", "Normal", "Hard"], s.difficulty)
        .text_field("name", "Player name", s.name.clone())
        .button("back", "Back")
}

impl Scene for MainMenu {
    fn start(&mut self, ctx: &mut Context) {
        // Own interface sounds, designed in code. Files work too:
        // ctx.ui.set_sound(UiSound::Navigate, Some("ui/click"));
        ctx.ui.set_sound_data(UiSound::Navigate, Synth::triangle(1800.0).length(0.025).decay(150.0).volume(0.4));
        ctx.ui.set_sound_data(UiSound::Wrap, Synth::triangle(1800.0).sweep_to(2600.0).length(0.06).decay(60.0).volume(0.4));
        ctx.ui.set_sound_data(UiSound::Typing, Synth::noise().length(0.012).decay(300.0).volume(0.25));
        ctx.ui.set_sound_data(
            UiSound::Activate,
            Synth::sine(880.0).length(0.04).volume(0.35).then(Synth::sine(1320.0).length(0.08).decay(40.0).volume(0.35)),
        );
        ctx.insert(Settings { volume: 0.8, difficulty: 1, name: "Player".into() });
        ctx.ui.open(main_menu());
    }

    fn on_ui(&mut self, event: &UiEvent, ctx: &mut Context) {
        match event {
            UiEvent::Activated { menu, item } => match (menu.as_str(), item.as_str()) {
                ("main", "play") => {
                    let s = ctx.get::<Settings>().expect("settings exist");
                    let text = format!("Good luck, {}! (this is where your game would start)", s.name);
                    ctx.ui.message("intro", text);
                }
                ("main", "options") => {
                    let menu = options_menu(ctx.get::<Settings>().expect("settings exist"));
                    ctx.ui.open(menu);
                }
                ("main", "name") => ctx.ui.open(TextInput::new("name", "Enter your name").max_length(20).required()),
                ("main", "quit") => ctx.ui.open(Confirm::new("quit", "Do you really want to quit?").default_no()),
                ("options", "back") => ctx.ui.close(),
                _ => {}
            },
            UiEvent::Changed { menu, item, value } if menu == "options" => {
                let s = ctx.get_mut::<Settings>().expect("settings exist");
                match item.as_str() {
                    "volume" => s.volume = value.as_f32() / 100.0,
                    "difficulty" => s.difficulty = value.as_index(),
                    "name" => s.name = value.as_str().to_string(),
                    _ => {}
                }
                let volume = s.volume;
                ctx.audio.set_master_volume(volume);
            }
            UiEvent::TextSubmitted { id, text } if id == "name" => {
                ctx.get_mut::<Settings>().expect("settings exist").name = text.clone();
                ctx.speech.speak_queued(format!("Hello, {text}"));
            }
            UiEvent::Confirmed { id, yes: true } if id == "quit" => ctx.quit(),
            _ => {}
        }
    }

    fn update(&mut self, _ctx: &mut Context) {}
}

fn main() -> audio_code::Result<()> {
    Engine::new("Menu demo").echo_speech(true).run(MainMenu)
}
