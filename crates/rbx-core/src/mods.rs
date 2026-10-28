//! The Modifications folder, like Bloxstrap's mods: every file in it is copied over the
//! Roblox install on each launch, so `content/sounds/ouch.ogg` there replaces the death
//! sound. Files a mod overwrote are kept aside and put back once the mod is removed.
//!
//! Also Voidstrap's fullbright, which hides the lighting lookup texture.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

/// Where overwritten originals are kept, inside the version folder.
const BACKUP_DIR: &str = ".rusticean-originals";
/// The list of files we put in the version folder last time.
const APPLIED_LIST: &str = ".rusticean-mods.json";

/// Copy every file in `sources` into `version_dir` (a later folder wins when two have
/// the same file) and undo mods that were removed. Returns how many files are applied.
pub fn apply(sources: &[&Path], version_dir: &Path) -> io::Result<usize> {
    let backup_dir = version_dir.join(BACKUP_DIR);
    let list_file = version_dir.join(APPLIED_LIST);

    let mut origin = BTreeMap::new();
    for dir in sources {
        let mut files = BTreeSet::new();
        collect_files(dir, dir, &mut files)?;
        for rel in files {
            origin.insert(rel, *dir);
        }
    }
    let current: BTreeSet<String> = origin.keys().cloned().collect();
    let previous: BTreeSet<String> = std::fs::read_to_string(&list_file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();

    for rel in previous.difference(&current) {
        let target = version_dir.join(rel);
        let original = backup_dir.join(rel);
        if original.exists() {
            std::fs::rename(&original, &target)?;
        } else {
            // the mod added a file Roblox didn't have
            remove_if_exists(&target)?;
        }
    }

    for rel in &current {
        let target = version_dir.join(rel);
        if !previous.contains(rel) && target.exists() {
            let original = backup_dir.join(rel);
            if let Some(parent) = original.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(&target, &original)?;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(origin[rel].join(rel), &target)?;
    }

    let list = serde_json::to_string(&current).map_err(io::Error::other)?;
    std::fs::write(&list_file, list)?;
    Ok(current.len())
}

/// Paths of every file under `dir`, relative to `root`, with `/` separators.
fn collect_files(root: &Path, dir: &Path, out: &mut BTreeSet<String>) -> io::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(root, &path, out)?;
        } else if let Ok(rel) = path.strip_prefix(root) {
            let parts: Vec<_> = rel.iter().map(|p| p.to_string_lossy()).collect();
            out.insert(parts.join("/"));
        }
    }
    Ok(())
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

const DISABLED: &str = ".disabled";

/// Turn fullbright on by renaming `brdfLUT.*` in the textures folder out of the way,
/// or off by renaming it back.
pub fn set_fullbright(version_dir: &Path, on: bool) -> io::Result<()> {
    let textures = version_dir.join("PlatformContent/pc/textures");
    let mut files = Vec::new();
    find_brdf(&textures, &mut files)?;
    for path in files {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let disabled = name.ends_with(DISABLED);
        if on && !disabled {
            let mut to = path.clone().into_os_string();
            to.push(DISABLED);
            std::fs::rename(&path, PathBuf::from(to))?;
        } else if !on && disabled {
            let restored = name.trim_end_matches(DISABLED).to_owned();
            std::fs::rename(&path, path.with_file_name(restored))?;
        }
    }
    Ok(())
}

fn find_brdf(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            find_brdf(&path, out)?;
        } else if path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("brdfLUT"))
        {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn applies_and_restores_mods() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        let version = dir.path().join("version");
        let sound = version.join("content/sounds/ouch.ogg");
        write(&sound, "stock");
        write(&mods.join("content/sounds/ouch.ogg"), "oof");
        write(&mods.join("content/extra.txt"), "new");

        assert_eq!(apply(&[&mods], &version).unwrap(), 2);
        assert_eq!(read(&sound), "oof");
        assert_eq!(read(&version.join("content/extra.txt")), "new");

        // applying again keeps the stock backup intact
        apply(&[&mods], &version).unwrap();
        assert_eq!(
            read(&version.join(BACKUP_DIR).join("content/sounds/ouch.ogg")),
            "stock"
        );

        std::fs::remove_dir_all(&mods).unwrap();
        assert_eq!(apply(&[&mods], &version).unwrap(), 0);
        assert_eq!(read(&sound), "stock");
        assert!(!version.join("content/extra.txt").exists());
    }

    #[test]
    fn fullbright_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let lut = dir.path().join("PlatformContent/pc/textures/brdfLUT.dds");
        write(&lut, "lut");

        set_fullbright(dir.path(), true).unwrap();
        assert!(!lut.exists());
        set_fullbright(dir.path(), true).unwrap();
        set_fullbright(dir.path(), false).unwrap();
        assert_eq!(read(&lut), "lut");
    }
}
