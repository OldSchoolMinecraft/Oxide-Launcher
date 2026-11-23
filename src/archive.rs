use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path},
};

const MAX_UNPACKED: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Zip,
    TarGz,
    Executable,
}

impl Kind {
    pub fn of(name: &str) -> Self {
        let name = name.to_ascii_lowercase();
        if name.ends_with(".zip") {
            Self::Zip
        } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            Self::TarGz
        } else {
            Self::Executable
        }
    }
}

#[cfg(unix)]
pub(crate) fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}

pub(crate) fn safe_relative(path: &Path) -> bool {
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
pub(crate) fn extract(download: &Path, kind: Kind, root: &Path) -> Result<(), String> {
    let mut budget = MAX_UNPACKED;
    match kind {
        Kind::Zip => {
            let mut archive =
                zip::ZipArchive::new(fs::File::open(download).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            if archive.len() > MAX_ENTRIES {
                return Err("Too many archive entries.".into());
            }
            for i in 0..archive.len() {
                let entry = archive.by_index(i).map_err(|e| e.to_string())?;
                let path = entry.enclosed_name().ok_or("Unsafe archive path")?;
                if !safe_relative(&path)
                    || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
                {
                    return Err("Unsafe archive entry.".into());
                }
                if entry.is_dir() {
                    fs::create_dir_all(root.join(path)).map_err(|e| e.to_string())?;
                } else {
                    copy_entry(entry, &root.join(path), &mut budget)?;
                }
            }
        }
        Kind::TarGz => {
            let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(
                fs::File::open(download).map_err(|e| e.to_string())?,
            ));
            for (index, entry) in archive.entries().map_err(|e| e.to_string())?.enumerate() {
                if index >= MAX_ENTRIES {
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
        }
        Kind::Executable => {
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
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal() {
        assert!(!safe_relative(Path::new("../escape")));
        assert!(!safe_relative(Path::new("/absolute")));
        assert!(safe_relative(Path::new("game/rust173")));
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
        assert!(extract(&archive, Kind::Zip, &root).is_err());
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
        extract(&archive, Kind::TarGz, &root).unwrap();
        assert_eq!(fs::read(root.join("bin/rust173")).unwrap(), b"game");
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
        assert!(extract(&links, Kind::TarGz, &root).is_err());
    }
}
