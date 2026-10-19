#!/usr/bin/env python3
"""Builds a game as a macOS app bundle (`My Game.app`) and zips it for sharing.

The binary is universal (Apple Silicon and Intel). On a Mac this uses cargo, lipo and
codesign. On Windows or Linux it cross-compiles with cargo-zigbuild, which needs zig and a
macOS SDK (see docs/macos.md).

    python scripts/macos_app.py --example bell_catcher --name "Bell Catcher"
    python scripts/macos_app.py --bin my_game --name "My Game" --resources sounds
"""

import argparse
import os
import platform
import plistlib
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path

TARGETS = ["aarch64-apple-darwin", "x86_64-apple-darwin"]


def run(cmd, **kwargs):
    print("+", " ".join(str(c) for c in cmd), flush=True)
    subprocess.run(cmd, check=True, **kwargs)


def build(args, root: Path) -> Path:
    kind = ["--example", args.example] if args.example else ["--bin", args.bin]
    name = args.example or args.bin
    sub = "examples" if args.example else ""
    env = dict(os.environ, MACOSX_DEPLOYMENT_TARGET=args.min_macos)
    extra = ["--features", args.features] if args.features else []

    if platform.system() == "Darwin":
        for target in TARGETS:
            run(["cargo", "build", "--release", "--target", target, *kind, *extra], cwd=root, env=env)
        out = root / "target" / "universal-apple-darwin" / "release"
        out.mkdir(parents=True, exist_ok=True)
        binary = out / name
        parts = [root / "target" / t / "release" / sub / name for t in TARGETS]
        run(["lipo", "-create", "-output", binary, *parts])
        return binary

    if not os.environ.get("SDKROOT"):
        sys.exit("SDKROOT must point to a macOS SDK (MacOSX*.sdk) to cross-compile, see docs/macos.md")
    # zig ignores MACOSX_DEPLOYMENT_TARGET, so pass the minimum version to the linker too, and
    # leave room in the header for the code signature added later.
    flags = f"-C link-arg=-mmacosx-version-min={args.min_macos} -C link-arg=-Wl,-headerpad_max_install_names"
    for target in TARGETS:
        key = "CARGO_TARGET_" + target.upper().replace("-", "_") + "_RUSTFLAGS"
        env[key] = (env.get(key, "") + " " + flags).strip()
    run(["cargo", "zigbuild", "--release", "--target", "universal2-apple-darwin", *kind, *extra], cwd=root, env=env)
    return root / "target" / "universal2-apple-darwin" / "release" / sub / name


def make_bundle(args, binary: Path, out: Path) -> Path:
    app = out / f"{args.name}.app"
    if app.exists():
        shutil.rmtree(app)
    macos = app / "Contents" / "MacOS"
    resources = app / "Contents" / "Resources"
    macos.mkdir(parents=True)
    resources.mkdir()
    executable = binary.name
    shutil.copy2(binary, macos / executable)
    os.chmod(macos / executable, 0o755)
    for res in args.resources:
        src = Path(res)
        dest = resources / src.name
        if src.is_dir():
            shutil.copytree(src, dest)
        else:
            shutil.copy2(src, dest)
    info = {
        "CFBundleName": args.name,
        "CFBundleDisplayName": args.name,
        "CFBundleIdentifier": args.id,
        "CFBundleExecutable": executable,
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": args.version,
        "CFBundleVersion": args.version,
        "CFBundleInfoDictionaryVersion": "6.0",
        "LSMinimumSystemVersion": args.min_macos,
        "LSApplicationCategoryType": "public.app-category.games",
        "NSHighResolutionCapable": True,
        "NSPrincipalClass": "NSApplication",
    }
    with open(app / "Contents" / "Info.plist", "wb") as f:
        plistlib.dump(info, f)
    return app


def sign(app: Path):
    """Ad-hoc signature: required on Apple Silicon, but not notarized."""
    if platform.system() == "Darwin":
        run(["codesign", "--force", "--deep", "--sign", "-", app])
    elif shutil.which("rcodesign"):
        run(["rcodesign", "sign", app])
    else:
        print("warning: rcodesign not found (cargo install apple-codesign), the app is not signed")


def zip_app(app: Path) -> Path:
    archive = app.with_suffix(".zip")
    if platform.system() == "Darwin":
        run(["ditto", "-c", "-k", "--keepParent", app, archive])
        return archive
    # Store Unix permissions so the executable stays executable after unzipping on a Mac.
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as z:
        for path in sorted(app.rglob("*")):
            rel = path.relative_to(app.parent).as_posix()
            if path.is_dir():
                info = zipfile.ZipInfo(rel + "/")
                info.external_attr = (0o40755 << 16) | 0x10
                z.writestr(info, "")
                continue
            info = zipfile.ZipInfo.from_file(path, rel)
            mode = 0o755 if path.parent.name == "MacOS" else 0o644
            info.external_attr = (0o100000 | mode) << 16
            info.create_system = 3  # Unix
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, path.read_bytes())
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    what = parser.add_mutually_exclusive_group(required=True)
    what.add_argument("--example", help="build this example")
    what.add_argument("--bin", help="build this binary")
    parser.add_argument("--name", required=True, help='app name, e.g. "My Game"')
    parser.add_argument("--id", help="bundle identifier (default: org.audiocode.<name>)")
    parser.add_argument("--version", default="0.1.0")
    parser.add_argument("--min-macos", default="11.7", help="oldest macOS version (default 11.7, the oldest zig supports)")
    parser.add_argument("--features", help="cargo features to enable")
    parser.add_argument("--resources", nargs="*", default=[], help="files or folders to ship, e.g. sounds")
    parser.add_argument("--out", default="target/macos", help="output folder")
    args = parser.parse_args()
    args.id = args.id or "org.audiocode." + "".join(c for c in args.name.lower() if c.isalnum())

    root = Path(__file__).resolve().parent.parent
    binary = build(args, root)
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    app = make_bundle(args, binary, out)
    sign(app)
    archive = zip_app(app)
    print(f"\n{app}\n{archive}")


if __name__ == "__main__":
    main()
