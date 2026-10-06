//! Deleting old cached packages and logs, like Fishstrap's cache cleaner.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::paths::Paths;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Report {
    pub files: u64,
    pub bytes: u64,
}

/// Folders whose files are safe to delete once they're old: our package cache, our logs
/// and Roblox's own logs.
pub fn folders(paths: &Paths) -> Vec<PathBuf> {
    let mut dirs = vec![paths.downloads.clone(), paths.logs.clone()];
    if let Some(local) = dirs::data_local_dir() {
        dirs.push(local.join("Roblox").join("logs"));
    }
    dirs
}

/// Delete files older than `max_age` from [`folders`]. Files in use are skipped.
pub fn run(paths: &Paths, max_age: Duration) -> Report {
    let cutoff = SystemTime::now()
        .checked_sub(max_age)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut report = Report::default();
    for dir in folders(paths) {
        clean_dir(&dir, cutoff, &mut report);
    }
    report
}

fn clean_dir(dir: &Path, cutoff: SystemTime, report: &mut Report) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let modified = meta.modified().unwrap_or(SystemTime::now());
        if modified < cutoff && std::fs::remove_file(entry.path()).is_ok() {
            report.files += 1;
            report.bytes += meta.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_only_old_files() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.log");
        let new = dir.path().join("new.log");
        std::fs::write(&old, b"12345").unwrap();
        std::fs::write(&new, b"1").unwrap();
        let week_ago = SystemTime::now() - Duration::from_secs(8 * 24 * 3600);
        std::fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(week_ago)
            .unwrap();

        let mut report = Report::default();
        let cutoff = SystemTime::now() - Duration::from_secs(7 * 24 * 3600);
        clean_dir(dir.path(), cutoff, &mut report);

        assert_eq!(report, Report { files: 1, bytes: 5 });
        assert!(!old.exists());
        assert!(new.exists());
    }
}
