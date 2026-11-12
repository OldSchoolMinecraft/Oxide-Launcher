# Oxide Launcher

A native Rust + Iced desktop launcher for [Oxide](https://github.com/LeowFlow/Oxide), a Rust port of Minecraft Beta 1.7.3. A late-2000s desktop style uses glossy controls, beveled blue-gray panels, and an original block landscape. Dark mode is on by default; the light-mode option is saved in Settings.

## Run

Install a current stable Rust toolchain, then:

```sh
cargo run --release
```

Linux needs a graphical X11 or Wayland session and the usual system font and windowing libraries. Windows and macOS use their native window backends. The launcher uses Iced 0.14's GPU-accelerated wgpu renderer, with a software fallback if GPU initialization fails. Build natively on each platform with `cargo build --release`; the binary is `target/release/rust173-launcher` (`.exe` on Windows). On macOS this is an executable, not a signed `.app` bundle.

## Using the launcher

- Enter an offline player name and choose **Install & play**. Names use 1–16 ASCII letters, digits, or underscores.
- GitHub is checked on startup. Automatic updates are enabled by default and can be disabled in Settings. **Check for updates** checks again; **Install latest** installs the available release. An installed version can still launch when GitHub is unreachable.
- Settings include an optional offline server, singleplayer server JAR, texture and sound paths, audio mute, smooth lighting, first-person animations, and window dimensions. Leave resource fields empty to use game defaults. Use absolute paths for resources.
- Choose to keep the launcher open, minimize it, or hide it while playing. Minimized/hidden launchers return when the child game process exits. Closing a visible launcher leaves the game running.
- To use a development build, put its absolute executable path in **Settings → Local installation**. Managed installation updates are skipped at startup while this override is active.

Debug and diagnostic game flags are deliberately absent. Arguments are passed directly to the game process without a shell.

Settings are saved after edits and on window close, in the operating system's per-user configuration directory. Game installs, worlds, and `game.log` live in a separate per-user data directory shown in Settings. The game starts in that directory's `game` subdirectory. Logs are replaced each launch. Usernames and settings are only stored locally; release checks contact GitHub.

## Branding

The masthead reserves a logo area beside the project name. `assets/logo-placeholder.svg` is an original temporary block emblem, embedded at compile time. Replace that file with the final project mark and rebuild; its aspect ratio is preserved within a 78 × 78 display area. The wordmark and subtitle are defined in `src/ui.rs`.

## GitHub release contract

The launcher reads `https://api.github.com/repos/LeowFlow/Oxide/releases/latest`, selecting stable published releases. At implementation time, this endpoint returned 404 publicly; live downloads could not be verified. A missing/private repository or absent release is reported in the UI, and local executable mode remains usable.

Publish exactly one matching asset per supported OS/CPU. Names must identify `linux`, `windows`, or `macos`/`darwin`/`osx`, plus `x86_64`/`amd64`/`x64` or `aarch64`/`arm64`. For example:

```
rust173-x86_64-unknown-linux-gnu.tar.gz
rust173-x86_64-pc-windows-msvc.zip
rust173-aarch64-apple-darwin.tar.gz
```

Packages may be ZIP or gzip-compressed TAR, with one executable named `rust173`, `rust173-client`, or `hydra-client` (`.exe` for Windows), optionally in a nested folder. Include required runtime resources alongside it; the directory layout is preserved. A Windows `.exe` or extensionless platform-named binary is also supported. Source archives, installers, DMGs, AppImages, and ambiguous multiple platform matches are not selected. Shared library compatibility remains the responsibility of the game release build.

Downloads use HTTPS, enforce the GitHub-reported byte size and SHA-256 digest when GitHub supplies one, and are capped at 512 MiB compressed / 2 GiB unpacked / 20,000 entries. Archive traversal, links, and special files are rejected. Each install is staged into a new directory; the active install manifest changes only after success. Old versions remain available on disk for manual recovery. This updates the **game**, not the launcher binary.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `src/main.rs`: Iced state, persistence scheduling, and game lifecycle.
- `src/ui.rs`: Play and Settings views and logo placement.
- `src/chrome.rs`: dark/light palettes and shared glossy surface styles.
- `src/settings.rs`: typed preferences, atomic JSON storage, and game arguments.
- `src/releases.rs`: GitHub discovery, platform matching, bounded download and extraction.
- `src/landscape.rs`: original vector landscape rendered with Iced Canvas.

Tests cover settings replacement, exit-time saves, argument boundaries, game process start/exit, platform selection, executable discovery, and archive traversal. CI checks Linux, Windows, and macOS. Graphify's generated architecture map is local-only and ignored by Git.
