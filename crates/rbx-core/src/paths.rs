use std::path::PathBuf;

pub const APP_NAME: &str = "robloxbootstrapper";

/// Every directory and file the bootstrapper owns, rooted at one base folder.
#[derive(Debug, Clone)]
pub struct Paths {
    pub base: PathBuf,
    pub downloads: PathBuf,
    pub versions: PathBuf,
    pub logs: PathBuf,
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
            state_file: base.join("State.json"),
            settings_file: base.join("Settings.json"),
            base,
        }
    }

    /// `%LOCALAPPDATA%\robloxbootstrapper` on Windows.
    pub fn default_base() -> Option<PathBuf> {
        dirs::data_local_dir().map(|d| d.join(APP_NAME))
    }

    pub fn version_dir(&self, version_guid: &str) -> PathBuf {
        self.versions.join(version_guid)
    }

    /// The stock launcher's package cache, which we can copy from instead of downloading.
    pub fn stock_downloads() -> Option<PathBuf> {
        dirs::data_local_dir().map(|d| d.join("Roblox").join("Downloads"))
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        for dir in [&self.base, &self.downloads, &self.versions, &self.logs] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}
