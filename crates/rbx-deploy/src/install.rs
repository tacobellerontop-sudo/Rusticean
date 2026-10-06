//! Downloading, verifying and extracting a Roblox version's packages into a version folder.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::StreamExt;
use md5::{Digest, Md5};
use tokio::io::AsyncWriteExt;

use crate::cdn::Mirror;
use crate::{BinaryType, Error, Package, Result, hash};

const MAX_DOWNLOAD_TRIES: u32 = 5;

/// Written next to the executable so the client finds its content folder.
pub const APP_SETTINGS_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Settings>\r\n\
\t<ContentFolder>content</ContentFolder>\r\n\
\t<BaseUrl>http://www.roblox.com</BaseUrl>\r\n\
</Settings>\r\n";

/// Only extracted on demand, when the WebView2 runtime needs installing.
const WEBVIEW2_INSTALLER: &str = "WebView2RuntimeInstaller.zip";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    Downloading { package: String },
    Extracting,
    Done,
}

#[derive(Debug, Clone)]
pub struct Progress {
    pub stage: Stage,
    /// Bytes of packed (zip) data downloaded or found in cache so far.
    pub downloaded: u64,
    /// Total packed size of all packages.
    pub total: u64,
}

pub type ProgressFn = Arc<dyn Fn(Progress) + Send + Sync>;

/// Everything needed to install one version.
pub struct InstallJob {
    pub client: reqwest::Client,
    pub mirror: Mirror,
    pub binary: BinaryType,
    pub version_guid: String,
    pub packages: Vec<Package>,
    /// Our package cache, files named by their MD5.
    pub downloads_dir: PathBuf,
    /// Where the version gets extracted; wiped first for a clean install.
    pub version_dir: PathBuf,
    /// The stock Roblox launcher's cache (`%LOCALAPPDATA%\Roblox\Downloads`), reused if present.
    pub stock_downloads_dir: Option<PathBuf>,
    pub on_progress: ProgressFn,
    pub cancel: Arc<AtomicBool>,
}

struct Tracker {
    total: u64,
    downloaded: u64,
    on_progress: ProgressFn,
}

impl Tracker {
    fn report(&self, stage: Stage) {
        (self.on_progress)(Progress {
            stage,
            downloaded: self.downloaded,
            total: self.total,
        });
    }
}

impl InstallJob {
    fn check_cancel(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }

    pub async fn run(self) -> Result<()> {
        // start from a clean folder, in case a previous attempt left half a version behind
        if tokio::fs::try_exists(&self.version_dir)
            .await
            .unwrap_or(false)
        {
            tokio::fs::remove_dir_all(&self.version_dir)
                .await
                .map_err(|e| Error::io(&self.version_dir, e))?;
        }

        for dir in [&self.downloads_dir, &self.version_dir] {
            tokio::fs::create_dir_all(dir)
                .await
                .map_err(|e| Error::io(dir, e))?;
        }

        let mut tracker = Tracker {
            total: self.packages.iter().map(|p| p.packed_size).sum(),
            downloaded: 0,
            on_progress: self.on_progress.clone(),
        };

        let mut extractions = Vec::new();

        // download one at a time, extract in the background as soon as each one lands
        for package in &self.packages {
            self.check_cancel()?;

            tracker.report(Stage::Downloading {
                package: package.name.clone(),
            });
            let zip_path = self.ensure_downloaded(package, &mut tracker).await?;

            if package.name == WEBVIEW2_INSTALLER {
                continue;
            }

            let Some(subdir) = self.binary.package_dir(&package.name) else {
                tracing::warn!(package = %package.name, "unknown package, not extracting");
                continue;
            };

            let dest = self.version_dir.join(subdir);
            let name = package.name.clone();
            extractions.push(tokio::task::spawn_blocking(move || {
                extract_zip(&zip_path, &dest).map_err(|message| Error::Extract {
                    package: name,
                    message,
                })
            }));
        }

        tracker.report(Stage::Extracting);
        for task in extractions {
            task.await.map_err(|e| Error::Extract {
                package: "?".into(),
                message: e.to_string(),
            })??;
        }
        self.check_cancel()?;

        let settings = self.version_dir.join("AppSettings.xml");
        tokio::fs::write(&settings, APP_SETTINGS_XML)
            .await
            .map_err(|e| Error::io(&settings, e))?;

        tracker.report(Stage::Done);
        Ok(())
    }

    /// Return the path of a verified copy of `package` in our cache, downloading it if needed.
    async fn ensure_downloaded(&self, package: &Package, tracker: &mut Tracker) -> Result<PathBuf> {
        let cached = self.downloads_dir.join(&package.signature);

        if cached.exists() {
            if hash_matches(&cached, &package.signature).await {
                tracing::debug!(package = %package.name, "using cached package");
                tracker.downloaded += package.packed_size;
                return Ok(cached);
            }
            tracing::warn!(package = %package.name, "cached package is corrupt, re-downloading");
            let _ = tokio::fs::remove_file(&cached).await;
        }

        if let Some(stock) = &self.stock_downloads_dir {
            let stock_file = stock.join(&package.signature);
            if stock_file.exists() && hash_matches(&stock_file, &package.signature).await {
                tracing::debug!(package = %package.name, "copying package from stock launcher cache");
                tokio::fs::copy(&stock_file, &cached)
                    .await
                    .map_err(|e| Error::io(&cached, e))?;
                tracker.downloaded += package.packed_size;
                return Ok(cached);
            }
        }

        let url = self.mirror.package_url(&self.version_guid, &package.name);
        let mut attempt = 1;
        loop {
            self.check_cancel()?;
            let before = tracker.downloaded;

            match self.download(&url, package, &cached, tracker).await {
                Ok(()) => return Ok(cached),
                Err(e @ (Error::Cancelled | Error::Checksum { .. })) => return Err(e),
                Err(e) if attempt >= MAX_DOWNLOAD_TRIES => return Err(e),
                Err(e) => {
                    tracing::warn!(package = %package.name, attempt, error = %e, "download failed, retrying");
                    tracker.downloaded = before;
                    attempt += 1;
                }
            }
        }
    }

    async fn download(
        &self,
        url: &str,
        package: &Package,
        dest: &Path,
        tracker: &mut Tracker,
    ) -> Result<()> {
        tracing::info!(package = %package.name, "downloading");

        let part = dest.with_extension("part");
        let response = self.client.get(url).send().await?.error_for_status()?;
        let mut stream = response.bytes_stream();
        let mut file = tokio::fs::File::create(&part)
            .await
            .map_err(|e| Error::io(&part, e))?;
        let mut hasher = Md5::new();

        let result: Result<()> = async {
            while let Some(chunk) = stream.next().await {
                self.check_cancel()?;
                let chunk = chunk?;
                hasher.update(&chunk);
                file.write_all(&chunk)
                    .await
                    .map_err(|e| Error::io(&part, e))?;
                tracker.downloaded += chunk.len() as u64;
                tracker.report(Stage::Downloading {
                    package: package.name.clone(),
                });
            }
            file.flush().await.map_err(|e| Error::io(&part, e))?;
            Ok(())
        }
        .await;
        drop(file);

        if let Err(e) = result {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(e);
        }

        let actual = hex::encode(hasher.finalize());
        if actual != package.signature {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(Error::Checksum {
                package: package.name.clone(),
                expected: package.signature.clone(),
                actual,
            });
        }

        tokio::fs::rename(&part, dest)
            .await
            .map_err(|e| Error::io(dest, e))
    }
}

async fn hash_matches(path: &Path, expected: &str) -> bool {
    let path = path.to_owned();
    match tokio::task::spawn_blocking(move || hash::md5_file(&path)).await {
        Ok(Ok(actual)) => actual == expected,
        _ => false,
    }
}

/// Turn a zip entry name (Roblox zips use `\` separators) into a safe relative path.
fn sanitize_entry(name: &str) -> Option<PathBuf> {
    let normalized = name.replace('\\', "/");
    let mut out = PathBuf::new();
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(part) => {
                if part.to_string_lossy().contains(':') {
                    return None;
                }
                out.push(part);
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(out)
}

/// Extract every entry of `zip_path` under `dest`.
pub fn extract_zip(zip_path: &Path, dest: &Path) -> std::result::Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive =
        zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let raw_name = entry.name().to_owned();

        let Some(rel) = sanitize_entry(&raw_name) else {
            return Err(format!("refusing unsafe entry {raw_name:?}"));
        };
        let out = dest.join(&rel);

        if raw_name.ends_with('/') || raw_name.ends_with('\\') {
            std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
            continue;
        }

        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let mut target =
            std::fs::File::create(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        std::io::copy(&mut entry, &mut target).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn sanitizes_entries() {
        assert_eq!(
            sanitize_entry("content\\fonts\\a.ttf"),
            Some(PathBuf::from("content/fonts/a.ttf"))
        );
        assert_eq!(sanitize_entry("./a.txt"), Some(PathBuf::from("a.txt")));
        assert_eq!(sanitize_entry("..\\evil.dll"), None);
        assert_eq!(sanitize_entry("/etc/passwd"), None);
        assert_eq!(sanitize_entry("C:\\Windows\\x"), None);
    }

    #[test]
    fn extracts_backslash_paths() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("p.zip");
        {
            let mut w = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            w.start_file("sub\\file.txt", opts).unwrap();
            w.write_all(b"hi").unwrap();
            w.start_file("top.txt", opts).unwrap();
            w.write_all(b"top").unwrap();
            w.finish().unwrap();
        }
        let dest = dir.path().join("out");
        extract_zip(&zip_path, &dest).unwrap();
        assert_eq!(std::fs::read(dest.join("sub/file.txt")).unwrap(), b"hi");
        assert_eq!(std::fs::read(dest.join("top.txt")).unwrap(), b"top");
    }
}
