use serde::{Deserialize, Serialize};
use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

pub(crate) const DEFAULT_WINDOW_SIZE: (u32, u32) = (900, 550);
pub(crate) const MIN_WINDOW_SIZE: (u32, u32) = (780, 480);

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
/// Where the Skin Editor looks a username's skin and cape up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SkinSource {
    /// The Old School Minecraft cosmetics the Oxide client shows first.
    #[default]
    Osm,
    Mojang,
}
impl SkinSource {
    pub const ALL: [Self; 2] = [Self::Osm, Self::Mojang];
}
impl fmt::Display for SkinSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Osm => "OSM",
            Self::Mojang => "Mojang",
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
    pub launcher_width: String,
    pub launcher_height: String,
    pub mute: bool,
    pub lighting: Lighting,
    pub visibility: Visibility,
    pub auto_update: bool,
    pub launcher_auto_update: bool,
    pub dark_mode: bool,
    /// Dark chrome for the whole launcher; `dark_mode` covers only the log.
    pub dark_theme: bool,
    pub skin_source: SkinSource,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            username: String::new(),
            server: String::new(),
            singleplayer_server: String::new(),
            assets: String::new(),
            sounds: String::new(),
            executable: String::new(),
            width: "854".into(),
            height: "480".into(),
            launcher_width: DEFAULT_WINDOW_SIZE.0.to_string(),
            launcher_height: DEFAULT_WINDOW_SIZE.1.to_string(),
            mute: false,
            lighting: Lighting::default(),
            visibility: Visibility::default(),
            auto_update: true,
            launcher_auto_update: true,
            dark_mode: true,
            dark_theme: false,
            skin_source: SkinSource::default(),
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
    /// Invalid/incomplete edits fall back to the default on the next start.
    pub fn launcher_size(&self) -> Option<(u32, u32)> {
        let width = self.launcher_width.trim().parse().ok()?;
        let height = self.launcher_height.trim().parse().ok()?;
        ((MIN_WINDOW_SIZE.0..=4096).contains(&width)
            && (MIN_WINDOW_SIZE.1..=4096).contains(&height))
        .then_some((width, height))
    }
    pub fn player_name(&self) -> Option<&str> {
        let name = self.username.trim();
        (!name.is_empty()
            && name.len() <= 16
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'))
        .then_some(name)
    }

    pub fn arguments(&self) -> Result<Vec<String>, String> {
        let name = self
            .player_name()
            .ok_or("Username must be 1–16 letters, numbers, or underscores.")?;
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
        let settings: Settings =
            serde_json::from_str(r#"{"username":"Steve","animation":"Hydra"}"#).unwrap();
        assert!(
            !settings
                .arguments()
                .unwrap()
                .iter()
                .any(|arg| arg == "--animation-preset")
        );
        assert!(settings.dark_mode);
        assert_eq!(settings.username, "Steve");
    }
    #[test]
    fn default_game_size_matches_java_and_saved_sizes_are_preserved() {
        let defaults: Settings = serde_json::from_str(r#"{"username":"Steve"}"#).unwrap();
        let args = defaults.arguments().unwrap();
        assert!(args.windows(2).any(|p| p == ["--width", "854"]));
        assert!(args.windows(2).any(|p| p == ["--height", "480"]));
        assert_eq!(defaults.launcher_size(), Some((900, 550)));
        let saved: Settings = serde_json::from_str(
            r#"{"width":"1280","height":"720","launcher_width":"1024","launcher_height":"768"}"#,
        )
        .unwrap();
        assert_eq!(
            (saved.width.as_str(), saved.height.as_str()),
            ("1280", "720")
        );
        assert_eq!(saved.launcher_size(), Some((1024, 768)));
        let loaded: Settings =
            serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        assert_eq!(loaded.launcher_size(), Some((1024, 768)));
    }
    #[test]
    fn launcher_size_rejects_incomplete_or_out_of_bounds_edits() {
        let mut settings = Settings::default();
        for (width, height) in [
            ("", "550"),
            ("abc", "550"),
            ("779", "550"),
            ("900", "479"),
            ("4097", "550"),
            ("900", "4097"),
        ] {
            settings.launcher_width = width.into();
            settings.launcher_height = height.into();
            assert_eq!(settings.launcher_size(), None);
        }
    }
    #[test]
    fn arguments_preserve_paths_and_negative_validation() {
        let mut s = Settings {
            username: "Steve".into(),
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
