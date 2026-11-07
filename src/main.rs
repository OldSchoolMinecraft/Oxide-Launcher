#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod chrome;
mod landscape;
mod releases;
mod settings;
mod ui;

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
    iced::application(App::new, App::update, App::view)
        .title("Rust173 Launcher")
        .theme(App::theme)
        .subscription(App::subscription)
        .window(window::Settings {
            size: iced::Size::new(1060., 790.),
            min_size: Some(iced::Size::new(780., 650.)),
            ..Default::default()
        })
        .run()
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Play,
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
    DarkMode(bool),
    Check,
    Checked(Result<Release, String>),
    Install(bool),
    Installed(Result<Installed, String>),
    Play,
    Tick,
    Window(Option<window::Id>),
    OpenProject,
}
struct App {
    paths: Result<Paths, String>,
    settings: Settings,
    page: Page,
    release: Option<Release>,
    installed: Option<Installed>,
    busy: bool,
    status: String,
    error: Option<String>,
    save_error: Option<String>,
    dirty: Option<Instant>,
    child: Option<Child>,
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
        let app = Self {
            paths,
            settings: loaded.unwrap_or_default(),
            page: Page::Play,
            release: None,
            installed,
            busy: true,
            status: "Checking GitHub for the latest release…".into(),
            error,
            save_error: None,
            dirty: None,
            child: None,
            window: None,
            launch_after_install: false,
            active_visibility: Visibility::KeepOpen,
        };
        (
            app,
            Task::batch([
                Task::perform(releases::latest(), Message::Checked),
                window::latest().map(Message::Window),
            ]),
        )
    }
    fn theme(&self) -> Theme {
        chrome::theme(self.settings.dark_mode)
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
        match message {
            Message::Page(page) => {
                self.page = page;
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
            Message::Check if !self.busy => {
                self.busy = true;
                self.error = None;
                self.status = "Checking GitHub for the latest release…".into();
                return Task::perform(releases::latest(), Message::Checked);
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
                        self.release = Some(release);
                        if outdated
                            && self.settings.auto_update
                            && self.child.is_none()
                            && self.settings.executable.trim().is_empty()
                        {
                            return self.update(Message::Install(false));
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
            Message::Play if self.child.is_none() && !self.busy => {
                if self.settings.executable.trim().is_empty() && self.installed.is_none() {
                    return self.update(Message::Install(true));
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
                if let Some(child) = &mut self.child {
                    match child.try_wait() {
                        Ok(Some(exit)) => {
                            self.child = None;
                            self.status = if exit.success() {
                                "Game closed.".into()
                            } else {
                                format!(
                                    "Game exited with {exit}. See game.log in the launcher data directory."
                                )
                            };
                            return self.restore();
                        }
                        Err(e) => {
                            self.error = Some(format!("Could not monitor game: {e}"));
                            self.child = None;
                            return self.restore();
                        }
                        Ok(None) => {}
                    }
                }
            }
            Message::OpenProject => {
                let _ = open_url("https://github.com/LeowFlow/Rust173");
            }
            _ => {}
        }
        Task::none()
    }
    fn launch(&mut self) -> Task<Message> {
        let result = (|| -> Result<Child, String> {
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
                return Err("Choose an existing absolute executable path in Settings.".into());
            }
            let game_dir = paths.data.join("game");
            fs::create_dir_all(&game_dir).map_err(|e| e.to_string())?;
            let log = fs::File::create(paths.data.join("game.log")).map_err(|e| e.to_string())?;
            Command::new(&executable)
                .args(args)
                .current_dir(game_dir)
                .stdin(Stdio::null())
                .stdout(log.try_clone().map_err(|e| e.to_string())?)
                .stderr(log)
                .spawn()
                .map_err(|e| format!("Could not start {}: {e}", executable.display()))
        })();
        match result {
            Ok(child) => {
                self.save();
                self.child = Some(child);
                self.error = None;
                self.status = "Rust173 is running.".into();
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
    fn isolated_app(root: &std::path::Path) -> App {
        App {
            paths: Ok(Paths {
                config: root.join("settings.json"),
                data: root.join("data"),
            }),
            settings: Settings::default(),
            page: Page::Play,
            release: None,
            installed: None,
            busy: false,
            status: String::new(),
            error: None,
            save_error: None,
            dirty: None,
            child: None,
            window: None,
            launch_after_install: false,
            active_visibility: Visibility::KeepOpen,
        }
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
