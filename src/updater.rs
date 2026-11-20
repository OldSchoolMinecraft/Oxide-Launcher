//! Independent launcher updates. An empty GitHub repository is a normal state.
use crate::releases::{self, Asset, Release};
use semver::Version;
use std::{fs, path::Path};

const REPOSITORY: &str = "OldSchoolMinecraft/Oxide-Launcher";

pub async fn latest() -> Result<Option<Release>, String> {
    latest_from(&format!(
        "https://api.github.com/repos/{REPOSITORY}/releases/latest"
    ))
    .await
}

async fn latest_from(url: &str) -> Result<Option<Release>, String> {
    let response = releases::client()?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Launcher update check failed: {e}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let release: Release = response
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| format!("Invalid launcher release: {e}"))?;
    if newer(&release.tag_name, env!("CARGO_PKG_VERSION"))? {
        Ok(Some(release))
    } else {
        Ok(None)
    }
}

fn newer(tag: &str, current: &str) -> Result<bool, String> {
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag))
        .map_err(|e| format!("Invalid launcher version {tag}: {e}"))?;
    let current = Version::parse(current).map_err(|e| e.to_string())?;
    Ok(version.pre.is_empty() && version.cmp_precedence(&current).is_gt())
}

fn asset(release: &Release) -> Result<Asset, String> {
    let matches: Vec<_> = release
        .assets
        .iter()
        .filter(|a| {
            a.name.to_ascii_lowercase().starts_with("oxide-launcher-")
                && releases::asset_matches(&a.name, std::env::consts::OS, std::env::consts::ARCH)
        })
        .collect();
    match matches.as_slice() {
        [asset] => Ok((*asset).clone()),
        [] => Err("No launcher update package is published for this platform yet.".into()),
        _ => Err("Multiple launcher update packages match this platform.".into()),
    }
}

pub async fn install(release: Release) -> Result<String, String> {
    let asset = asset(&release)?;
    // Launcher packages are executable code: require GitHub's SHA-256 digest.
    if !asset
        .digest
        .as_deref()
        .is_some_and(|d| d.starts_with("sha256:") && d.len() == 71)
    {
        return Err("Launcher update has no SHA-256 digest; keeping the current launcher.".into());
    }
    let stage = tempfile::tempdir().map_err(|e| e.to_string())?;
    let download = stage.path().join("download");
    releases::download_asset(&asset, REPOSITORY, &download).await?;
    tokio::task::spawn_blocking(move || {
        let root = stage.path().join("package");
        fs::create_dir(&root).map_err(|e| e.to_string())?;
        // The release contract uses archives, so extraction preserves macOS bundle paths.
        if !(asset.name.ends_with(".zip")
            || asset.name.ends_with(".tar.gz")
            || asset.name.ends_with(".tgz"))
        {
            return Err("Launcher updates must be ZIP or tar.gz packages.".into());
        }
        releases::unpack(&download, &asset.name, &root)?;
        let executable = executable(&root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        replace(&root, &executable)?;
        Ok(release.tag_name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(not(target_os = "macos"))]
fn replace(_root: &Path, executable: &Path) -> Result<(), String> {
    self_replace::self_replace(executable)
        .map_err(|e| format!("Could not replace launcher (check installation permissions): {e}"))
}

#[cfg(target_os = "macos")]
fn replace(root: &Path, _executable: &Path) -> Result<(), String> {
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let bundle = current
        .ancestors()
        .nth(3)
        .filter(|p| p.extension().is_some_and(|e| e == "app"))
        .ok_or("Run the packaged .app to update the launcher on macOS.")?;
    let parent = bundle.parent().ok_or("The app has no parent directory.")?;
    // Stage on the destination volume. Replace the entire signed bundle, including
    // Info.plist and _CodeSignature, rather than invalidating its resource seal.
    let stage = tempfile::tempdir_in(parent).map_err(|e| e.to_string())?;
    fn copy_tree(source: &Path, target: &Path) -> std::io::Result<()> {
        fs::create_dir(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let destination = target.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_tree(&entry.path(), &destination)?;
            } else {
                fs::copy(entry.path(), destination)?;
            }
        }
        Ok(())
    }
    let replacement = stage.path().join("new.app");
    copy_tree(&root.join("Oxide Launcher.app"), &replacement).map_err(|e| e.to_string())?;
    let backup = stage.path().join("previous.app");
    fs::rename(bundle, &backup).map_err(|e| format!("Could not move the current app: {e}"))?;
    if let Err(error) = fs::rename(&replacement, bundle) {
        if let Err(rollback) = fs::rename(&backup, bundle) {
            let recovery = stage.keep();
            return Err(format!(
                "Update failed: {error}; restore failed: {rollback}. Previous app is in {}",
                recovery.display()
            ));
        }
        return Err(format!("Update failed; previous app restored: {error}"));
    }
    Ok(())
}

fn executable(root: &Path) -> Result<std::path::PathBuf, String> {
    let relative = if cfg!(target_os = "macos") {
        "Oxide Launcher.app/Contents/MacOS/oxide-launcher"
    } else if cfg!(windows) {
        "oxide-launcher.exe"
    } else {
        "oxide-launcher"
    };
    let path = root.join(relative);
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("Launcher package is missing {relative}."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mock_response(status: &str, body: &str) -> Result<Option<Release>, String> {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/latest", listener.local_addr().unwrap());
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0; 4096];
            assert!(socket.read(&mut request).unwrap() > 0);
            socket.write_all(response.as_bytes()).unwrap();
        });
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(latest_from(&url));
        server.join().unwrap();
        result
    }
    #[test]
    fn empty_feed_is_normal_but_http_and_json_errors_are_reported() {
        assert!(mock_response("404 Not Found", "{}").unwrap().is_none());
        assert!(mock_response("403 Forbidden", "{}").is_err());
        assert!(mock_response("200 OK", "bad json").is_err());
        assert!(
            mock_response("200 OK", r#"{"tag_name":"v0.1.0","assets":[]}"#)
                .unwrap()
                .is_none()
        );
        assert!(
            mock_response("200 OK", r#"{"tag_name":"v99.0.0","assets":[]}"#)
                .unwrap()
                .is_some()
        );
    }
    #[test]
    fn platform_package_selection_rejects_game_assets_and_ambiguity() {
        let mut release = Release {
            tag_name: "v0.2.0".into(),
            body: None,
            assets: vec![],
        };
        let name = format!(
            "Oxide-Launcher-v0.2.0-{}-{}.zip",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        let package = Asset {
            name,
            browser_download_url: String::new(),
            size: 0,
            digest: None,
        };
        release.assets.push(Asset {
            name: "rust173-linux-x86_64.zip".into(),
            ..package.clone()
        });
        assert!(asset(&release).is_err());
        release.assets.push(package.clone());
        assert_eq!(asset(&release).unwrap().name, package.name);
        release.assets.push(package);
        assert!(asset(&release).is_err());
    }
    #[test]
    fn versions_do_not_downgrade_or_install_prereleases() {
        assert!(newer("v0.2.0", "0.1.0").unwrap());
        for tag in ["v0.1.0", "0.0.9", "v0.2.0-beta.1", "0.1.0+build"] {
            assert!(!newer(tag, "0.1.0").unwrap());
        }
        assert!(newer("latest", "0.1.0").is_err());
    }
    #[test]
    fn finds_only_expected_launcher_path() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("hydra-client"), "game").unwrap();
        assert!(executable(root.path()).is_err());
        let path = root.path().join(if cfg!(target_os = "macos") {
            "Oxide Launcher.app/Contents/MacOS/oxide-launcher"
        } else if cfg!(windows) {
            "oxide-launcher.exe"
        } else {
            "oxide-launcher"
        });
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "launcher").unwrap();
        assert_eq!(executable(root.path()).unwrap(), path);
    }
}
