use std::path::PathBuf;

pub const APP_NAME: &str = "Rusticean";
/// The data folder's name before the rename to Rusticean.
const LEGACY_APP_NAME: &str = "robloxbootstrapper";

/// Every directory and file the bootstrapper owns, rooted at one base folder.
#[derive(Debug, Clone)]
pub struct Paths {
    pub base: PathBuf,
    pub downloads: PathBuf,
    pub versions: PathBuf,
    pub logs: PathBuf,
    /// Files in here are copied over the Roblox install on every launch.
    pub modifications: PathBuf,
    pub state_file: PathBuf,
    pub settings_file: PathBuf,
}

impl Paths {
    pub fn new(base: impl Into<PathBuf>) -> Self {
        let base = base.into();
        Paths {
            downloads: base.join("Downloads"),
            versions: base.join("Versions"),
            logs: base.join("Logs"),
            modifications: base.join("Modifications"),
            state_file: base.join("State.json"),
            settings_file: base.join("Settings.json"),
            base,
        }
    }

    /// `%LOCALAPPDATA%\Rusticean` on Windows.
    pub fn default_base() -> Option<PathBuf> {
        dirs::data_local_dir().map(|d| d.join(APP_NAME))
    }

    /// Move data from the pre-rename `%LOCALAPPDATA%\robloxbootstrapper` folder into `base`,
    /// once, so settings and installed versions carry over.
    pub fn migrate_legacy(&self) {
        let Some(legacy) = self.base.parent().map(|p| p.join(LEGACY_APP_NAME)) else {
            return;
        };
        if self.base.exists() || !legacy.is_dir() {
            return;
        }
        match std::fs::rename(&legacy, &self.base) {
            Ok(()) => tracing::info!(from = %legacy.display(), "moved data to the new folder"),
            Err(e) => tracing::warn!(error = %e, "could not move the old data folder"),
        }
    }

    pub fn version_dir(&self, version_guid: &str) -> PathBuf {
        self.versions.join(version_guid)
    }

    /// The stock launcher's package cache, which we can copy from instead of downloading.
    pub fn stock_downloads() -> Option<PathBuf> {
        dirs::data_local_dir().map(|d| d.join("Roblox").join("Downloads"))
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        for dir in [
            &self.base,
            &self.downloads,
            &self.versions,
            &self.logs,
            &self.modifications,
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}
