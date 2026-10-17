# Speech

`ctx.speech` speaks through [prism](https://github.com/ethindp/prism), which talks to
whatever the player uses:

- **Windows:** NVDA, JAWS, ZoomText and other screen readers, or SAPI / OneCore voices
- **macOS / iOS:** VoiceOver, AVSpeech
- **Linux:** Orca, speech-dispatcher

prism picks the best available output automatically. Speech runs on its own thread, so
speaking never blocks a frame.

## Basics

```rust
ctx.speech.speak("Level 2");             // interrupts current speech
ctx.speech.speak_queued("Get ready");    // waits until current speech is done
ctx.speech.stop();                       // silence
ctx.speech.braille("HP 50");             // braille display only
```

`speak` interrupts by default because in games the newest information is usually the most
important.

## History

The last 100 messages are kept. Give players a key to hear the last message again:

```rust
if ctx.input.pressed(Key::F1) {
    ctx.speech.repeat_last();
}
let all = ctx.speech.history();
```

## Options

```rust
ctx.speech.set_rate(0.7);      // only for TTS voices; screen readers keep user settings
ctx.speech.set_volume(0.9);
ctx.speech.set_muted(true);
ctx.speech.set_echo(true);     // also print to the console
ctx.speech.backend_name();     // e.g. Some("NVDA")
```

`Engine::new("..").echo_speech(true)` turns on console echo from the start. This is useful
while developing.

## Without prism

If you build with `default-features = false`, or prism finds no output on the system,
speech is printed to the console. The game keeps working.
