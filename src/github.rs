use crate::archive;
use futures_util::StreamExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};
use tokio::io::AsyncWriteExt;

const MAX_DOWNLOAD: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub body: Option<String>,
    pub published_at: Option<String>,
    pub assets: Vec<Asset>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
    pub digest: Option<String>,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("Oxide-Launcher/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())
}

pub(crate) async fn get(url: &str) -> Result<reqwest::Response, String> {
    client()?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| error.to_string())
}

pub(crate) fn asset_matches(name: &str, os: &str, arch: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let os_match = match os {
        "windows" => name.contains("windows"),
        "macos" => name.contains("macos") || name.contains("darwin") || name.contains("osx"),
        "linux" => name.contains("linux"),
        _ => false,
    };
    let arch_match = match arch {
        "x86_64" => name.contains("x86_64") || name.contains("amd64") || name.contains("x64"),
        "aarch64" => name.contains("aarch64") || name.contains("arm64"),
        _ => false,
    };
    os_match
        && arch_match
        && !name.contains("sha256")
        && !name.contains("checksum")
        && !name.ends_with(".sig")
        && (archive::Kind::of(&name) != archive::Kind::Executable
            || (os == "windows" && name.ends_with(".exe"))
            || !name.contains('.'))
}
pub(crate) async fn download_asset(
    asset: &Asset,
    repository: &str,
    destination: &Path,
) -> Result<(), String> {
    if asset.size > MAX_DOWNLOAD {
        return Err("Release download exceeds the 512 MiB limit.".into());
    }
    let url = reqwest::Url::parse(&asset.browser_download_url).map_err(|e| e.to_string())?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url
            .path()
            .starts_with(&format!("/{repository}/releases/download/"))
    {
        return Err("Unexpected release download URL.".into());
    }
    let response = client()?
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(destination)
        .await
        .map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
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
    if total != asset.size {
        return Err("Incomplete release download. Please retry.".into());
    }
    if let Some(digest) = &asset.digest
        && digest != &format!("sha256:{:x}", hash.finalize())
    {
        return Err("Release checksum verification failed.".into());
    }
    Ok(())
}
