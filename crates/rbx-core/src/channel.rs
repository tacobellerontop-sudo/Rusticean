//! Which deployment channel to install from. Order matches Bloxstrap:
//! explicit `-channel` flag, then `channel:<name>` in the launch URI, then the registry,
//! then `production`.

use std::sync::LazyLock;

use rbx_deploy::version::DEFAULT_CHANNEL;
use regex::Regex;

static URI_CHANNEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)channel:([a-z0-9_-]+)").expect("valid regex"));

pub fn from_launch_args(launch_args: &str) -> Option<String> {
    URI_CHANNEL
        .captures(launch_args)
        .map(|c| c[1].to_ascii_lowercase())
}

pub fn resolve(flag: Option<&str>, launch_args: &str, registry: Option<String>) -> String {
    if let Some(flag) = flag.filter(|f| !f.is_empty()) {
        return flag.to_ascii_lowercase();
    }
    if let Some(channel) = from_launch_args(launch_args) {
        return channel;
    }
    if let Some(channel) = registry.filter(|c| !c.is_empty()) {
        return channel.to_ascii_lowercase();
    }
    DEFAULT_CHANNEL.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence() {
        let uri = "roblox-player:1+launchmode:play+channel:ZBeta+browsertrackerid:1";
        assert_eq!(resolve(Some("Live"), uri, Some("reg".into())), "live");
        assert_eq!(resolve(None, uri, Some("reg".into())), "zbeta");
        assert_eq!(resolve(None, "roblox-player:1", Some("Reg".into())), "reg");
        assert_eq!(resolve(None, "", Some(String::new())), "production");
        assert_eq!(resolve(Some(""), "", None), "production");
    }
}
