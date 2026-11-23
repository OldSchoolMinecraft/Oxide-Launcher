use crate::settings::{Settings, Visibility};
use iced::{Task, window};
use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

pub(crate) struct Game {
    pub child: Child,
    log: Option<File>,
    visibility: Visibility,
}

impl Game {
    pub fn start(
        settings: &Settings,
        data: &Path,
        installed: Option<&Path>,
    ) -> Result<Self, String> {
        let args = settings.arguments()?;
        let executable = if settings.executable.trim().is_empty() {
            data.join(installed.ok_or("Install a release first")?)
        } else {
            PathBuf::from(settings.executable.trim())
        };
        if !executable.is_absolute() || !executable.is_file() {
            return Err("Choose an existing absolute executable path in Profile Editor.".into());
        }

        let directory = data.join("game");
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let log_path = data.join("game.log");
        let output = File::create(&log_path).map_err(|error| error.to_string())?;
        let log = File::open(&log_path).map_err(|error| error.to_string())?;
        let child = command(&executable)
            .args(args)
            .env("HYDRA_HOME", &directory)
            .current_dir(&directory)
            .stdin(Stdio::null())
            .stdout(output.try_clone().map_err(|error| error.to_string())?)
            .stderr(output)
            .spawn()
            .map_err(|error| format!("Could not start {}: {error}", executable.display()))?;
        Ok(Self {
            child,
            log: Some(log),
            visibility: settings.visibility,
        })
    }

    pub fn read_log(&mut self, output: &mut Vec<u8>, limit: u64) -> io::Result<()> {
        if let Some(log) = &mut self.log
            && let Err(error) = log.take(limit).read_to_end(output)
        {
            self.log = None;
            return Err(error);
        }
        Ok(())
    }

    pub fn apply_visibility<Message>(
        &self,
        id: Option<window::Id>,
        playing: bool,
    ) -> Task<Message> {
        match (id, self.visibility) {
            (Some(id), Visibility::Minimize) => window::minimize(id, playing),
            (Some(id), Visibility::Hide) => window::set_mode(
                id,
                if playing {
                    window::Mode::Hidden
                } else {
                    window::Mode::Windowed
                },
            ),
            _ => Task::none(),
        }
    }
}

pub(crate) fn command(executable: &Path) -> Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = Command::new(executable);
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }
    #[cfg(not(target_os = "windows"))]
    Command::new(executable)
}
