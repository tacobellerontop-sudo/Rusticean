//! Asking Roblox which client version is current for a channel.

use reqwest::StatusCode;
use serde::Deserialize;

use crate::{BinaryType, Error, Result};

pub const DEFAULT_CHANNEL: &str = "production";

const HOSTS: &[&str] = &[
    "https://clientsettingscdn.roblox.com",
    "https://clientsettings.roblox.com",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientVersion {
    /// Human-readable version, e.g. `0.650.0.6500712`.
    pub version: String,
    /// The version GUID used in CDN paths, e.g. `version-1a2b3c4d5e6f7a8b`.
    #[serde(rename = "clientVersionUpload")]
    pub version_guid: String,
}

pub fn is_default_channel(channel: &str) -> bool {
    channel.eq_ignore_ascii_case(DEFAULT_CHANNEL)
}

fn path_for(binary: BinaryType, channel: &str) -> String {
    if is_default_channel(channel) {
        format!("/v2/client-version/{}", binary.api_name())
    } else {
        format!("/v2/client-version/{}/channel/{channel}", binary.api_name())
    }
}

/// Fetch the current version for `channel`, trying the CDN host first and falling back
/// to the origin. Returns [`Error::InvalidChannel`] when a non-default channel is refused.
pub async fn fetch(
    client: &reqwest::Client,
    binary: BinaryType,
    channel: &str,
) -> Result<ClientVersion> {
    let path = path_for(binary, channel);
    let mut last_error = None;

    for host in HOSTS {
        let url = format!("{host}{path}");
        tracing::debug!(%url, "fetching client version");

        let response = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(%url, error = %e, "client version request failed");
                last_error = Some(e.into());
                continue;
            }
        };

        let status = response.status();
        if !is_default_channel(channel)
            && matches!(
                status,
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN | StatusCode::NOT_FOUND
            )
        {
            return Err(Error::InvalidChannel(channel.to_owned()));
        }

        match response.error_for_status() {
            Ok(r) => return Ok(r.json().await?),
            Err(e) => {
                tracing::warn!(%url, error = %e, "client version request failed");
                last_error = Some(e.into());
            }
        }
    }

    Err(last_error.expect("HOSTS is non-empty"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_paths() {
        assert_eq!(
            path_for(BinaryType::WindowsPlayer, "production"),
            "/v2/client-version/WindowsPlayer"
        );
        assert_eq!(
            path_for(BinaryType::WindowsPlayer, "Production"),
            "/v2/client-version/WindowsPlayer"
        );
        assert_eq!(
            path_for(BinaryType::WindowsPlayer, "zbeta"),
            "/v2/client-version/WindowsPlayer/channel/zbeta"
        );
    }

    #[test]
    fn parses_response() {
        let json = r#"{"version":"0.650.0.6500712","clientVersionUpload":"version-1a2b3c4d5e6f7a8b","bootstrapperVersion":"1, 6, 0, 6500712"}"#;
        let v: ClientVersion = serde_json::from_str(json).unwrap();
        assert_eq!(v.version_guid, "version-1a2b3c4d5e6f7a8b");
        assert_eq!(v.version, "0.650.0.6500712");
    }
}
