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
ctx.speech.set_rate(0.7);      // 0.0 slowest to 1.0 fastest; only for TTS voices, screen readers keep user settings
ctx.speech.set_volume(0.9);    // 0.0 silent to 1.0 full; also only for TTS voices
ctx.speech.set_muted(true);
ctx.speech.set_echo(true);     // also print to the console
ctx.speech.backend_name();     // e.g. Some("NVDA")
```

### Voices

Players can pick the voice of the system voice output, for example in an options menu:

```rust
let voices = ctx.speech.voices();          // Vec<Voice>: id, name, language
ctx.speech.set_voice("Anna");              // by name or id; returns false if not installed
ctx.speech.set_voice(&voices[3].id);
ctx.speech.reset_voice();                  // back to the default voice
ctx.speech.voice();                        // Some(&Voice) after set_voice
```

`voices()` lists the voices of prism's TTS output or, on macOS, of `AVSpeechSynthesizer`.
It is empty when a screen reader speaks: NVDA, VoiceOver and the others keep the voice the
player chose in the screen reader, which is what players expect. Store `voice.id` with
`ctx.storage` to keep the choice; `examples/bell_catcher.rs` has a complete voice menu.

`Engine::new("..").echo_speech(true)` turns on console echo from the start. This is useful
while developing.

## Fallbacks

The engine uses the first output that works:

1. **prism** (feature `prism`, on by default)
2. **VoiceOver or the system voice** (macOS, feature `macos`)
3. **the console**

`ctx.speech.backend_name()` tells you which one is active, e.g. `"NVDA"`.

prism picks the best output that is available right now: a running screen reader (NVDA,
JAWS, ZoomText, ...) first, otherwise the system voice (SAPI / OneCore on Windows). Screen
readers keep the user's own rate and volume, so `set_rate` and `set_volume` only affect the
system voice. Building prism needs CMake and a C++ compiler, see the README. prism is built
as a shared library, so ship `prism.dll` (Linux: `libprism.so`, macOS: `libprism.dylib`)
next to your executable. Without the `prism` feature the build is
pure Rust.

### macOS: VoiceOver and the system voice

On macOS the default build speaks through Apple's own APIs, without prism (feature `macos`,
on by default):

- When **VoiceOver** is on, every message is sent to VoiceOver as an accessibility
  announcement. Players hear their own VoiceOver voice, rate and verbosity, and braille
  displays show the text too. No setup is needed.
- Otherwise the **system voice** speaks through `AVSpeechSynthesizer`. It uses the voice and
  rate from *System Settings → Accessibility → Spoken Content* (macOS 14 and newer) until the
  game calls `set_rate` or `set_voice`. `set_rate` (0.5 is normal), `set_volume` and
  `set_voice` work here, and
  `speak_queued` waits for the current message.

The engine checks for VoiceOver on every message, so turning it on or off while the game
runs works. With VoiceOver, `speak` interrupts and `speak_queued` is sent as a lower priority
announcement; VoiceOver decides when that is spoken and may skip it when many messages
arrive at once. VoiceOver has no "stop" for announcements, so `stop` only silences the system
voice. Braille-only output (`braille`) needs prism on macOS.

AppKit only works on the main thread, so speech is handed to the main thread and spoken
between frames. VoiceOver ignores announcements while the game window is not active.

### Console

If no output works, speech is printed to the console, so the game always keeps working.
