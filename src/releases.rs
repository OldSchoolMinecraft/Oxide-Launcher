//! GitHub release discovery and staged, versioned game installation.
use crate::settings::save_json;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::Duration,
};

const API: &str = "https://api.github.com/repos/LeowFlow/Oxide/releases/latest";
const MAX_DOWNLOAD: u64 = 512 * 1024 * 1024;
const MAX_UNPACKED: u64 = 2 * 1024 * 1024 * 1024;
#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub body: Option<String>,
    pub assets: Vec<Asset>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
    pub digest: Option<String>,
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
        if safe_relative(&value.executable) && data.join(&value.executable).is_file() {
            Some(value)
        } else {
            None
        }
    }
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("Oxide-Launcher/0.1.0")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())
}
pub async fn latest() -> Result<Release, String> {
    let response = client()?
        .get(API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Could not reach GitHub: {e}"))?;
    match response.status().as_u16() {
        404 => Err("No public release found. If you are offline, you can use a local executable in Profile Editor.".into()),
        403 | 429 => Err("GitHub's request limit was reached. Try again later; your installed game is still available.".into()),
        _ => response.error_for_status().map_err(|e| e.to_string())?.json().await.map_err(|e| format!("Invalid release response: {e}")),
    }
}
fn asset_matches(name: &str, os: &str, arch: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let os_match = match os {
        "windows" => n.contains("windows") || n.contains("pc-windows"),
        "macos" => n.contains("macos") || n.contains("darwin") || n.contains("osx"),
        "linux" => n.contains("linux"),
        _ => false,
    };
    let arch_match = match arch {
        "x86_64" => n.contains("x86_64") || n.contains("amd64") || n.contains("x64"),
        "aarch64" => n.contains("aarch64") || n.contains("arm64"),
        _ => false,
    };
    os_match
        && arch_match
        && !n.contains("sha256")
        && !n.contains("checksum")
        && !n.ends_with(".sig")
        && (n.ends_with(".zip")
            || n.ends_with(".tar.gz")
            || n.ends_with(".tgz")
            || (os == "windows" && n.ends_with(".exe"))
            || !n.contains('.'))
}
pub fn compatible_asset(release: &Release) -> Result<Asset, String> {
    let matches: Vec<_> = release
        .assets
        .iter()
        .filter(|a| asset_matches(&a.name, std::env::consts::OS, std::env::consts::ARCH))
        .collect();
    match matches.as_slice() {
        [asset] => Ok((*asset).clone()),
        [] => Err(format!("No compatible release download for {} / {}. Use a local executable in Profile Editor.", std::env::consts::OS, std::env::consts::ARCH)),
        _ => Err("Several downloads match this platform. Release assets need one unambiguous OS/architecture package.".into()),
    }
}
pub async fn install(release: Release, data: PathBuf) -> Result<Installed, String> {
    let asset = compatible_asset(&release)?;
    if asset.size > MAX_DOWNLOAD {
        return Err("Release download exceeds the 512 MiB limit.".into());
    }
    let url = reqwest::Url::parse(&asset.browser_download_url).map_err(|e| e.to_string())?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.path().starts_with("/LeowFlow/Oxide/releases/download/")
    {
        return Err("Unexpected release download URL.".into());
    }
    fs::create_dir_all(&data).map_err(|e| e.to_string())?;
    let stage = tempfile::tempdir_in(&data).map_err(|e| e.to_string())?;
    let download = stage.path().join("download");
    let response = client()?
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(&download)
        .await
        .map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    use tokio::io::AsyncWriteExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        total += chunk.len() as u64;
        if total > MAX_DOWNLOAD {
            return Err("Release download is too large.".into());
        }
        hash.update(&chunk);
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
    }
    file.sync_all().await.map_err(|e| e.to_string())?;
    drop(file);
    if total != asset.size {
        return Err("Incomplete release download. Please retry.".into());
    }
    if let Some(digest) = &asset.digest
        && digest != &format!("sha256:{:x}", hash.finalize())
    {
        return Err("Release checksum verification failed.".into());
    }
    tokio::task::spawn_blocking(move || {
        let root = stage.path().join("game");
        fs::create_dir(&root).map_err(|e| e.to_string())?;
        unpack(&download, &asset.name, &root)?;
        let executable = find_executable(&root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
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
            .map_err(|e| e.to_string())?
            .keep();
        for item in fs::read_dir(&root).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            fs::rename(item.path(), destination.join(item.file_name()))
                .map_err(|e| e.to_string())?;
        }
        let installed = Installed {
            tag: release.tag_name,
            executable: destination
                .join(relative)
                .strip_prefix(&data)
                .map_err(|e| e.to_string())?
                .to_owned(),
        };
        save_json(&data.join("installed.json"), &installed)?;
        Ok(installed)
    })
    .await
    .map_err(|e| e.to_string())?
}
fn safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}
fn copy_entry(mut reader: impl Read, target: &Path, budget: &mut u64) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut output = fs::File::create(target).map_err(|e| e.to_string())?;
    let copied = std::io::copy(&mut reader.by_ref().take(*budget + 1), &mut output)
        .map_err(|e| e.to_string())?;
    if copied > *budget {
        return Err("Unpacked release exceeds the 2 GiB limit.".into());
    }
    *budget -= copied;
    output.flush().map_err(|e| e.to_string())
}
fn unpack(download: &Path, name: &str, root: &Path) -> Result<(), String> {
    let mut budget = MAX_UNPACKED;
    if name.ends_with(".zip") {
        let mut archive =
            zip::ZipArchive::new(fs::File::open(download).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if archive.len() > 20000 {
            return Err("Too many archive entries.".into());
        }
        for i in 0..archive.len() {
            let entry = archive.by_index(i).map_err(|e| e.to_string())?;
            let path = entry.enclosed_name().ok_or("Unsafe archive path")?;
            if !safe_relative(&path) || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
            {
                return Err("Unsafe archive entry.".into());
            }
            if entry.is_dir() {
                fs::create_dir_all(root.join(path)).map_err(|e| e.to_string())?;
            } else {
                copy_entry(entry, &root.join(path), &mut budget)?;
            }
        }
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(
            fs::File::open(download).map_err(|e| e.to_string())?,
        ));
        for (index, entry) in archive.entries().map_err(|e| e.to_string())?.enumerate() {
            if index >= 20000 {
                return Err("Too many archive entries.".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path().map_err(|e| e.to_string())?.into_owned();
            if !safe_relative(&path) {
                return Err("Unsafe archive path.".into());
            }
            if entry.header().entry_type().is_dir() {
                fs::create_dir_all(root.join(path)).map_err(|e| e.to_string())?;
            } else if entry.header().entry_type().is_file() {
                copy_entry(entry, &root.join(path), &mut budget)?;
            } else {
                return Err("Unsupported archive link or special file.".into());
            }
        }
    } else {
        fs::copy(
            download,
            root.join(if cfg!(windows) {
                "rust173.exe"
            } else {
                "rust173"
            }),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
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
    #[test]
    #[ignore = "Requires OXIDE_RELEASE_ARCHIVE pointing to a native client release package"]
    fn prepared_client_archive_matches_launcher_contract() {
        let archive = PathBuf::from(
            std::env::var_os("OXIDE_RELEASE_ARCHIVE").expect("OXIDE_RELEASE_ARCHIVE"),
        );
        let name = archive.file_name().unwrap().to_string_lossy().to_string();
        assert!(asset_matches(
            &name,
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
        let root = tempfile::tempdir().unwrap();
        let install = root.path().join("install");
        fs::create_dir(&install).unwrap();
        unpack(&archive, &name, &install).unwrap();
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
    use super::*;
    #[test]
    fn platform_selection() {
        assert!(asset_matches(
            "rust173-x86_64-unknown-linux-gnu.tar.gz",
            "linux",
            "x86_64"
        ));
        assert!(!asset_matches(
            "rust173-aarch64-apple-darwin.zip",
            "macos",
            "x86_64"
        ));
        assert!(asset_matches(
            "rust173-aarch64-apple-darwin.zip",
            "macos",
            "aarch64"
        ));
        assert!(!asset_matches(
            "rust173-linux-x64.sha256",
            "linux",
            "x86_64"
        ));
    }
    #[test]
    fn rejects_traversal() {
        assert!(!safe_relative(Path::new("../escape")));
        assert!(!safe_relative(Path::new("/absolute")));
        assert!(safe_relative(Path::new("game/rust173")));
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
    #[test]
    fn zip_traversal_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("test.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        zip.start_file("../escape", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"bad").unwrap();
        zip.finish().unwrap();
        let root = dir.path().join("out");
        fs::create_dir(&root).unwrap();
        assert!(unpack(&archive, "test.zip", &root).is_err());
        assert!(!dir.path().join("escape").exists());
    }
    #[test]
    fn tar_extracts_resources_and_rejects_links() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("game.tar.gz");
        let file = fs::File::create(&archive).unwrap();
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
            file,
            flate2::Compression::default(),
        ));
        let mut header = tar::Header::new_gnu();
        header.set_size(4);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append_data(&mut header, "bin/rust173", &b"game"[..])
            .unwrap();
        tar.into_inner().unwrap().finish().unwrap();
        let root = dir.path().join("out");
        fs::create_dir(&root).unwrap();
        unpack(&archive, "game.tar.gz", &root).unwrap();
        assert_eq!(fs::read(find_executable(&root).unwrap()).unwrap(), b"game");
        let links = dir.path().join("links.tar.gz");
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
            fs::File::create(&links).unwrap(),
            flate2::Compression::default(),
        ));
        let mut header = tar::Header::new_gnu();
        header.set_size(0);
        header.set_mode(0o777);
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_link_name("../../escape").unwrap();
        header.set_cksum();
        tar.append_data(&mut header, "link", std::io::empty())
            .unwrap();
        tar.into_inner().unwrap().finish().unwrap();
        assert!(unpack(&links, "links.tar.gz", &root).is_err());
    }
}
