# Speech

Screen reader and text-to-speech output: `ctx.speech` (type `Speech`). `ctx.speak(text)` is
the shortcut for the most common call.

The engine picks the first output that works: prism (feature `prism`, on by default, talks to
NVDA, JAWS, VoiceOver, Orca or the system voice), then VoiceOver or the system voice (macOS,
feature `macos`, on by default), then the console. Speech runs on its own
thread and never blocks the game. Empty text is ignored. In headless mode
(`Engine::headless`) everything is printed.

### `ctx.speech.speak(text: impl AsRef<str>)`

Speaks text and interrupts whatever is being said. This is what you want almost always,
because the newest information matters most in a game.

```rust
ctx.speech.speak("Health low");
ctx.speech.speak(format!("{} meters to the exit", 12));
```

### `ctx.speech.speak_queued(text: impl AsRef<str>)`

Speaks text after the current speech has finished, without interrupting.

```rust
ctx.speech.speak("Level 2.");
ctx.speech.speak_queued("Find the key.");
```

### `ctx.speech.braille(text: impl AsRef<str>)`

Shows text on a braille display only, without speaking it. Needs an output that supports
braille (prism). Not added to the history.

```rust
ctx.speech.braille("HP 80/100");
```

### `ctx.speech.stop()`

Silences speech immediately.

```rust
if ctx.pressed(Key::Control) {
    ctx.speech.stop();
}
```

### `ctx.speech.repeat_last()`

Speaks the most recent message again. Bind it to a key so players can catch up.

```rust
if ctx.pressed(Key::R) {
    ctx.speech.repeat_last();
}
```

### `ctx.speech.last() -> Option<&str>`

The most recently spoken message. Useful in tests.

```rust
ctx.speak("Ready");
assert_eq!(ctx.speech.last(), Some("Ready"));
```

### `ctx.speech.history() -> &[String]`

The last 100 messages, oldest first. Messages spoken while muted are included.

```rust
let previous: Vec<String> = ctx.speech.history().iter().rev().take(5).cloned().collect();
```

### `ctx.speech.set_rate(rate: f32)`

Speech rate from 0.0 (slowest) to 1.0 (fastest), if the output supports it (prism and the
macOS system voice, where 0.5 is a normal rate).
Screen readers usually keep the user's own settings.

```rust
ctx.speech.set_rate(0.6);
```

### `ctx.speech.set_volume(volume: f32)`

Speech volume from 0.0 to 1.0, if the output supports it (prism and the macOS system voice).

```rust
ctx.speech.set_volume(0.8);
```

### `ctx.speech.voices() -> Vec<Voice>`

The voices of the system voice output (prism's TTS voices, or `AVSpeechSynthesizer` on
macOS), sorted by language and name. Empty when a screen reader speaks (it keeps the voice
the player set up there) or speech goes to the console. Right after the start this waits until
the output is ready, at most a few seconds.

```rust
for voice in ctx.speech.voices() {
    println!("{voice}"); // "Anna (de-DE)"
}
```

### `ctx.speech.set_voice(voice: impl AsRef<str>) -> bool`

Switches the system voice. Accepts a `Voice::id` or a voice name in any case. Returns `false`
(and prints a warning) if there is no such voice; the current voice stays.

```rust
if !ctx.speech.set_voice("Anna") {
    ctx.speak("That voice is not installed");
}
```

### `ctx.speech.reset_voice()`

Goes back to the voice the game started with.

```rust
ctx.speech.reset_voice();
```

### `ctx.speech.voice() -> Option<&Voice>`

The voice chosen with `set_voice`, or `None` for the default voice.

```rust
let name = ctx.speech.voice().map_or("default", |v| v.name.as_str());
```

### `Voice`

```rust
pub struct Voice {
    pub id: String,               // for set_voice, e.g. "com.apple.voice.compact.de-DE.Anna"
    pub name: String,             // "Anna", show this to players
    pub language: Option<String>, // "de-DE"
}
```

`Display` prints the name and language, e.g. `Anna (de-DE)`. Save `id` to remember the
player's choice:

```rust
if let Some(voice) = ctx.speech.voice() {
    let id = voice.id.clone();
    ctx.storage.set("voice", id);
}
```

### `ctx.speech.set_echo(echo: bool)`

Also prints everything spoken to the console. Same as `Engine::echo_speech`, but can be
changed while running.

```rust
ctx.speech.set_echo(true);
```

### `ctx.speech.set_muted(muted: bool)`

Mutes speech output. Messages still go to the history.

```rust
ctx.speech.set_muted(true);
```

### `ctx.speech.backend_name() -> Option<String>`

Name of the active output, for example `"NVDA"`, `"SAPI"`, `"VoiceOver"` or `"AVSpeechSynthesizer"`. `None` while starting up or when printing to the console.

```rust
if let Some(name) = ctx.speech.backend_name() {
    println!("speaking through {name}");
}
```

### `Speech::console() -> Speech`

Speech that only prints to the console. Used by headless mode and for tests.

```rust
let mut speech = Speech::console();
speech.speak("printed");
```
