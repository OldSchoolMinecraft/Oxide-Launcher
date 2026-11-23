//! Discovery and versioned installation of the Oxide game.
use crate::{
    archive,
    github::{self, Asset, Release},
    settings::save_json,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

const API: &str = "https://api.github.com/repos/LeowFlow/Oxide/releases";

#[derive(Debug, Clone, Deserialize)]
pub struct Note {
    pub tag_name: String,
    pub published_at: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installed {
    pub tag: String,
    pub executable: PathBuf,
}
impl Installed {
    pub fn load(data: &Path) -> Option<Self> {
        let value: Self =
            serde_json::from_slice(&fs::read(data.join("installed.json")).ok()?).ok()?;
        (archive::safe_relative(&value.executable) && data.join(&value.executable).is_file())
            .then_some(value)
    }
}

pub async fn latest() -> Result<Release, String> {
    let response = github::get(&format!("{API}/latest"))
        .await
        .map_err(|e| format!("Could not reach GitHub: {e}"))?;
    match response.status().as_u16() {
        404 => Err("No public release found. If you are offline, you can use a local executable in Profile Editor.".into()),
        403 | 429 => Err("GitHub's request limit was reached. Try again later; your installed game is still available.".into()),
        _ => response.error_for_status().map_err(|e| e.to_string())?.json().await.map_err(|e| format!("Invalid release response: {e}")),
    }
}

pub async fn history() -> Result<Vec<Note>, String> {
    const PAGE_SIZE: usize = 100;
    let mut notes = Vec::new();
    for page in 1.. {
        let response = github::get(&format!("{API}?per_page={PAGE_SIZE}&page={page}"))
            .await?
            .error_for_status()
            .map_err(|error| error.to_string())?;
        let batch: Vec<Note> = response.json().await.map_err(|error| error.to_string())?;
        let last_page = batch.len() < PAGE_SIZE;
        notes.extend(batch);
        if last_page {
            break;
        }
    }
    Ok(notes)
}

fn compatible_asset(release: &Release) -> Result<Asset, String> {
    let matches: Vec<_> = release
        .assets
        .iter()
        .filter(|a| github::asset_matches(&a.name, std::env::consts::OS, std::env::consts::ARCH))
        .collect();
    match matches.as_slice() {
        [asset] => Ok((*asset).clone()),
        [] => Err(format!("No compatible release download for {} / {}. Use a local executable in Profile Editor.", std::env::consts::OS, std::env::consts::ARCH)),
        _ => Err("Several downloads match this platform. Release assets need one unambiguous OS/architecture package.".into()),
    }
}
pub async fn install(release: Release, data: PathBuf) -> Result<Installed, String> {
    let asset = compatible_asset(&release)?;
    fs::create_dir_all(&data).map_err(|e| e.to_string())?;
    let stage = tempfile::tempdir_in(&data).map_err(|e| e.to_string())?;
    let download = stage.path().join("download");
    github::download_asset(&asset, "LeowFlow/Oxide", &download).await?;
    tokio::task::spawn_blocking(move || {
        let root = stage.path().join("game");
        fs::create_dir(&root).map_err(|e| e.to_string())?;
        archive::extract(&download, archive::Kind::of(&asset.name), &root)?;
        let executable = find_executable(&root)?;
        #[cfg(unix)]
        archive::make_executable(&executable).map_err(|error| error.to_string())?;
        let relative = executable
            .strip_prefix(&root)
            .map_err(|e| e.to_string())?
            .to_owned();
        // Unique directories preserve the previous working installation if any step fails.
        let key = format!("{:x}", Sha256::digest(release.tag_name.as_bytes()));
        let versions = data.join("versions");
        fs::create_dir_all(&versions).map_err(|e| e.to_string())?;
        let destination = tempfile::Builder::new()
            .prefix(&key[..12])
            .tempdir_in(&versions)
            .map_err(|error| error.to_string())?;
        for item in fs::read_dir(&root).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            fs::rename(item.path(), destination.path().join(item.file_name()))
                .map_err(|e| e.to_string())?;
        }
        let installed = Installed {
            tag: release.tag_name,
            executable: destination
                .path()
                .join(relative)
                .strip_prefix(&data)
                .map_err(|e| e.to_string())?
                .to_owned(),
        };
        save_json(&data.join("installed.json"), &installed)?;
        let _ = destination.keep();
        Ok(installed)
    })
    .await
    .map_err(|e| e.to_string())?
}
fn find_executable(root: &Path) -> Result<PathBuf, String> {
    fn walk(path: &Path, found: &mut Vec<PathBuf>) -> Result<(), String> {
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                walk(&entry.path(), found)?;
            } else {
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if [
                    "rust173",
                    "rust173.exe",
                    "rust173-client",
                    "rust173-client.exe",
                    "hydra-client",
                    "hydra-client.exe",
                ]
                .contains(&name.as_str())
                {
                    found.push(entry.path());
                }
            }
        }
        Ok(())
    }
    let mut found = Vec::new();
    walk(root, &mut found)?;
    if found.len() == 1 {
        Ok(found.remove(0))
    } else {
        Err(
            "Package must contain exactly one rust173, rust173-client, or hydra-client executable."
                .into(),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Requires OXIDE_RELEASE_ARCHIVE pointing to a native client release package"]
    fn prepared_client_archive_matches_launcher_contract() {
        let archive = PathBuf::from(
            std::env::var_os("OXIDE_RELEASE_ARCHIVE").expect("OXIDE_RELEASE_ARCHIVE"),
        );
        let name = archive.file_name().unwrap().to_string_lossy().to_string();
        assert!(github::asset_matches(
            &name,
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
        let root = tempfile::tempdir().unwrap();
        let install = root.path().join("install");
        fs::create_dir(&install).unwrap();
        archive::extract(&archive, archive::Kind::of(&name), &install).unwrap();
        let executable = find_executable(&install).unwrap();
        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(install.join("RESOURCE_MANIFEST.json")).expect("bundled resource manifest"),
        )
        .unwrap();
        let audio = manifest["audioFiles"].as_object().expect("audioFiles");
        assert_eq!(audio.len(), 224);
        for (relative, digest) in audio {
            let bytes =
                fs::read(install.join(relative)).expect("launcher must preserve bundled audio");
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                digest.as_str().unwrap()
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let game = root.path().join("game");
        fs::create_dir(&game).unwrap();
        let version = std::process::Command::new(&executable)
            .arg("--version")
            .current_dir(&game)
            .output()
            .unwrap();
        assert!(version.status.success());
        assert!(String::from_utf8_lossy(&version.stdout).starts_with("hydra-client "));
        let settings = crate::settings::Settings {
            username: "Launcher_Test".into(),
            mute: true,
            assets: "/a path/game.zip".into(),
            sounds: "/a path/resources".into(),
            ..Default::default()
        };
        let output = std::process::Command::new(executable)
            .args(settings.arguments().unwrap())
            .arg("--help")
            .env("HYDRA_HOME", &game)
            .current_dir(&game)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[test]
    fn platform_selection() {
        assert!(github::asset_matches(
            "rust173-x86_64-unknown-linux-gnu.tar.gz",
            "linux",
            "x86_64"
        ));
        assert!(!github::asset_matches(
            "rust173-aarch64-apple-darwin.zip",
            "macos",
            "x86_64"
        ));
        assert!(github::asset_matches(
            "rust173-aarch64-apple-darwin.zip",
            "macos",
            "aarch64"
        ));
        assert!(!github::asset_matches(
            "rust173-linux-x64.sha256",
            "linux",
            "x86_64"
        ));
    }
    #[test]
    fn finds_nested_binary() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("bin")).unwrap();
        fs::write(dir.path().join("bin/hydra-client"), "test").unwrap();
        assert!(
            find_executable(dir.path())
                .unwrap()
                .ends_with("bin/hydra-client")
        );
    }
}
