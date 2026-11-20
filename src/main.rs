#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod chrome;
mod landscape;
mod releases;
mod settings;
mod ui;
mod updater;

use iced::{Subscription, Task, Theme, window};
use releases::{Installed, Release};
use settings::{Animation, Lighting, Paths, Settings, Visibility};
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

fn main() -> iced::Result {
    if matches!(std::env::args().nth(1).as_deref(), Some("--version" | "-V")) {
        println!("oxide-launcher {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let (width, height) = Paths::new()
        .ok()
        .and_then(|paths| Settings::load(&paths.config).ok())
        .and_then(|settings| settings.launcher_size())
        .unwrap_or((900, 550));
    iced::application(App::new, App::update, App::view)
        .title("Oxide Launcher")
        .theme(App::theme)
        .subscription(App::subscription)
        .window(window::Settings {
            icon: Some(
                window::icon::from_rgba(
                    include_bytes!("../assets/icons/minecraft.rgba").to_vec(),
                    32,
                    32,
                )
                .expect("embedded 32x32 Minecraft icon"),
            ),
            #[cfg(target_os = "linux")]
            platform_specific: window::settings::PlatformSpecific {
                application_id: "dev.oxide.launcher".into(),
                ..Default::default()
            },
            size: iced::Size::new(width as f32, height as f32),
            min_size: Some(iced::Size::new(780., 480.)),
            ..Default::default()
        })
        .run()
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Play,
    Log,
    GameLog,
    Settings,
}
#[derive(Debug, Clone, Copy)]
enum Field {
    Username,
    Server,
    Singleplayer,
    Assets,
    Sounds,
    Executable,
    Width,
    Height,
    LauncherWidth,
    LauncherHeight,
}
#[derive(Debug, Clone)]
enum Message {
    Page(Page),
    Edit(Field, String),
    Visibility(Visibility),
    Lighting(Lighting),
    Animation(Animation),
    Mute(bool),
    AutoUpdate(bool),
    LauncherAutoUpdate(bool),
    LauncherChecked(Result<Option<Release>, String>),
    UpdateLauncher,
    LauncherUpdated(Result<String, String>),
    DarkMode(bool),
    Check,
    Checked(Result<Release, String>),
    Install(bool),
    Installed(Result<Installed, String>),
    Play,
    Tick,
    Window(Option<window::Id>),
    OpenProject,
    OpenOldSchoolMinecraft,
    OpenDiscord,
    OpenNewsLink(String),
}
struct App {
    paths: Result<Paths, String>,
    settings: Settings,
    page: Page,
    release: Option<Release>,
    release_notes: Vec<iced::widget::markdown::Item>,
    installed: Option<Installed>,
    busy: bool,
    status: String,
    launcher_log: std::collections::VecDeque<String>,
    error: Option<String>,
    save_error: Option<String>,
    dirty: Option<Instant>,
    child: Option<Child>,
    game_log_reader: Option<fs::File>,
    game_log: Vec<u8>,
    launcher_update: Option<Release>,
    launcher_updating: bool,
    launcher_checking: bool,
    launcher_updated: bool,
    launcher_update_status: String,
    window: Option<window::Id>,
    launch_after_install: bool,
    active_visibility: Visibility,
}
impl App {
    fn new() -> (Self, Task<Message>) {
        let paths = Paths::new();
        let loaded = paths
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|p| Settings::load(&p.config));
        let error = loaded.as_ref().err().cloned();
        let installed = paths.as_ref().ok().and_then(|p| Installed::load(&p.data));
        let mut app = Self {
            paths,
            settings: loaded.unwrap_or_default(),
            page: Page::Play,
            release: None,
            release_notes: ui::parse_release_notes(None),
            installed,
            busy: true,
            status: "Checking GitHub for the latest release…".into(),
            launcher_log: std::collections::VecDeque::from([
                "[INFO] Oxide Launcher started. Checking for releases.".into(),
            ]),
            error,
            save_error: None,
            dirty: None,
            child: None,
            game_log_reader: None,
            game_log: Vec::new(),
            launcher_update: None,
            launcher_updating: false,
            launcher_checking: false,
            launcher_updated: false,
            launcher_update_status: String::new(),
            window: None,
            launch_after_install: false,
            active_visibility: Visibility::KeepOpen,
        };
        app.launcher_checking = true;
        app.launcher_update_status = "Checking for launcher updates…".into();
        (
            app,
            Task::batch([
                Task::perform(releases::latest(), Message::Checked),
                Task::perform(updater::latest(), Message::LauncherChecked),
                window::latest().map(Message::Window),
            ]),
        )
    }
    fn theme(&self) -> Theme {
        chrome::theme(self.settings.dark_mode && self.page != Page::Settings)
    }

    fn subscription(&self) -> Subscription<Message> {
        if self.child.is_some() || self.dirty.is_some() {
            iced::time::every(Duration::from_millis(250)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        }
    }
    fn save(&mut self) -> bool {
        match self
            .paths
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|p| self.settings.save(&p.config))
        {
            Ok(()) => {
                self.dirty = None;
                self.save_error = None;
                true
            }
            Err(e) => {
                self.save_error = Some(format!("Settings could not be saved: {e}"));
                self.dirty = None;
                false
            }
        }
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        let previous = (
            self.status.clone(),
            self.error.clone(),
            self.save_error.clone(),
            self.launcher_update_status.clone(),
        );
        let task = self.update_inner(message);
        for line in [
            (self.status != previous.0).then(|| format!("[INFO] {}", self.status)),
            (self.launcher_update_status != previous.3)
                .then(|| format!("[INFO] {}", self.launcher_update_status)),
            (self.error != previous.1)
                .then(|| self.error.as_ref().map(|e| format!("[ERROR] {e}")))
                .flatten(),
            (self.save_error != previous.2)
                .then(|| self.save_error.as_ref().map(|e| format!("[ERROR] {e}")))
                .flatten(),
        ]
        .into_iter()
        .flatten()
        {
            self.launcher_log.push_back(line);
        }
        while self.launcher_log.len() > 256 {
            self.launcher_log.pop_front();
        }
        task
    }
    fn update_inner(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Page(page) => {
                self.page = if page == Page::GameLog && self.child.is_none() {
                    Page::Play
                } else {
                    page
                };
                if self.dirty.is_some() {
                    self.save();
                }
            }
            Message::Window(id) => self.window = id,
            Message::Edit(field, value) => {
                *match field {
                    Field::Username => &mut self.settings.username,
                    Field::Server => &mut self.settings.server,
                    Field::Singleplayer => &mut self.settings.singleplayer_server,
                    Field::Assets => &mut self.settings.assets,
                    Field::Sounds => &mut self.settings.sounds,
                    Field::Executable => &mut self.settings.executable,
                    Field::Width => &mut self.settings.width,
                    Field::Height => &mut self.settings.height,
                    Field::LauncherWidth => &mut self.settings.launcher_width,
                    Field::LauncherHeight => &mut self.settings.launcher_height,
                } = value;
                self.dirty = Some(Instant::now());
            }
            Message::Visibility(v) => {
                self.settings.visibility = v;
                self.save();
            }
            Message::Lighting(v) => {
                self.settings.lighting = v;
                self.save();
            }
            Message::Animation(v) => {
                self.settings.animation = v;
                self.save();
            }
            Message::Mute(v) => {
                self.settings.mute = v;
                self.save();
            }
            Message::DarkMode(v) => {
                self.settings.dark_mode = v;
                self.save();
            }
            Message::AutoUpdate(v) => {
                self.settings.auto_update = v;
                self.save();
            }
            Message::LauncherAutoUpdate(enabled) => {
                self.settings.launcher_auto_update = enabled;
                self.save();
                if enabled {
                    return self.update_inner(Message::UpdateLauncher);
                }
            }
            Message::LauncherChecked(result) => {
                self.launcher_checking = false;
                match result {
                    Ok(Some(release)) => {
                        self.launcher_update_status =
                            format!("Launcher {} available.", release.tag_name);
                        self.launcher_update = Some(release);
                        if self.settings.launcher_auto_update {
                            return self.update_inner(Message::UpdateLauncher);
                        }
                    }
                    Ok(None) => {
                        self.launcher_update = None;
                        self.launcher_update_status = "No newer launcher release published.".into();
                    }
                    Err(error) => {
                        self.launcher_update_status =
                            format!("Launcher update check unavailable: {error}")
                    }
                }
            }
            Message::UpdateLauncher
                if !self.launcher_updating && !self.launcher_updated && self.child.is_none() =>
            {
                if let Some(release) = self.launcher_update.clone() {
                    self.launcher_updating = true;
                    self.launcher_update_status =
                        format!("Installing launcher {}…", release.tag_name);
                    return Task::perform(updater::install(release), Message::LauncherUpdated);
                }
            }
            Message::LauncherUpdated(result) => {
                self.launcher_updating = false;
                self.launcher_update_status = match result {
                    Ok(tag) => {
                        self.launcher_updated = true;
                        self.launcher_update = None;
                        format!("Launcher {tag} installed. Restart the launcher to use it.")
                    }
                    Err(error) => format!("Launcher update failed: {error}"),
                };
            }
            Message::Check if !self.busy => {
                self.busy = true;
                self.error = None;
                self.status = "Checking GitHub for the latest release…".into();
                let game = Task::perform(releases::latest(), Message::Checked);
                if !self.launcher_checking && !self.launcher_updating && !self.launcher_updated {
                    self.launcher_checking = true;
                    self.launcher_update_status = "Checking for launcher updates…".into();
                    return Task::batch([
                        game,
                        Task::perform(updater::latest(), Message::LauncherChecked),
                    ]);
                }
                return game;
            }
            Message::Checked(result) => {
                self.busy = false;
                match result {
                    Ok(release) => {
                        self.status = format!("Latest release: {}", release.tag_name);
                        let outdated = self
                            .installed
                            .as_ref()
                            .is_none_or(|i| i.tag != release.tag_name);
                        self.release_notes = ui::parse_release_notes(release.body.as_deref());
                        self.release = Some(release);
                        if outdated
                            && self.settings.auto_update
                            && self.child.is_none()
                            && self.settings.executable.trim().is_empty()
                        {
                            return self.update_inner(Message::Install(false));
                        }
                    }
                    Err(e) => {
                        self.status = "Release check unavailable".into();
                        self.error = Some(e);
                    }
                }
            }
            Message::Install(launch) if !self.busy && self.child.is_none() => {
                if let (Some(release), Ok(paths)) = (&self.release, &self.paths) {
                    self.busy = true;
                    self.error = None;
                    self.launch_after_install = launch;
                    self.status = format!("Downloading and installing {}…", release.tag_name);
                    return Task::perform(
                        releases::install(release.clone(), paths.data.clone()),
                        Message::Installed,
                    );
                }
            }
            Message::Installed(result) => {
                self.busy = false;
                match result {
                    Ok(installed) => {
                        self.status = format!("{} is ready to play", installed.tag);
                        self.installed = Some(installed);
                        if self.launch_after_install {
                            self.launch_after_install = false;
                            return self.launch();
                        }
                    }
                    Err(e) => {
                        self.launch_after_install = false;
                        self.status = "Installation failed; previous version kept".into();
                        self.error = Some(e);
                    }
                }
            }
            Message::Play if self.child.is_none() && !self.busy && !self.launcher_updating => {
                if self.settings.executable.trim().is_empty() && self.installed.is_none() {
                    return self.update_inner(Message::Install(true));
                }
                return self.launch();
            }
            Message::Tick => {
                if self
                    .dirty
                    .is_some_and(|t| t.elapsed() >= Duration::from_millis(500))
                {
                    self.save();
                }
                self.read_game_log(false);
                if let Some(child) = &mut self.child {
                    match child.try_wait() {
                        Ok(Some(exit)) => {
                            self.child = None;
                            self.finish_game_log();
                            self.status = if exit.success() {
                                "Game closed.".into()
                            } else {
                                format!(
                                    "Game exited with {exit}. See game.log in the launcher data directory."
                                )
                            };
                            let restore = self.restore();
                            if self.settings.launcher_auto_update {
                                return Task::batch([
                                    restore,
                                    self.update_inner(Message::UpdateLauncher),
                                ]);
                            }
                            return restore;
                        }
                        Err(e) => {
                            self.error = Some(format!("Could not monitor game: {e}"));
                            self.child = None;
                            self.finish_game_log();
                            return self.restore();
                        }
                        Ok(None) => {}
                    }
                }
            }
            Message::OpenOldSchoolMinecraft => {
                let _ = open_url("https://os-mc.net");
            }
            Message::OpenDiscord => {
                let _ = open_url("https://discord.gg/J3vKjD3");
            }
            Message::OpenProject => {
                let _ = open_url("https://github.com/LeowFlow/Oxide");
            }
            Message::OpenNewsLink(link) => {
                if let Some(url) = news_url(&link)
                    && let Err(error) = open_url(url.as_str())
                {
                    self.error = Some(format!("Could not open news link: {error}"));
                }
            }
            _ => {}
        }
        Task::none()
    }
    fn launch(&mut self) -> Task<Message> {
        if self.launcher_updating {
            return Task::none();
        }
        let result = (|| -> Result<(Child, fs::File), String> {
            let args = self.settings.arguments()?;
            let paths = self.paths.as_ref().map_err(Clone::clone)?;
            fs::create_dir_all(&paths.data).map_err(|e| e.to_string())?;
            let executable = if self.settings.executable.trim().is_empty() {
                paths.data.join(
                    &self
                        .installed
                        .as_ref()
                        .ok_or("Install a release first")?
                        .executable,
                )
            } else {
                PathBuf::from(self.settings.executable.trim())
            };
            if !executable.is_absolute() || !executable.is_file() {
                return Err(
                    "Choose an existing absolute executable path in Profile Editor.".into(),
                );
            }
            let game_dir = paths.data.join("game");
            fs::create_dir_all(&game_dir).map_err(|e| e.to_string())?;
            let log = fs::File::create(paths.data.join("game.log")).map_err(|e| e.to_string())?;
            let reader = fs::File::open(paths.data.join("game.log")).map_err(|e| e.to_string())?;
            let child = game_command(&executable)
                .args(args)
                .env("HYDRA_HOME", &game_dir)
                .current_dir(&game_dir)
                .stdin(Stdio::null())
                .stdout(log.try_clone().map_err(|e| e.to_string())?)
                .stderr(log)
                .spawn()
                .map_err(|e| format!("Could not start {}: {e}", executable.display()))?;
            Ok((child, reader))
        })();
        match result {
            Ok((child, reader)) => {
                self.save();
                self.child = Some(child);
                self.game_log.clear();
                self.game_log_reader = Some(reader);
                self.error = None;
                self.status = "Oxide is running.".into();
                self.active_visibility = self.settings.visibility;
                if let Some(id) = self.window {
                    return match self.active_visibility {
                        Visibility::KeepOpen => Task::none(),
                        Visibility::Minimize => window::minimize(id, true),
                        Visibility::Hide => window::set_mode(id, window::Mode::Hidden),
                    };
                }
            }
            Err(e) => self.error = Some(e),
        }
        Task::none()
    }
    fn read_game_log(&mut self, finished: bool) {
        use std::io::Read;
        if let Some(reader) = &mut self.game_log_reader {
            let limit = if finished { u64::MAX } else { 256 * 1024 };
            if let Err(error) = reader.take(limit).read_to_end(&mut self.game_log) {
                self.error = Some(format!("Could not read game.log: {error}"));
                self.game_log_reader = None;
            }
        }
    }
    fn finish_game_log(&mut self) {
        self.read_game_log(true);
        self.game_log_reader = None;
        if self.page == Page::GameLog {
            self.page = Page::Play;
        }
    }
    fn restore(&self) -> Task<Message> {
        if let Some(id) = self.window {
            match self.active_visibility {
                Visibility::KeepOpen => Task::none(),
                Visibility::Minimize => window::minimize(id, false),
                Visibility::Hide => window::set_mode(id, window::Mode::Windowed),
            }
        } else {
            Task::none()
        }
    }
}
fn game_command(executable: &std::path::Path) -> Command {
    let command = Command::new(executable);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut command = command;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }
    #[cfg(not(target_os = "windows"))]
    command
}

fn news_url(link: &str) -> Option<reqwest::Url> {
    reqwest::Url::parse(link)
        .ok()
        .filter(|url| matches!(url.scheme(), "https" | "http"))
}
fn open_url(url: &str) -> std::io::Result<Child> {
    #[cfg(target_os = "windows")]
    {
        Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Command::new("xdg-open").arg(url).spawn()
    }
}

// Flush text edits even if the window closes before the debounce timer fires.
impl Drop for App {
    fn drop(&mut self) {
        if self.dirty.is_some() {
            self.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checked_release_refreshes_markdown_and_empty_notes_keep_a_fallback() {
        use iced::widget::markdown::Item;
        let root = tempfile::tempdir().unwrap();
        let mut app = isolated_app(root.path());
        app.settings.auto_update = false;
        let _ = app.update(Message::Checked(Ok(Release {
            tag_name: "v0.2.0".into(),
            body: Some(
                "# Oxide v0.2.0\n\n**Available:** Windows\n\n- Bundled sounds\n- Textures".into(),
            ),
            assets: Vec::new(),
        })));
        assert!(matches!(app.release_notes[0], Item::Heading(..)));
        assert!(
            app.release_notes
                .iter()
                .any(|item| matches!(item, Item::List {bullets,..} if bullets.len()==2))
        );
        let _ = app.update(Message::Checked(Err("Offline".into())));
        assert!(matches!(app.release_notes[0], Item::Heading(..)));
        let _ = app.update(Message::Checked(Ok(Release {
            tag_name: "v0.2.1".into(),
            body: Some(" \n".into()),
            assets: Vec::new(),
        })));
        assert_eq!(app.release_notes.len(), 1);
        assert!(matches!(app.release_notes[0], Item::Paragraph(_)));
    }
    #[test]
    fn news_links_only_open_web_urls() {
        assert!(news_url("https://github.com/LeowFlow/Oxide").is_some());
        assert!(news_url("http://example.org/notes").is_some());
        for link in [
            "javascript:alert(1)",
            "file:///C:/test.exe",
            "mailto:test@example.org",
            "relative/path",
            "",
        ] {
            assert!(news_url(link).is_none());
        }
    }
    fn isolated_app(root: &std::path::Path) -> App {
        App {
            paths: Ok(Paths {
                config: root.join("settings.json"),
                data: root.join("data"),
            }),
            settings: Settings::default(),
            page: Page::Play,
            release: None,
            release_notes: ui::parse_release_notes(None),
            installed: None,
            busy: false,
            status: String::new(),
            launcher_log: Default::default(),
            error: None,
            save_error: None,
            dirty: None,
            child: None,
            game_log_reader: None,
            game_log: Vec::new(),
            launcher_update: None,
            launcher_updating: false,
            launcher_checking: false,
            launcher_updated: false,
            launcher_update_status: String::new(),
            window: None,
            launch_after_install: false,
            active_visibility: Visibility::KeepOpen,
        }
    }
    #[cfg(target_os = "windows")]
    #[test]
    fn game_process_starts_without_an_attached_console() {
        let output = game_command(&std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::console_probe",
                "--ignored",
                "--nocapture",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("no attached console"));
    }
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "Invoked by the Windows child-process regression test"]
    fn console_probe() {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetConsoleWindow() -> *mut std::ffi::c_void;
        }
        assert!(unsafe { GetConsoleWindow() }.is_null());
        println!("no attached console");
    }
    #[test]
    fn closing_flushes_username_edits() {
        let root = tempfile::tempdir().unwrap();
        {
            let mut app = isolated_app(root.path());
            let _ = app.update(Message::Edit(Field::Username, "Alex".into()));
        }
        assert_eq!(
            Settings::load(&root.path().join("settings.json"))
                .unwrap()
                .username,
            "Alex"
        );
    }
    #[cfg(unix)]
    #[test]
    fn launches_without_shell_and_observes_exit() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let game = root.path().join("fake game");
        fs::write(&game, "#!/bin/sh\nprintf '%s\\n' \"$@\"\nexit 0\n").unwrap();
        fs::set_permissions(&game, fs::Permissions::from_mode(0o755)).unwrap();
        let mut app = isolated_app(root.path());
        app.settings.executable = game.to_string_lossy().into_owned();
        app.settings.assets = "/some path/$(touch NEVER_EXECUTE)".into();
        let _ = app.update(Message::Play);
        assert!(app.child.is_some(), "{:?}", app.error);
        app.child.as_mut().unwrap().wait().unwrap();
        let _ = app.update(Message::Tick);
        assert!(app.child.is_none());
        assert!(app.status.contains("Game closed"));
        let log = fs::read_to_string(root.path().join("data/game.log")).unwrap();
        assert!(log.contains("--username\nHydra_Explorer"));
        assert!(log.contains("/some path/$(touch NEVER_EXECUTE)"));
    }
    #[test]
    fn launcher_log_records_failures_and_bounds_session_history() {
        let root = tempfile::tempdir().unwrap();
        let mut app = isolated_app(root.path());
        for index in 0..300 {
            let _ = app.update(Message::Checked(Err(format!("Offline check {index}"))));
        }
        assert_eq!(app.launcher_log.len(), 256);
        assert!(
            app.launcher_log
                .back()
                .unwrap()
                .contains("Offline check 299")
        );
        assert!(
            !app.launcher_log
                .iter()
                .any(|line| line.ends_with("Offline check 0"))
        );
        let _ = app.update(Message::Page(Page::Log));
        assert_eq!(app.launcher_log.len(), 256);
    }

    #[test]
    fn launcher_empty_feed_and_opt_out_leave_play_available() {
        let root = tempfile::tempdir().unwrap();
        let mut app = isolated_app(root.path());
        app.settings.launcher_auto_update = false;
        let _ = app.update(Message::LauncherChecked(Ok(None)));
        assert!(!app.launcher_updating);
        assert!(app.error.is_none());
        let _ = app.update(Message::LauncherChecked(Ok(Some(Release {
            tag_name: "v99.0.0".into(),
            body: None,
            assets: vec![],
        }))));
        assert!(app.launcher_update.is_some());
        assert!(!app.launcher_updating);
        let _ = app.update(Message::UpdateLauncher);
        assert!(app.launcher_updating);
        let _ = app.update(Message::LauncherUpdated(Ok("v99.0.0".into())));
        assert!(!app.launcher_updating);
        assert!(app.launcher_updated);
        assert!(app.launcher_update_status.contains("Restart"));
    }
    #[cfg(unix)]
    #[test]
    fn game_log_is_live_includes_partial_output_and_closes_with_client() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let game = root.path().join("fake-client");
        fs::write(&game, "#!/bin/sh\nprintf 'hello \\342\\230\\203'\nprintf 'stderr' >&2\ncount=0\nwhile [ ! -f done ] && [ $count -lt 500 ]; do sleep 0.01; count=$((count+1)); done\nprintf 'final'\n").unwrap();
        fs::set_permissions(&game, fs::Permissions::from_mode(0o755)).unwrap();
        let mut app = isolated_app(root.path());
        app.settings.executable = game.to_string_lossy().into_owned();
        let _ = app.update(Message::Page(Page::GameLog));
        assert_eq!(app.page, Page::Play);
        let _ = app.update(Message::Play);
        let _ = app.update(Message::Page(Page::GameLog));
        assert_eq!(app.page, Page::GameLog);
        let _ = app.update(Message::LauncherChecked(Ok(Some(Release {
            tag_name: "v99.0.0".into(),
            body: None,
            assets: vec![],
        }))));
        assert!(
            !app.launcher_updating,
            "Updates must wait for the running client"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while !String::from_utf8_lossy(&app.game_log).contains("stderr") {
            let _ = app.update(Message::Tick);
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(String::from_utf8_lossy(&app.game_log).contains("hello ☃"));
        assert!(app.child.is_some());
        fs::write(root.path().join("data/game/done"), "").unwrap();
        app.child.as_mut().unwrap().wait().unwrap();
        let _ = app.update(Message::Tick);
        assert!(app.child.is_none());
        assert_eq!(app.page, Page::Play);
        assert!(String::from_utf8_lossy(&app.game_log).ends_with("final"));
        assert!(
            app.launcher_updating,
            "The pending update starts after client exit"
        );
        assert_eq!(
            app.game_log,
            fs::read(root.path().join("data/game.log")).unwrap()
        );
    }
    #[test]
    fn failed_launch_keeps_launcher_available() {
        let root = tempfile::tempdir().unwrap();
        let mut app = isolated_app(root.path());
        app.settings.executable = root.path().join("missing").to_string_lossy().into_owned();
        let _ = app.update(Message::Play);
        assert!(app.child.is_none());
        assert!(app.error.is_some());
        assert!(!app.busy);
    }
}
