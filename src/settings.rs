use serde::{Deserialize, Serialize};
use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Visibility {
    #[default]
    KeepOpen,
    Minimize,
    Hide,
}
impl Visibility {
    pub const ALL: [Self; 3] = [Self::KeepOpen, Self::Minimize, Self::Hide];
}
impl fmt::Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::KeepOpen => "Keep launcher open",
            Self::Minimize => "Minimize while playing",
            Self::Hide => "Hide until game exits",
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Lighting {
    #[default]
    GameDefault,
    On,
    Off,
}
impl Lighting {
    pub const ALL: [Self; 3] = [Self::GameDefault, Self::On, Self::Off];
}
impl fmt::Display for Lighting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::GameDefault => "Use game setting",
            Self::On => "On",
            Self::Off => "Off",
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Animation {
    Vanilla,
    #[default]
    Hydra,
}
impl Animation {
    pub const ALL: [Self; 2] = [Self::Hydra, Self::Vanilla];
}
impl fmt::Display for Animation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Vanilla => "Vanilla",
            Self::Hydra => "Hydra",
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub username: String,
    pub server: String,
    pub singleplayer_server: String,
    pub assets: String,
    pub sounds: String,
    pub executable: String,
    pub width: String,
    pub height: String,
    pub mute: bool,
    pub lighting: Lighting,
    pub animation: Animation,
    pub visibility: Visibility,
    pub auto_update: bool,
    pub dark_mode: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            username: "Hydra_Explorer".into(),
            server: String::new(),
            singleplayer_server: String::new(),
            assets: String::new(),
            sounds: String::new(),
            executable: String::new(),
            width: "1280".into(),
            height: "800".into(),
            mute: false,
            lighting: Lighting::default(),
            animation: Animation::default(),
            visibility: Visibility::default(),
            auto_update: true,
            dark_mode: true,
        }
    }
}
impl Settings {
    pub fn load(path: &Path) -> Result<Self, String> {
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| format!("Could not read settings: {e}. Original file preserved.")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        save_json(path, self)
    }
    pub fn arguments(&self) -> Result<Vec<String>, String> {
        let name = self.username.trim();
        if name.is_empty()
            || name.len() > 16
            || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err("Username must be 1–16 letters, numbers, or underscores.".into());
        }
        for (label, value) in [("Width", &self.width), ("Height", &self.height)] {
            if !value.parse::<u32>().is_ok_and(|v| (64..=4096).contains(&v)) {
                return Err(format!("{label} must be between 64 and 4096."));
            }
        }
        let mut args = vec![
            "--username".into(),
            name.into(),
            "--width".into(),
            self.width.clone(),
            "--height".into(),
            self.height.clone(),
        ];
        for (flag, value) in [
            ("--server", &self.server),
            ("--singleplayer-server", &self.singleplayer_server),
            ("--assets", &self.assets),
            ("--sounds", &self.sounds),
        ] {
            if !value.trim().is_empty() {
                args.extend([flag.into(), value.trim().into()]);
            }
        }
        if self.mute {
            args.push("--mute".into());
        }
        if self.lighting != Lighting::GameDefault {
            args.extend([
                "--smooth-lighting".into(),
                (self.lighting == Lighting::On).to_string(),
            ]);
        }
        args.extend([
            "--animation-preset".into(),
            self.animation.to_string().to_lowercase(),
        ]);
        Ok(args)
    }
}
pub fn save_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let parent = path.parent().ok_or("Missing parent directory")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    use std::io::Write;
    temp.write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
#[derive(Clone)]
pub struct Paths {
    pub config: PathBuf,
    pub data: PathBuf,
}
impl Paths {
    pub fn new() -> Result<Self, String> {
        let dirs = directories::ProjectDirs::from("dev", "Oxide", "OxideLauncher")
            .ok_or("Could not locate application directories")?;
        Ok(Self {
            config: dirs.config_dir().join("settings.json"),
            data: dirs.data_local_dir().to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let s = Settings {
            username: "Steve".into(),
            visibility: Visibility::Hide,
            dark_mode: false,
            ..Settings::default()
        };
        s.save(&path).unwrap();
        s.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded.username, "Steve");
        assert_eq!(loaded.visibility, Visibility::Hide);
        assert!(!loaded.dark_mode);
    }
    #[test]
    fn old_settings_default_to_dark_mode() {
        let settings: Settings = serde_json::from_str(r#"{"username":"Steve"}"#).unwrap();
        assert!(settings.dark_mode);
        assert_eq!(settings.username, "Steve");
    }
    #[test]
    fn arguments_preserve_paths_and_negative_validation() {
        let mut s = Settings {
            assets: "/a path/game.zip".into(),
            ..Settings::default()
        };
        let a = s.arguments().unwrap();
        assert!(a.windows(2).any(|p| p == ["--assets", "/a path/game.zip"]));
        assert!(!a.iter().any(|x| x == "--fly"));
        s.width = "4097".into();
        assert!(s.arguments().is_err());
        s.width = "1280".into();
        s.username = "".into();
        assert!(s.arguments().is_err());
    }
}
