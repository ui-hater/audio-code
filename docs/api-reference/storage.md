# Storage

Simple saving of high scores, settings and progress: `ctx.storage` (type `Storage`).

Values are kept as text in a `key = value` file. It is loaded when the game starts and
written when it quits (or when you call `save`). The file is `<title>.save` in the working
directory unless set with `Engine::save_file`. In headless mode the storage lives only in
memory. Anything that implements `Display` / `ToString` can be stored and anything that
implements `FromStr` can be read: numbers, `bool`, `String`, ...

```rust
use audio_code::prelude::*;   // Storage
```

### `ctx.storage.get<T: FromStr>(key: &str) -> Option<T>`

Reads a value. `None` if it is missing or cannot be parsed as `T`. The type usually comes
from the variable or a turbofish.

```rust
let name: Option<String> = ctx.storage.get("player_name");
let level = ctx.storage.get::<u32>("level");
```

### `ctx.storage.get_or<T: FromStr>(key: &str, default: T) -> T`

Reads a value, or `default` if it is missing or invalid.

```rust
let best: u32 = ctx.storage.get_or("highscore", 0);
let volume: f32 = ctx.storage.get_or("volume", 0.8);
ctx.audio.set_master_volume(volume);
```

### `ctx.storage.set(key: &str, value: impl ToString)`

Stores a value. It is written to disk on quit or `save`. Panics if `key` contains `=` or a
line break. Values may contain anything, including line breaks.

```rust
let score = 1200;
if score > ctx.storage.get_or("highscore", 0) {
    ctx.storage.set("highscore", score);
    ctx.speak("New high score!");
}
```

### `ctx.storage.add(key: &str, amount: i64) -> i64`

Adds `amount` to a number (missing counts as 0) and returns the new value.

```rust
let played = ctx.storage.add("games_played", 1);
ctx.speak(format!("Game number {played}"));
```

### `ctx.storage.remove(key: &str)`

Removes a value.

```rust
ctx.storage.remove("checkpoint");
```

### `ctx.storage.contains(key: &str) -> bool`

Whether a value exists.

```rust
if !ctx.storage.contains("tutorial_done") {
    ctx.speak("Let's start with the tutorial");
    ctx.storage.set("tutorial_done", true);
}
```

### `ctx.storage.keys() -> impl Iterator<Item = &str>`

All keys in alphabetical order.

```rust
let keys: Vec<String> = ctx.storage.keys().map(String::from).collect();
```

### `ctx.storage.clear()`

Removes everything.

```rust
ctx.storage.clear();
```

### `ctx.storage.save() -> Result<()>`

Writes changes to disk now and creates the folder if needed. Does nothing without changes
or for in-memory storage. Saving also happens automatically when the game quits.

```rust
if let Err(e) = ctx.storage.save() {
    eprintln!("could not save: {e}");
}
```

### `ctx.storage.path() -> Option<&Path>`

The file this storage is saved to, `None` for in-memory storage.

```rust
if let Some(path) = ctx.storage.path() {
    println!("saving to {}", path.display());
}
```

### `Storage::open(path: impl Into<PathBuf>) -> Storage`

Loads storage from a plain text file. A missing file gives empty storage. An encrypted
file cannot be read without its secret; it is kept as `<file>.broken` (see `load_error`).
The engine does this for `ctx.storage`; use it for extra files such as several save slots.

```rust
let mut slot2 = Storage::open("saves/slot2.save");
slot2.set("level", 4);
let _ = slot2.save();
```

## Encryption

Optional. With `Engine::encrypt_save(secret)` (or `Storage::open_encrypted`) the file is
encrypted and authenticated with XChaCha20-Poly1305, using a key made from `secret`.
`get` / `set` work the same. Plain text saves still load and are written encrypted on the
next save. A file that cannot be decrypted (edited, damaged, other secret) is renamed to
`<file>.broken` instead of being overwritten, and the storage starts empty. The secret is
compiled into the game, so this stops casual editing, not a determined attacker.

### `Storage::open_encrypted(path: impl Into<PathBuf>, secret: &str) -> Storage`

Loads an encrypted file (or a plain one, to be converted). A missing file gives empty
storage.

```rust
let mut slot = Storage::open_encrypted("saves/slot1.save", "my secret word");
slot.set("coins", 120);
let _ = slot.save();
```

### `ctx.storage.load_error() -> Option<&str>`

Why the save file could not be loaded: wrong secret, edited or damaged, or encrypted
while no secret was given. The file was kept as `<file>.broken`. `None` when loading worked
or there was no file. Plain `Storage::open` also reports a file that is encrypted or not
valid text.

```rust
if ctx.storage.load_error().is_some() {
    ctx.speak("Your save file could not be read. A new game starts.");
}
```

### `ctx.storage.is_encrypted() -> bool`

Whether the file is written encrypted.

```rust
let protected = ctx.storage.is_encrypted();
```

### `ctx.storage.set_encryption(secret: Option<&str>)`

Turns encryption on (`Some(secret)`) or off (`None`). The file changes form on the next
save.

```rust
ctx.storage.set_encryption(Some("my secret word"));
let _ = ctx.storage.save();
```

### `Storage::in_memory() -> Storage`

Storage that is never written to disk. Same as `Storage::default()`.

```rust
let mut temp = Storage::in_memory();
temp.set("x", 1);
assert_eq!(temp.get::<i32>("x"), Some(1));
```
