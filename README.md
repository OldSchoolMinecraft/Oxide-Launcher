# Oxide Launcher

A native Rust + Iced desktop launcher for [Oxide](https://github.com/LeowFlow/Oxide), a Rust port of Minecraft Beta 1.7.3. The UI closely follows the classic OLauncher shell: small **Update Notes**, **Launcher Log**, and **Profile Editor** tabs; a textured news page with a 180-pixel links sidebar; and a compact system-gray bottom bar split into three equal columns. The default client area is 900 × 550 (approximately the original 900 × 580 outer window). All controls remain native Rust/Iced; no Java launcher code or Microsoft sign-in is included. Dark mode applies to the launcher log; the Profile Editor matches the light gray bottom bar; the classic shell stays system gray and the news page stays textured in both modes.

## Run

Install a current stable Rust toolchain, then:

```sh
cargo run --release
```

Linux needs a graphical X11 or Wayland session and the usual system font and windowing libraries. Windows and macOS use their native window backends. The launcher uses Iced 0.14's GPU-accelerated wgpu renderer, with a software fallback if GPU initialization fails. Build natively on each platform with `cargo build --release`; the binary is `target/release/rust173-launcher` (`.exe` on Windows). On macOS this is an executable, not a signed `.app` bundle.

## Using the launcher

- Enter an offline player name and choose **Install & play**. Names use 1–16 ASCII letters, digits, or underscores.
- GitHub is checked on startup. Automatic updates are enabled by default and can be disabled in Profile Editor. **Check for updates** checks again; **Install latest** installs the available release. An installed version can still launch when GitHub is unreachable.
- Settings include an optional offline server, singleplayer server JAR, texture and sound paths, audio mute, smooth lighting, first-person animations, and window dimensions. Leave resource fields empty to use game defaults. Use absolute paths for resources.
- Choose to keep the launcher open, minimize it, or hide it while playing. Minimized/hidden launchers return when the child game process exits. Closing a visible launcher leaves the game running.
- To use a development build, put its absolute executable path in **Profile Editor → Local installation**. Managed installation updates are skipped at startup while this override is active.

The **Oxide News** release notes render Markdown headings, bold/italic text,
bulleted and numbered lists, inline/fenced code, quotes, tables and web links.
Click a link to open it in the system browser. Empty notes show a fallback message;
failed refreshes keep the last loaded notes. The news page keeps its classic dirt
background and readable text/link colors in both launcher themes.

Debug and diagnostic game flags are deliberately absent. Arguments are passed directly to the game process without a shell.

Settings are saved after edits and on window close, in the operating system's per-user configuration directory. Game installs, worlds, and `game.log` live in a separate per-user data directory shown in Profile Editor. The game starts in that directory's `game` subdirectory. The launcher passes that absolute path as `HYDRA_HOME`, so client options, worlds, texture packs and caches stay there across client updates (client v0.2.0+). Older manual-install saves are not migrated automatically; back them up and copy them into the launcher's game directory when needed. Logs are replaced each launch. Usernames and settings are only stored locally; release checks contact GitHub.

## Classic layout

The visual reference is the local OLauncher codebase (`X:\olauncher`), its base launcher's `LauncherTabPanel` and `BottomBarPanel`, and the [OLauncher news page](https://olauncher.github.io/news/). The layout and CSS colors are recreated with Rust widgets. At the user's request, the news background tiles the real classic launcher dirt texture. Its original 16 × 16 RGB pixels are embedded in `assets/dirt.rgb`; provenance is recorded in `assets/dirt.txt`. It remains third-party Mojang artwork.

The bottom-left username replaces the authenticated profile selector. **Edit Profile** opens all existing Rust launch options, **Check for updates** retains manual release discovery, and the right side shows the offline player and installed version. The bottom bar uses equal-width columns with consistent outer gutters, aligned username actions, a centered Play button, and right-aligned player/version details. The center Play control keeps the existing install, busy, and running states. The news sidebar contains GitHub, [Old School Minecraft](https://oldschoolminecraft.net), and [Discord](https://discord.gg/J3vKjD3). The bottom bar has no offline-play caption or project website button. There are no inactive account/profile controls. **Launcher Log** shows up to 256 session status and error messages; the game's existing `game.log` file remains separate. Settings persistence and save errors remain visible below the launch bar. The previously supplied `assets/logo-placeholder.svg` remains available but is not displayed in this layout.

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
- `src/ui.rs`: classic tabs, news, launch bar, log, and settings views.
- `src/chrome.rs`: dark/light palettes and shared classic surface styles.
- `src/settings.rs`: typed preferences, atomic JSON storage, and game arguments.
- `src/releases.rs`: GitHub discovery, platform matching, bounded download and extraction.
- `src/landscape.rs`: original launcher dirt texture tiled with Iced Canvas.

Tests cover release-note Markdown structure and styling, refresh/fallback handling, web-link validation, bounded launcher log history, settings replacement, exit-time saves, argument boundaries, game process start/exit, platform selection, executable discovery, and archive traversal. CI checks Linux, Windows, and macOS. Graphify's generated architecture map is local-only and ignored by Git.

To verify a locally prepared native game release with the launcher's own archive
reader, set `OXIDE_RELEASE_ARCHIVE` to the archive's absolute path and run
`cargo test --locked prepared_client_archive_matches_launcher_contract -- --ignored`.
