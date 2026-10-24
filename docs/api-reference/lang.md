# Lang

Translations and the current language: `ctx.lang` (type `Lang`). The guide is
[translations.md](../translations.md).

Texts come from `lang/<code>.txt` files (`key = text` per line, `#` comments, `{name}`
placeholders, `\n` line breaks), loaded when the game starts. The start language is the
player's saved choice, else `Engine::language`, else the system language if a file exists
for it, else the fallback (`"en"`). Missing keys come from the fallback language, then from
the built in engine texts (English and German), and finally the key itself is returned and
reported once on the console.

```rust
use audio_code::prelude::*;   // Lang
```

## Looking up texts

### `ctx.tr(key: &str) -> String`

The text for `key` in the current language. Same as `ctx.lang.get(key)`.

```rust
ctx.speak(ctx.tr("intro"));
```

### `ctx.tr_with(key: &str, args: &[(&str, &dyn Display)]) -> String`

The text with `{name}` placeholders filled in. Same as `ctx.lang.format(key, args)`.

```rust
// lang/en.txt: score = You have {points} points.
let points = 12;
ctx.speak(ctx.tr_with("score", &[("points", &points)]));
```

### `ctx.lang.get(key: &str) -> String`

The text for `key`: current language, fallback language, built in engine texts, else the
key itself (with a warning printed once).

```rust
let title = ctx.lang.get("menu.title");
```

### `ctx.lang.format(key: &str, args: &[(&str, &dyn Display)]) -> String`

`get` plus placeholders. Arguments can be anything that implements `Display`.

```rust
let text = ctx.lang.format("greeting", &[("name", &"Anna"), ("level", &3)]);
```

### `ctx.lang.plural(key: &str, n: i64) -> String`

Uses `key.one` when `n` is 1 or -1 and `key.other` otherwise, and fills in `{n}`. Without
those keys, `key` itself is used.

```rust
// coins.one = 1 coin
// coins.other = {n} coins
ctx.speak(ctx.lang.plural("coins", 5));   // "5 coins"
```

### `ctx.lang.plural_with(key: &str, n: i64, args: &[(&str, &dyn Display)]) -> String`

`plural` with more placeholders.

```rust
// found.other = {player} found {n} coins
let text = ctx.lang.plural_with("found", 2, &[("player", &"Anna")]);
```

### `ctx.lang.has(key: &str) -> bool`

Whether the current or the fallback language (or the engine) has a text for `key`.

```rust
if ctx.lang.has("level3.hint") {
    ctx.speak(ctx.tr("level3.hint"));
}
```

## Choosing the language

### `ctx.set_language(code: &str) -> bool`

Switches the language and saves the choice in `ctx.storage` (key `language`), so the next
start uses it. Returns `false` and changes nothing if there are no texts for it.

```rust
if let Some(code) = ctx.ui.activated("language").map(String::from) {
    ctx.set_language(&code);
}
```

### `ctx.lang.set(code: &str) -> bool`

Switches the language without saving it. `"de-AT"` and `"de_AT.UTF-8"` fall back to `"de"`.
English and German always exist because the engine texts are built in.

```rust
ctx.lang.set("de");
```

### `ctx.lang.current() -> &str`

The current language code, lower case, e.g. `"de"`.

```rust
let code = ctx.lang.current().to_string();
```

### `ctx.lang.fallback() -> &str` / `ctx.lang.set_fallback(code: &str)`

The language used for keys missing in the current one. Default `"en"`.

```rust
ctx.lang.set_fallback("de");
```

### `ctx.lang.languages() -> Vec<String>`

Codes of all languages with loaded texts, sorted. Built in engine texts do not count.

```rust
let mut menu = Menu::new("language", "Language");
for code in ctx.lang.languages() {
    let name = ctx.lang.name(&code);
    menu = menu.button(code, name);
}
ctx.ui.open(menu);
```

### `ctx.lang.name(code: &str) -> String`

The language's name from the `language` key of its file, else the code.

```rust
let name = ctx.lang.name("de");   // "Deutsch"
```

### `Lang::system() -> Option<String>`

The player's system language, e.g. `"de-de"`, if the system tells.

```rust
use audio_code::lang::Lang;
if let Some(code) = Lang::system() {
    ctx.speak(code);
}
```

## Adding texts

### `ctx.lang.load_dir(dir: impl AsRef<Path>) -> usize`

Loads every `<code>.txt` in a folder and returns how many files were loaded. The engine
already loads `Engine::lang_dir` (default `"lang"`) at start; a missing folder loads
nothing.

```rust
ctx.lang.load_dir("mods/lang");
```

### `ctx.lang.add_text(code: &str, text: &str)`

Adds the lines of a translation file to a language. Later keys replace earlier ones. Use it
with `include_str!` to ship the texts inside the executable.

```rust
ctx.lang.add_text("de", include_str!("../lang/de.txt"));
```

### `ctx.lang.insert(code: &str, key: impl Into<String>, text: impl Into<String>)`

Adds or replaces a single text. Also overrides engine texts.

```rust
ctx.lang.insert("de", "ui.checked", "an");
```

### `Lang::new() -> Lang`

Empty tables, English as current and fallback language. Handy in tests.

```rust
use audio_code::lang::Lang;
let mut lang = Lang::new();
lang.insert("en", "hi", "Hello, {name}");
assert_eq!(lang.format("hi", &[("name", &"Bob")]), "Hello, Bob");
```

## Engine texts in the current language

### `ctx.describe(from: BodyId, target: BodyId) -> String`

Where `target` is as seen from `from`, like `ctx.physics.describe` but in the current
language: `"3 o'clock, 12 meters"`, `"3 Uhr, 12 Meter"`.

```rust
ctx.speak(ctx.describe(player, enemy));
```

### `ctx.lang.describe_position(from: Vec3, yaw: f32, target: Vec3) -> String`

`math::describe_position` in the current language.

```rust
let text = ctx.lang.describe_position(Vec3::ZERO, 0.0, vec3(0.0, 10.0, 4.0));
```

### `ctx.lang.direction_name(degrees: f32) -> String`

`math::direction_name` in the current language (`"Nordosten"`).

```rust
let facing = ctx.physics.facing(player);
ctx.speak(ctx.lang.direction_name(facing));
```

### `ctx.lang.describe_speed(vehicle: &Vehicle) -> String`

`Vehicle::describe_speed` in the current language (`"54 Kilometer pro Stunde"`).

```rust
if let Some(car) = ctx.physics.vehicle(player) {
    let text = ctx.lang.describe_speed(car);
    ctx.speak(text);
}
```

The keys of all engine texts are listed in [translations.md](../translations.md#engine-texts).
