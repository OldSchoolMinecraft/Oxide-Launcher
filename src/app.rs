use crate::{
    chrome,
    game::Game,
    github::Release,
    news, releases,
    releases::Installed,
    settings::{Lighting, Paths, Settings, Visibility},
    skin, updater,
};
use iced::{Subscription, Task, Theme, window};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    News,
    Log,
    GameLog,
    Settings,
    Skin,
}
#[derive(Debug, Clone, Copy)]
pub(crate) enum Field {
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
pub(crate) enum Message {
    Page(Page),
    Edit(Field, String),
    Visibility(Visibility),
    Lighting(Lighting),
    Mute(bool),
    AutoUpdate(bool),
    LauncherAutoUpdate(bool),
    LauncherChecked(Result<Option<Release>, String>),
    UpdateLauncher,
    LauncherUpdated(Result<String, String>),
    DarkMode(bool),
    DarkTheme(bool),
    Check,
    Checked(Result<Release, String>),
    HistoryChecked(Result<Vec<releases::Note>, String>),
    Install,
    Installed(Result<Installed, String>),
    Play,
    Tick,
    Window(Option<window::Id>),
    OpenLink(String),
    Skin(skin::Msg),
}
#[derive(Default)]
enum GameUpdate {
    #[default]
    Idle,
    Checking,
    Installing {
        play: bool,
    },
}

#[derive(Default)]
enum LauncherUpdate {
    #[default]
    Idle,
    Available(Release),
    Installing(Release),
    Installed,
}

pub(crate) struct App {
    pub paths: Result<Paths, String>,
    pub settings: Settings,
    pub page: Page,
    pub release: Option<Release>,
    pub release_notes: Vec<iced::widget::markdown::Item>,
    pub release_history: Vec<news::Entry>,
    pub history_status: news::HistoryStatus,
    pub installed: Option<Installed>,
    pub status: String,
    pub launcher_log: VecDeque<String>,
    pub error: Option<String>,
    pub save_error: Option<String>,
    pub game: Option<Game>,
    pub game_log: Vec<u8>,
    pub launcher_update_status: String,
    pub skin: skin::Editor,
    dirty_since: Option<Instant>,
    game_update: GameUpdate,
    launcher_update: LauncherUpdate,
    launcher_checking: bool,
    window: Option<window::Id>,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let mut app = Self::with_paths(Paths::new());
        app.launcher_log
            .push_back("[INFO] Oxide Launcher started. Checking for releases.".into());
        let check = app.check();
        let skin = app
            .skin
            .start(app.settings.player_name())
            .map(Message::Skin);
        (
            app,
            Task::batch([check, skin, window::latest().map(Message::Window)]),
        )
    }

    fn with_paths(paths: Result<Paths, String>) -> Self {
        let settings = paths
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|paths| Settings::load(&paths.config));
        let error = settings.as_ref().err().cloned();
        let installed = paths
            .as_ref()
            .ok()
            .and_then(|paths| Installed::load(&paths.data));
        let settings = settings.unwrap_or_default();
        let skin = skin::Editor::new(paths.as_ref().ok().map(|paths| paths.data.as_path()))
            .with_source(settings.skin_source);
        Self {
            paths,
            skin,
            settings,
            page: Page::News,
            release: None,
            release_notes: news::parse(None),
            release_history: Vec::new(),
            history_status: news::HistoryStatus::Idle,
            installed,
            status: String::new(),
            launcher_log: VecDeque::new(),
            error,
            save_error: None,
            game: None,
            game_log: Vec::new(),
            launcher_update_status: String::new(),
            dirty_since: None,
            game_update: GameUpdate::Idle,
            launcher_update: LauncherUpdate::Idle,
            launcher_checking: false,
            window: None,
        }
    }

    pub fn theme(&self) -> Theme {
        chrome::theme(self.settings.dark_theme)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let tick = if self.game.is_some() || self.settings_dirty() {
            iced::time::every(Duration::from_millis(250)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        };
        let frames = if self.page == Page::Skin && self.game.is_none() && self.skin.animating() {
            iced::time::every(Duration::from_millis(16))
                .map(|now| Message::Skin(skin::Msg::Frame(now)))
        } else {
            Subscription::none()
        };
        // Undo and redo shortcuts, unless a text box used the key press.
        let keys = if self.page == Page::Skin {
            iced::event::listen_with(|event, status, _| match (event, status) {
                (
                    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                        key: iced::keyboard::Key::Character(key),
                        modifiers,
                        ..
                    }),
                    iced::event::Status::Ignored,
                ) if modifiers.command() => match (key.as_str(), modifiers.shift()) {
                    ("z" | "Z", false) => Some(Message::Skin(skin::Msg::Undo)),
                    ("y" | "Y", _) | ("z" | "Z", true) => Some(Message::Skin(skin::Msg::Redo)),
                    _ => None,
                },
                _ => None,
            })
        } else {
            Subscription::none()
        };
        Subscription::batch([tick, frames, keys])
    }

    pub fn settings_dirty(&self) -> bool {
        self.dirty_since.is_some()
    }

    pub fn busy(&self) -> bool {
        !matches!(self.game_update, GameUpdate::Idle)
    }

    pub fn launcher_updating(&self) -> bool {
        matches!(self.launcher_update, LauncherUpdate::Installing(_))
    }

    pub fn can_update_launcher(&self) -> bool {
        self.game.is_none() && matches!(self.launcher_update, LauncherUpdate::Available(_))
    }

    pub fn has_game_update(&self) -> bool {
        self.release.as_ref().is_some_and(|release| {
            self.installed
                .as_ref()
                .is_none_or(|installed| installed.tag != release.tag_name)
        })
    }

    pub fn can_play(&self) -> bool {
        self.settings.player_name().is_some()
            && !self.launcher_updating()
            && !self.busy()
            && self.game.is_none()
            && (self.installed.is_some() || self.local_game() || self.release.is_some())
    }

    fn local_game(&self) -> bool {
        !self.settings.executable.trim().is_empty()
    }

    fn save(&mut self) {
        self.dirty_since = None;
        self.save_error = self
            .paths
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|paths| self.settings.save(&paths.config))
            .err()
            .map(|error| format!("Settings could not be saved: {error}"));
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let status = self.status.clone();
        let launcher_status = self.launcher_update_status.clone();
        let error = self.error.clone();
        let save_error = self.save_error.clone();
        let task = self.handle(message);

        for (previous, current) in [
            (status.as_str(), self.status.as_str()),
            (
                launcher_status.as_str(),
                self.launcher_update_status.as_str(),
            ),
        ] {
            if previous != current {
                self.launcher_log.push_back(format!("[INFO] {current}"));
            }
        }
        for (previous, current) in [(&error, &self.error), (&save_error, &self.save_error)] {
            if previous != current
                && let Some(error) = current
            {
                self.launcher_log.push_back(format!("[ERROR] {error}"));
            }
        }
        while self.launcher_log.len() > 256 {
            self.launcher_log.pop_front();
        }
        task
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Page(page) => {
                self.page = if page == Page::GameLog && self.game.is_none() {
                    Page::News
                } else {
                    page
                };
                if self.settings_dirty() {
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
                self.dirty_since = Some(Instant::now());
                if matches!(field, Field::Username) {
                    return self
                        .skin
                        .username_changed(self.settings.player_name())
                        .map(Message::Skin);
                }
            }
            Message::Visibility(visibility) => {
                self.settings.visibility = visibility;
                self.save();
            }
            Message::Lighting(lighting) => {
                self.settings.lighting = lighting;
                self.save();
            }
            Message::Mute(enabled) => {
                self.settings.mute = enabled;
                self.save();
            }
            Message::DarkMode(enabled) => {
                self.settings.dark_mode = enabled;
                self.save();
            }
            Message::DarkTheme(enabled) => {
                self.settings.dark_theme = enabled;
                self.save();
            }
            Message::AutoUpdate(enabled) => {
                self.settings.auto_update = enabled;
                self.save();
            }
            Message::LauncherAutoUpdate(enabled) => {
                self.settings.launcher_auto_update = enabled;
                self.save();
                if enabled {
                    return self.update_launcher();
                }
            }
            Message::LauncherChecked(result) => return self.launcher_checked(result),
            Message::UpdateLauncher => return self.update_launcher(),
            Message::LauncherUpdated(result) => self.launcher_updated(result),
            Message::Check if !self.busy() => {
                self.error = None;
                return self.check();
            }
            Message::Checked(result) => return self.checked(result),
            Message::HistoryChecked(result) => match result {
                Ok(notes) => {
                    self.release_history = news::entries(notes);
                    self.history_status = news::HistoryStatus::Idle;
                }
                Err(_) => self.history_status = news::HistoryStatus::Unavailable,
            },
            Message::Install if !self.busy() && self.game.is_none() => {
                return self.install(false);
            }
            Message::Installed(result) => return self.installed(result),
            Message::Play if self.can_play() => {
                if !self.local_game() && self.installed.is_none() {
                    return self.install(true);
                }
                return self.launch();
            }
            Message::Tick => {
                if self
                    .dirty_since
                    .is_some_and(|since| since.elapsed() >= Duration::from_millis(500))
                {
                    self.save();
                }
                return self.poll_game();
            }
            Message::OpenLink(link) => {
                if let Err(error) = news::open_link(&link) {
                    self.error = Some(format!("Could not open news link: {error}"));
                }
            }
            Message::Skin(message) => {
                if let skin::Msg::Source(source) = message {
                    self.settings.skin_source = source;
                    self.save();
                }
                let username = self.settings.player_name().map(str::to_owned);
                return self.skin.update(message, username).map(Message::Skin);
            }
            Message::Check | Message::Install | Message::Play => {}
        }
        Task::none()
    }

    fn check(&mut self) -> Task<Message> {
        self.game_update = GameUpdate::Checking;
        self.status = "Checking GitHub for the latest release…".into();
        let mut checks = vec![Task::perform(releases::latest(), Message::Checked)];
        if self.history_status != news::HistoryStatus::Loading {
            self.history_status = news::HistoryStatus::Loading;
            checks.push(Task::perform(releases::history(), Message::HistoryChecked));
        }
        if !self.launcher_checking
            && !matches!(
                self.launcher_update,
                LauncherUpdate::Installing(_) | LauncherUpdate::Installed
            )
        {
            self.launcher_checking = true;
            self.launcher_update_status = "Checking for launcher updates…".into();
            checks.push(Task::perform(updater::latest(), Message::LauncherChecked));
        }
        Task::batch(checks)
    }

    fn checked(&mut self, result: Result<Release, String>) -> Task<Message> {
        self.game_update = GameUpdate::Idle;
        match result {
            Ok(release) => {
                self.status = format!("Latest release: {}", release.tag_name);
                self.release_notes = news::parse(release.body.as_deref());
                self.release = Some(release);
                if self.settings.auto_update
                    && self.game.is_none()
                    && !self.local_game()
                    && self.has_game_update()
                {
                    return self.install(false);
                }
            }
            Err(error) => {
                self.status = "Release check unavailable".into();
                self.error = Some(error);
            }
        }
        Task::none()
    }

    fn install(&mut self, play: bool) -> Task<Message> {
        let (Some(release), Ok(paths)) = (&self.release, &self.paths) else {
            return Task::none();
        };
        self.game_update = GameUpdate::Installing { play };
        self.error = None;
        self.status = format!("Downloading and installing {}…", release.tag_name);
        Task::perform(
            releases::install(release.clone(), paths.data.clone()),
            Message::Installed,
        )
    }

    fn installed(&mut self, result: Result<Installed, String>) -> Task<Message> {
        let play = matches!(self.game_update, GameUpdate::Installing { play: true });
        self.game_update = GameUpdate::Idle;
        match result {
            Ok(installed) => {
                self.status = format!("{} is ready to play", installed.tag);
                self.installed = Some(installed);
                if play {
                    return self.launch();
                }
            }
            Err(error) => {
                self.status = "Installation failed; previous version kept".into();
                self.error = Some(error);
            }
        }
        Task::none()
    }

    fn launcher_checked(&mut self, result: Result<Option<Release>, String>) -> Task<Message> {
        self.launcher_checking = false;
        if matches!(
            self.launcher_update,
            LauncherUpdate::Installing(_) | LauncherUpdate::Installed
        ) {
            return Task::none();
        }
        match result {
            Ok(Some(release)) => {
                self.launcher_update_status = format!("Launcher {} available.", release.tag_name);
                self.launcher_update = LauncherUpdate::Available(release);
                if self.settings.launcher_auto_update {
                    return self.update_launcher();
                }
            }
            Ok(None) => {
                self.launcher_update = LauncherUpdate::Idle;
                self.launcher_update_status = "No newer launcher release published.".into();
            }
            Err(error) => {
                self.launcher_update_status = format!("Launcher update check unavailable: {error}");
            }
        }
        Task::none()
    }

    fn update_launcher(&mut self) -> Task<Message> {
        if self.game.is_none()
            && let LauncherUpdate::Available(release) = &self.launcher_update
        {
            let release = release.clone();
            self.launcher_update_status = format!("Installing launcher {}…", release.tag_name);
            self.launcher_update = LauncherUpdate::Installing(release.clone());
            return Task::perform(updater::install(release), Message::LauncherUpdated);
        }
        Task::none()
    }

    fn launcher_updated(&mut self, result: Result<String, String>) {
        match result {
            Ok(tag) => {
                self.launcher_update = LauncherUpdate::Installed;
                self.launcher_update_status =
                    format!("Launcher {tag} installed. Restart the launcher to use it.");
            }
            Err(error) => {
                if let LauncherUpdate::Installing(release) =
                    std::mem::take(&mut self.launcher_update)
                {
                    self.launcher_update = LauncherUpdate::Available(release);
                }
                self.launcher_update_status = format!("Launcher update failed: {error}");
            }
        }
    }

    fn launch(&mut self) -> Task<Message> {
        if self.launcher_updating() {
            return Task::none();
        }

        let result = self.paths.as_ref().map_err(Clone::clone).and_then(|paths| {
            let installed = self
                .installed
                .as_ref()
                .map(|installed| installed.executable.as_path());
            Game::start(&self.settings, &paths.data, installed)
        });
        match result {
            Ok(game) => {
                self.save();
                self.game_log.clear();
                self.error = None;
                self.status = "Oxide is running.".into();
                let hide = game.apply_visibility(self.window, true);
                self.game = Some(game);
                hide
            }
            Err(error) => {
                self.error = Some(error);
                Task::none()
            }
        }
    }

    fn poll_game(&mut self) -> Task<Message> {
        let Some(game) = &mut self.game else {
            return Task::none();
        };
        let exit = game.child.try_wait().transpose();
        let limit = if exit.is_some() { u64::MAX } else { 256 * 1024 };
        if let Err(error) = game.read_log(&mut self.game_log, limit) {
            self.error = Some(format!("Could not read game.log: {error}"));
        }
        let Some(exit) = exit else {
            return Task::none();
        };
        let restore = game.apply_visibility(self.window, false);
        self.game = None;
        if self.page == Page::GameLog {
            self.page = Page::News;
        }
        match exit {
            Ok(exit) => {
                self.status = if exit.success() {
                    "Game closed.".into()
                } else {
                    format!("Game exited with {exit}. See game.log in the launcher data directory.")
                };
                if self.settings.launcher_auto_update {
                    return Task::batch([restore, self.update_launcher()]);
                }
            }
            Err(error) => self.error = Some(format!("Could not monitor game: {error}")),
        }
        restore
    }
}

// A pending edit may outlive the debounce timer when the window closes.
impl Drop for App {
    fn drop(&mut self) {
        if self.settings_dirty() {
            self.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::fs;
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
            published_at: None,
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
            published_at: None,
            assets: Vec::new(),
        })));
        assert_eq!(app.release_notes.len(), 1);
        assert!(matches!(app.release_notes[0], Item::Paragraph(_)));
    }

    #[test]
    fn release_history_updates_independently_and_survives_a_failed_refresh() {
        let root = tempfile::tempdir().unwrap();
        let mut app = isolated_app(root.path());
        let _ = app.update(Message::Check);
        let _ = app.update(Message::HistoryChecked(Ok(vec![releases::Note {
            tag_name: "v1.0.0".into(),
            published_at: None,
            body: Some("First release".into()),
        }])));
        assert!(app.busy());
        assert!(app.release.is_none());
        assert_eq!(app.release_history.len(), 1);
        let _ = app.update(Message::HistoryChecked(Err("Offline".into())));
        assert_eq!(app.release_history.len(), 1);
        assert!(app.history_status == news::HistoryStatus::Unavailable);
        assert!(app.error.is_none());
    }

    fn isolated_app(root: &std::path::Path) -> App {
        App::with_paths(Ok(Paths {
            config: root.join("settings.json"),
            data: root.join("data"),
        }))
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn game_process_starts_without_an_attached_console() {
        let output = crate::game::command(&std::env::current_exe().unwrap())
            .args([
                "--exact",
                "app::tests::console_probe",
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
    #[test]
    fn play_requires_a_valid_username() {
        let root = tempfile::tempdir().unwrap();
        let mut app = isolated_app(root.path());
        app.release = Some(Release {
            tag_name: "v1.0.1".into(),
            body: None,
            published_at: None,
            assets: Vec::new(),
        });

        assert!(app.settings.username.is_empty());
        for name in ["", "   ", "Alex Steve", "abcdefghijklmnopq"] {
            let _ = app.update(Message::Edit(Field::Username, name.into()));
            assert!(!app.can_play(), "{name:?}");
            let _ = app.update(Message::Play);
            assert!(!app.busy());
            assert!(app.game.is_none());
        }

        let _ = app.update(Message::Edit(Field::Username, " Alex_123 ".into()));
        assert_eq!(app.settings.player_name(), Some("Alex_123"));
        assert!(app.can_play());
        let _ = app.update(Message::Play);
        assert!(matches!(
            app.game_update,
            GameUpdate::Installing { play: true }
        ));

        let _ = app.update(Message::Edit(Field::Username, String::new()));
        let _ = app.update(Message::Installed(Ok(Installed {
            tag: "v1.0.1".into(),
            executable: "hydra-client".into(),
        })));
        assert!(app.installed.is_some());
        assert!(app.game.is_none());
        assert!(
            app.error
                .as_deref()
                .unwrap()
                .starts_with("Username must be")
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
        app.settings.username = "Alex".into();
        app.settings.executable = game.to_string_lossy().into_owned();
        app.settings.assets = "/some path/$(touch NEVER_EXECUTE)".into();
        let _ = app.update(Message::Play);
        assert!(app.game.is_some(), "{:?}", app.error);
        app.game.as_mut().unwrap().child.wait().unwrap();
        let _ = app.update(Message::Tick);
        assert!(app.game.is_none());
        assert!(app.status.contains("Game closed"));
        let log = fs::read_to_string(root.path().join("data/game.log")).unwrap();
        assert!(log.contains("--username\nAlex"));
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
        assert!(!app.launcher_updating());
        assert!(app.error.is_none());
        let _ = app.update(Message::LauncherChecked(Ok(Some(Release {
            tag_name: "v99.0.0".into(),
            body: None,
            published_at: None,
            assets: vec![],
        }))));
        assert!(app.can_update_launcher());
        assert!(!app.launcher_updating());
        let _ = app.update(Message::Check);
        let _ = app.update(Message::LauncherChecked(Err("Offline".into())));
        assert!(app.can_update_launcher());
        let _ = app.update(Message::UpdateLauncher);
        let _ = app.update(Message::LauncherUpdated(Err("File is locked".into())));
        assert!(app.can_update_launcher());
        assert!(app.launcher_update_status.contains("File is locked"));
        let _ = app.update(Message::UpdateLauncher);
        assert!(app.launcher_updating());
        let _ = app.update(Message::LauncherUpdated(Ok("v99.0.0".into())));
        assert!(!app.launcher_updating());
        assert!(matches!(app.launcher_update, LauncherUpdate::Installed));
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
        app.settings.username = "Alex".into();
        app.settings.executable = game.to_string_lossy().into_owned();
        let _ = app.update(Message::Page(Page::GameLog));
        assert_eq!(app.page, Page::News);
        let _ = app.update(Message::Play);
        let _ = app.update(Message::Page(Page::GameLog));
        assert_eq!(app.page, Page::GameLog);
        let _ = app.update(Message::LauncherChecked(Ok(Some(Release {
            tag_name: "v99.0.0".into(),
            body: None,
            published_at: None,
            assets: vec![],
        }))));
        assert!(
            !app.launcher_updating(),
            "Updates must wait for the running client"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while !String::from_utf8_lossy(&app.game_log).contains("stderr") {
            let _ = app.update(Message::Tick);
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(String::from_utf8_lossy(&app.game_log).contains("hello ☃"));
        assert!(app.game.is_some());
        fs::write(root.path().join("data/game/done"), "").unwrap();
        app.game.as_mut().unwrap().child.wait().unwrap();
        let _ = app.update(Message::Tick);
        assert!(app.game.is_none());
        assert_eq!(app.page, Page::News);
        assert!(String::from_utf8_lossy(&app.game_log).ends_with("final"));
        assert!(
            app.launcher_updating(),
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
        app.settings.username = "Alex".into();
        app.settings.executable = root.path().join("missing").to_string_lossy().into_owned();
        let _ = app.update(Message::Play);
        assert!(app.game.is_none());
        assert!(app.error.is_some());
        assert!(!app.busy());
    }
}
