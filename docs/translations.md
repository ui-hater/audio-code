# Translations

Make your game playable in several languages. Texts live in one plain text file per
language, the game speaks keys instead of text, and the engine's own announcements (menus,
dialogs, directions, speeds) follow the chosen language too.

## Your first translated game

Create a `lang` folder next to the game (like `sounds`) with one file per language. The
file name is the language code: `en.txt`, `de.txt`, `fr.txt`, `es.txt`, ...

`lang/en.txt`:

```text
# Lines starting with # are comments.
language = English
intro = Welcome! Press space to jump, escape to quit.
score = You have {points} points.
```

`lang/de.txt`:

```text
language = Deutsch
intro = Willkommen! Drücke Leertaste zum Springen, Escape zum Beenden.
score = Du hast {points} Punkte.
```

Speak keys with `ctx.tr`, and fill placeholders with `ctx.tr_with`:

```rust
use audio_code::prelude::*;

fn main() -> audio_code::Result<()> {
    let mut points = 0;
    audio_code::run("Jumper", move |ctx| {
        if ctx.once("intro") {
            ctx.speak(ctx.tr("intro"));
        }
        if ctx.pressed(Key::Space) {
            points += 1;
            ctx.speak(ctx.tr_with("score", &[("points", &points)]));
        }
        if ctx.pressed(Key::Escape) {
            ctx.quit();
        }
    })
}
```

On a German computer this game speaks German, everywhere else English.

## The file format

- One `key = text` per line. Spaces around the key and the text are ignored.
- Lines starting with `#` and empty lines are skipped.
- `{name}` is a placeholder, filled in by `tr_with`, `lang.format` and `lang.plural`.
  Placeholders without an argument stay as they are.
- `\n` in a text is a line break (useful for `ctx.ui.message`).
- Save the files as UTF-8 (Notepad and every other editor do this by default).
- Keys are free text. Dots make groups readable: `menu.play`, `menu.options`,
  `level1.intro`.
- The key `language` is the language's own name ("Deutsch", "Français"). It is shown in
  language menus.

## Which language is used

When the game starts, the first of these that exists wins:

1. The language the player chose last time with `ctx.set_language`.
2. The language given with `Engine::new(..).language("de")`.
3. The system language, if the game has a file for it.
4. The fallback language, English (`ctx.lang.set_fallback("de")` changes it).

`"de-AT"` or `"de_AT.UTF-8"` find `de.txt` when there is no `de-at.txt`.

If a key is missing in the current language, the text from the fallback language is used.
If it is missing everywhere, the key itself is spoken and a warning is printed once, so a
forgotten line never crashes the game.

A game written only in German needs no files for the engine texts at all:

```rust
use audio_code::prelude::*;

fn main() -> audio_code::Result<()> {
    Engine::new("Spiel").language("de").run_fn(|ctx| {
        // Menus now say "aktiviert", dialogs "Ja" / "Nein", directions "3 Uhr, 5 Meter".
        if ctx.pressed(Key::Escape) {
            ctx.quit();
        }
    })
}
```

## A language menu

`ctx.lang.languages()` lists the codes of all loaded files and `ctx.lang.name(code)` their
names. `ctx.set_language` switches and remembers the choice in `ctx.storage`:

```rust
use audio_code::prelude::*;

fn open_language_menu(ctx: &mut Context) {
    let mut menu = Menu::new("language", ctx.tr("menu.language"));
    for code in ctx.lang.languages() {
        let name = ctx.lang.name(&code);
        menu = menu.button(code, name);
    }
    ctx.ui.open(menu);
}

fn update(ctx: &mut Context) {
    if let Some(code) = ctx.ui.activated("language").map(String::from) {
        ctx.set_language(&code);
        ctx.ui.close();
        ctx.speak(ctx.tr("language.changed"));
    }
}
```

Menus keep the texts they were built with, so build a menu again after switching the
language.

## Plurals

`ctx.lang.plural(key, n)` uses `key.one` for 1 and `key.other` for every other number, and
fills in `{n}`:

```text
coins.one = 1 coin
coins.other = {n} coins
```

```rust
ctx.speak(ctx.lang.plural("coins", 3));   // "3 coins"
```

`plural_with` takes more placeholders. Languages with more plural forms (Polish, Russian,
Arabic, ...) need a key per form chosen by your own code.

## Texts inside the executable

Instead of shipping a `lang` folder, compile the files into the game:

```rust
ctx.lang.add_text("en", include_str!("../lang/en.txt"));
ctx.lang.add_text("de", include_str!("../lang/de.txt"));
ctx.lang.set("de");
```

`ctx.lang.insert("de", "key", "Text")` adds a single text. Other folders work with
`Engine::lang_dir("assets/lang")`. In a macOS app bundle the folder is also looked up in
`Contents/Resources`.

## Engine texts

The engine speaks some words by itself: "checked" on checkboxes, "Yes" / "No" in
questions, character names in text fields, `ctx.describe(player, enemy)` and
`ctx.lang.describe_speed(vehicle)`. English and German are built in. For other languages,
or to change the wording, put these keys into your files:

| Key | English | German |
|-----|---------|--------|
| `ui.checked` / `ui.not_checked` | checked / not checked | aktiviert / nicht aktiviert |
| `ui.blank` | blank (empty text field) | leer |
| `ui.empty` | empty (menu without items) | leer |
| `ui.edit` | {label}, edit, {value} | {label}, Eingabefeld, {value} |
| `ui.characters.one` / `.other` | 1 character / {n} characters (passwords) | 1 Zeichen / {n} Zeichen |
| `ui.position` | {index} of {count} (`Menu::positions`) | {index} von {count} |
| `ui.yes` / `ui.no` | Yes / No | Ja / Nein |
| `ui.star` | star (typed password character) | Stern |
| `char.space`, `char.dot`, `char.comma`, `char.exclamation`, `char.question`, `char.dash`, `char.underscore`, `char.colon`, `char.semicolon`, `char.apostrophe`, `char.quote`, `char.slash`, `char.backslash`, `char.at`, `char.number`, `char.left_paren`, `char.right_paren` | space, dot, comma, ... | Leerzeichen, Punkt, Komma, ... |
| `direction.north`, `direction.north_east`, `direction.east`, `direction.south_east`, `direction.south`, `direction.south_west`, `direction.west`, `direction.north_west` | north, north east, ... | Norden, Nordosten, ... |
| `describe.clock` | {hour} o'clock | {hour} Uhr |
| `describe.above` / `describe.below` | {distance} above / below | {distance} darüber / darunter |
| `describe.directly_above` / `describe.directly_below` | directly above, {distance} | direkt darüber, {distance} |
| `unit.meters.one` / `.other` | 1 meter / {n} meters | 1 Meter / {n} Meter |
| `unit.kmh.one` / `.other` | 1 kilometer per hour / {n} kilometers per hour | 1 Kilometer pro Stunde / {n} Kilometer pro Stunde |
| `vehicle.reversing` | reversing, {speed} | rückwärts, {speed} |
| `net.left` | Left the game | Hat das Spiel verlassen |
| `net.closed` | The server was closed | Der Server wurde geschlossen |
| `net.kicked` / `net.kicked_reason` | Kicked from the server / Kicked from the server: {reason} | Vom Server geworfen / Vom Server geworfen: {reason} |
| `net.server_full` | The server is full | Der Server ist voll |
| `net.wrong_game` | This server runs another game: {game} | Auf diesem Server läuft ein anderes Spiel: {game} |
| `net.wrong_version` | Wrong game version. The server has version {version} | Falsche Spielversion. Der Server hat Version {version} |
| `net.timed_out` | The connection timed out | Zeitüberschreitung der Verbindung |
| `net.failed` | Could not connect: {error} | Verbindung fehlgeschlagen: {error} |
| `net.lost` | Connection lost: {error} | Verbindung verloren: {error} |

`ctx.physics.describe`, `math::describe_position`, `math::direction_name` and
`Vehicle::describe_speed` stay English; use `ctx.describe`, `ctx.lang.describe_position`,
`ctx.lang.direction_name` and `ctx.lang.describe_speed` for the current language.

Custom widgets reach the translations as `ui.lang` in their `UiContext`.

All functions are listed in the [API reference](api-reference/lang.md).
