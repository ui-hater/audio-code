# macOS

Games made with audio-code run on macOS 11.7 and newer, on Apple Silicon and Intel Macs.
Nothing in your game code changes; this page covers what is different on the Mac and how
to ship a game as a `.app`.

## Speech

The default build speaks through Apple's own APIs (feature `macos`, on by default):

- With **VoiceOver** on, messages go to VoiceOver as announcements. Players hear their own
  VoiceOver voice and settings, and braille displays show the text.
- Without VoiceOver, the **system voice** (`AVSpeechSynthesizer`) speaks. It follows
  *System Settings → Accessibility → Spoken Content*, and `ctx.speech.voices()` /
  `set_voice` let players pick another voice.

Details are in [speech.md](speech.md#macos-voiceover-and-the-system-voice).

## Files and saves inside a .app

A game started from the Finder has `/` as its working directory. The engine handles that:

- Sounds are also looked up in the bundle's `Contents/Resources` folder, so
  `ctx.play("step")` finds `My Game.app/Contents/Resources/sounds/step.ogg`.
- The default save file goes to `~/Library/Application Support/<game title>/`, because the
  app itself should never be written to.

Outside a bundle (`cargo run`) everything works as on other systems.

## Building a .app

`scripts/macos_app.py` builds a universal binary (Apple Silicon and Intel), puts it into an
app bundle with an `Info.plist`, signs it ad hoc and zips it:

```sh
python scripts/macos_app.py --example bell_catcher --name "Bell Catcher"
python scripts/macos_app.py --bin my_game --name "My Game" --resources sounds
```

`--resources` copies files or folders into `Contents/Resources`. Other options: `--id`
(bundle identifier), `--version`, `--min-macos` and `--features`. The result lands in
`target/macos/`.

### On a Mac

Install the Xcode command line tools and both Rust targets, then run the script:

```sh
xcode-select --install
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

### On Windows or Linux

The script cross-compiles with [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild):

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
pip install ziglang==0.13.0
cargo install --locked cargo-zigbuild
cargo install --locked apple-codesign --bin rcodesign   # ad-hoc signing
```

zig 0.13 is used because newer versions only target macOS 13 and later. Make sure zig is on
`PATH` (the `ziglang` package installs `zig` next to its Python module).

Linking needs the frameworks of a macOS SDK (`MacOSX*.sdk`). Copy it from a Mac
(`xcrun --show-sdk-path`) and point `SDKROOT` to it. Apple's SDK license allows using the
SDK only on Apple hardware, so check that before you use a copy elsewhere.

```sh
SDKROOT=/path/to/MacOSX.sdk python scripts/macos_app.py --example bell_catcher --name "Bell Catcher"
```

## Sharing the game

The app is signed ad hoc, not with an Apple Developer ID, and it is not notarized. When
players open a downloaded copy for the first time, macOS blocks it. They can allow it in one
of two ways:

- *System Settings → Privacy & Security*, scroll down, **Open Anyway** (macOS 15 and later;
  on older versions: right-click the app, **Open**).
- In Terminal: `xattr -dr com.apple.quarantine "Bell Catcher.app"`

To avoid this, sign the app with a Developer ID certificate and notarize it
(`codesign --options runtime` and `xcrun notarytool`); that needs a paid Apple Developer
account.
