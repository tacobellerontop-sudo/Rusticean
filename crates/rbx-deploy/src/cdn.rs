//! Picking a working setup mirror. Each mirror is probed by fetching `/versionStudio`,
//! whose body is a fixed, well-known hash. Lower-priority mirrors start their probe a
//! little later, so the primary CDN wins whenever it's healthy.

use std::time::Duration;

use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;

use crate::{Error, Result};

/// Mirrors and how many seconds to wait before probing each one (0 = highest priority).
pub const MIRRORS: &[(&str, u64)] = &[
    ("https://setup.rbxcdn.com", 0),
    ("https://setup-aws.rbxcdn.com", 2),
    ("https://setup-ak.rbxcdn.com", 2),
    ("https://roblox-setup.cachefly.net", 2),
    ("https://s3.amazonaws.com/setup.roblox.com", 4),
];

/// `versionStudio` is the hash of the last MFC Studio build and never changes.
const VERSION_STUDIO_HASH: &str = "version-012732894899482c";

/// A selected mirror, used to build package URLs.
#[derive(Debug, Clone)]
pub struct Mirror {
    pub base_url: String,
}

impl Mirror {
    pub fn new(base_url: impl Into<String>) -> Self {
        Mirror {
            base_url: base_url.into(),
        }
    }

    /// URL of a file under `/channel/common`, e.g. `/version-xyz-rbxPkgManifest.txt`.
    pub fn location(&self, resource: &str) -> String {
        format!("{}/channel/common{}", self.base_url, resource)
    }

    pub fn package_manifest_url(&self, version_guid: &str) -> String {
        self.location(&format!("/{version_guid}-rbxPkgManifest.txt"))
    }

    pub fn package_url(&self, version_guid: &str, package_name: &str) -> String {
        self.location(&format!("/{version_guid}-{package_name}"))
    }
}

async fn probe(client: &reqwest::Client, url: &str, delay_secs: u64) -> Result<String> {
    tokio::time::sleep(Duration::from_secs(delay_secs)).await;
    tracing::debug!(%url, "probing mirror");

    let body = client
        .get(format!("{url}/versionStudio"))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    if body.trim() != VERSION_STUDIO_HASH {
        return Err(Error::NoMirror(format!(
            "{url}: versionStudio returned {body:?}"
        )));
    }
    Ok(url.to_owned())
}

/// Race all mirrors and return the first one that answers correctly.
/// Doubles as the connectivity check.
pub async fn find_mirror(client: &reqwest::Client) -> Result<Mirror> {
    let mut probes: FuturesUnordered<_> = MIRRORS
        .iter()
        .map(|(url, delay)| probe(client, url, *delay))
        .collect();

    let mut last_error = String::from("no mirrors configured");
    while let Some(result) = probes.next().await {
        match result {
            Ok(url) => {
                tracing::info!(%url, "selected mirror");
                // dropping `probes` cancels the rest
                return Ok(Mirror::new(url));
            }
            Err(e) => {
                tracing::warn!(error = %e, "mirror probe failed");
                last_error = e.to_string();
            }
        }
    }
    Err(Error::NoMirror(last_error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_urls() {
        let m = Mirror::new("https://setup.rbxcdn.com");
        assert_eq!(
            m.package_manifest_url("version-abc"),
            "https://setup.rbxcdn.com/channel/common/version-abc-rbxPkgManifest.txt"
        );
        assert_eq!(
            m.package_url("version-abc", "RobloxApp.zip"),
            "https://setup.rbxcdn.com/channel/common/version-abc-RobloxApp.zip"
        );
    }
}
