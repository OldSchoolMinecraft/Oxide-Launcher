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
