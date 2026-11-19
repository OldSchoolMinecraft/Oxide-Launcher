# Native launcher packages

The launcher has its own version in Cargo.toml (currently 0.1.0), independent of
the Oxide game version. These packages contain the launcher and its embedded UI
assets; the game, textures and audio are installed separately through the launcher.

| Platform | Package |
| --- | --- |
| Windows x86_64 | `Oxide-Launcher-v0.1.0-windows-x86_64.zip` |
| Linux x86_64 | `Oxide-Launcher-v0.1.0-linux-x86_64.tar.gz` |
| Intel Mac | `Oxide-Launcher-v0.1.0-macos-x86_64.zip` |
| Apple Silicon | `Oxide-Launcher-v0.1.0-macos-aarch64.zip` |

Mac ZIPs contain `Oxide Launcher.app` with versioned Info.plist metadata and a
native executable. Native builds are ad-hoc signed and verified; no Developer ID
certificate or notarization is configured. Linux targets glibc 2.35+ and needs a
graphical X11/Wayland session, xkbcommon and system fonts. Mac builds target macOS
12+. Native CLI startup checks do not replace desktop/GPU testing on those devices.

## Build locally

Use Rust 1.97.1, Python 3.11+, and native platform build tools. On Windows use
PowerShell 7; on Linux/macOS use Bash:

```powershell
./scripts/Build-Packages.ps1
```

```sh
bash scripts/build-packages.sh
```

Both wrappers call package-build.py. Optional flags are `-Target`/`--target`,
`-OutputDir`/`--output-dir`, and `-SkipBuild`/`--skip-build`. Target and native
Rust host must match. The default output is `.local/builds/v<version>/<platform>/`.
Existing output directories are not overwritten.

Only **public/** contains downloadable archives and checksums. Keep stage/ and
provenance.private.json private. Packages never include personal settings, saved
worlds, installed game files, logs, source code or symbols. The launcher supports
`--version`/`-V` for a headless native smoke test without starting the GUI or
performing game installation/update requests.

## Native CI

The **Build launcher packages** workflow runs for `codex/launcher-build-*`
branches and supports manual dispatch when present on the default branch. It
builds Windows 2022, Ubuntu 22.04, Intel macOS 15 and Apple Silicon macOS 15
packages. Each job checks formatting, strict Clippy, optimized native tests,
archive tests, executable versions, native CPU headers and executable permissions.
Mac jobs also verify app code signatures.

The final collector requires all four packages and verifies checksums, app metadata,
CPU types and executable modes. The `oxide-launcher-packages` Actions artifact
contains the four platform archives and one SHA256SUMS.txt. Download and extract
the Actions transport ZIP to obtain the actual packages. Nothing is published
automatically. Normal CI still runs on ordinary branches and pull requests.

Keep launcher downloads separate from game assets in LeowFlow/Oxide: adding a
second platform-matching launcher archive to a game release would make the game's
asset selection ambiguous. The launcher updates the game, not itself.

Local archive regression tests: `python scripts/test-packaging.py`.

## Verified build (2026-11-15)

All four native build jobs and final collection passed in Actions run 43751006517,
from source commit ed886334a5b649d311d5cc1692b4a8664e751833. Formatting, strict
Clippy, optimized tests, archive tests and headless native startup checks passed.
Both Mac app bundles passed ad-hoc code-signature verification. Native archive
headers, permissions and hashes were checked again after download, and the
Windows executable's --version startup was verified locally.

Collected archives and SHA256SUMS.txt are in `.local/builds/v0.1.0/packages/`.
The private validation report is alongside that directory. No GitHub launcher
release was created or published. Mac/Linux desktop GUI validation remains open.

## Application icons

The classic Minecraft grass-block favicon from the supplied OLauncher reference
is used for the launcher. Assets and provenance are in `assets/icons/`; these
are third-party Mojang artwork. `python scripts/generate-icons.py` regenerates
the platform variants (requires Pillow only when regenerating; normal builds
and packaging use the checked-in assets and do not need Pillow).

- Windows: `build.rs` embeds the multi-size `.ico` resource in every native
  `.exe` build. The Windows SDK resource compiler is used through `winresource`.
- macOS: both Intel and Apple Silicon `.app` packages include
  `Contents/Resources/minecraft.icns`, referenced by `CFBundleIconFile` before
  code signing. Use the `.app` for Finder/Dock icons; a raw Mach-O binary does
  not carry a Finder app icon.
- Linux: the archive includes `oxide-launcher.png` and
  `dev.oxide.launcher.desktop`. The window application ID matches the desktop
  entry name. Desktop environments use these files for menu/taskbar icons;
  a raw ELF file does not have a portable embedded file-manager icon.

To install the Linux launcher and its desktop icon for the current user, run
these commands from the extracted archive. Ensure `~/.local/bin` is on the
PATH of your graphical desktop session:

```sh
install -Dm755 oxide-launcher "$HOME/.local/bin/oxide-launcher"
install -Dm644 dev.oxide.launcher.desktop "$HOME/.local/share/applications/dev.oxide.launcher.desktop"
install -Dm644 oxide-launcher.png "$HOME/.local/share/icons/hicolor/256x256/apps/dev.oxide.launcher.png"
```

The Iced window uses the original 32x32 RGBA pixels for its native window icon
(on platforms supporting window icons). Packaging tests require the Mac bundle
icon and metadata and the Linux desktop entry and icon. The existing four-platform
CI packages these files with each native build. New Mac/Linux desktop icon
appearance still needs a native graphical session to verify visually.

## Startup window sizes and Windows console behavior

The launcher reads `launcher_width` and `launcher_height` from its saved settings
before creating its window. These are independent of game `width`/`height`.
Older settings files receive launcher defaults of 900x550; invalid or partial
launcher-size edits also fall back to that size on the next start. Game arguments
default to `--width 854 --height 480`; existing saved game sizes are preserved.
The client continues to interpret these arguments using its own window/DPI handling.

Windows builds use the GUI subsystem for the launcher and `CREATE_NO_WINDOW` for
the game child process. The client is started directly with its argument list,
working directory, `HYDRA_HOME`, and redirected stdout/stderr preserved. The native
Windows child-process regression test checks that no console is attached.
