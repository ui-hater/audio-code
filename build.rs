//! Copies the prism library (`prism.dll`, `libprism.so` or `libprism.dylib`) next to the
//! executables, because prism is linked dynamically and a game does not start without it.

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DEP_PRISM_ROOT");
    // Only set when the `prism` feature is on (prism-sys has `links = "prism"`).
    let Some(root) = env::var_os("DEP_PRISM_ROOT") else {
        return;
    };
    let Some(library) = find_library(Path::new(&root)) else {
        println!("cargo:warning=audio-code: prism library not found under {}", Path::new(&root).display());
        return;
    };
    println!("cargo:rerun-if-changed={}", library.display());
    let Some(profile_dir) = profile_dir() else {
        return;
    };
    let name = library.file_name().expect("library path has a file name");
    for dir in [profile_dir.clone(), profile_dir.join("examples"), profile_dir.join("deps")] {
        if std::fs::create_dir_all(&dir).is_err() {
            continue;
        }
        if let Err(e) = std::fs::copy(&library, dir.join(name)) {
            // For example a running game keeps the old copy locked.
            println!("cargo:warning=audio-code: could not copy {} to {}: {e}", library.display(), dir.display());
        }
    }
}

/// The built prism library inside prism-sys's CMake install prefix.
fn find_library(root: &Path) -> Option<PathBuf> {
    let names = ["prism.dll", "libprism.so", "libprism.dylib"];
    ["bin", "lib"].iter().flat_map(|dir| names.iter().map(move |name| root.join(dir).join(name))).find(|p| p.is_file())
}

/// `target/<profile>` (or `target/<triple>/<profile>`), derived from
/// `OUT_DIR = <profile dir>/build/audio-code-<hash>/out`.
fn profile_dir() -> Option<PathBuf> {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR")?);
    out_dir.ancestors().nth(3).map(Path::to_path_buf)
}
