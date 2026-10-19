//! macOS app bundles (`My Game.app`). A bundled game starts with `/` as working directory, so
//! files are looked up in the bundle's `Contents/Resources` folder and saves go to
//! `~/Library/Application Support` instead.

use std::path::{Path, PathBuf};

/// `Contents/Resources` of the app bundle the game runs from, if any.
pub(crate) fn resources() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    resources_of(&exe)
}

/// `.../Name.app/Contents/MacOS/exe` → `.../Name.app/Contents/Resources`.
fn resources_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    let is_bundle = macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension().is_some_and(|e| e.eq_ignore_ascii_case("app"));
    is_bundle.then(|| contents.join("Resources"))
}

/// Where a bundled game keeps its data: `~/Library/Application Support/<folder>`.
pub(crate) fn data_dir(folder: &str) -> Option<PathBuf> {
    resources()?;
    let home = std::env::var_os("HOME")?;
    Some(Path::new(&home).join("Library/Application Support").join(folder))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_resources_of_bundle() {
        let exe = Path::new("/Applications/Bell Catcher.app/Contents/MacOS/bell_catcher");
        assert_eq!(resources_of(exe), Some(PathBuf::from("/Applications/Bell Catcher.app/Contents/Resources")));
    }

    #[test]
    fn plain_executables_are_no_bundle() {
        assert_eq!(resources_of(Path::new("/home/me/game/target/debug/game")), None);
        assert_eq!(resources_of(Path::new("/tmp/Contents/MacOS/game")), None);
    }
}
