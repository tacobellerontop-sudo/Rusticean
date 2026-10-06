//! Parser for `<guid>-rbxPkgManifest.txt`.
//!
//! Format: a `v0` header line, then four lines per package: file name, MD5 signature,
//! packed (zip) size and unpacked size, all in bytes.

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    /// Lowercase hex MD5 of the zip as served by the CDN.
    pub signature: String,
    pub packed_size: u64,
    pub size: u64,
}

pub fn parse(data: &str) -> Result<Vec<Package>> {
    let mut lines = data.lines().map(str::trim);

    match lines.next() {
        Some("v0") => {}
        other => {
            return Err(Error::BadManifest(format!(
                "expected header 'v0', got {other:?}"
            )));
        }
    }

    let mut packages = Vec::new();

    while let (Some(name), Some(signature), Some(packed), Some(size)) =
        (lines.next(), lines.next(), lines.next(), lines.next())
    {
        if name.is_empty() || signature.is_empty() || packed.is_empty() || size.is_empty() {
            break;
        }

        // the stock launcher is listed last and isn't something we install
        if name == "RobloxPlayerLauncher.exe" {
            break;
        }

        let parse_size = |raw: &str| {
            raw.parse::<u64>()
                .map_err(|_| Error::BadManifest(format!("bad size '{raw}' for {name}")))
        };

        packages.push(Package {
            name: name.to_owned(),
            signature: signature.to_ascii_lowercase(),
            packed_size: parse_size(packed)?,
            size: parse_size(size)?,
        });
    }

    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "v0\r\nRobloxApp.zip\r\n0123456789ABCDEF0123456789abcdef\r\n100\r\n250\r\nshaders.zip\r\nfedcba9876543210fedcba9876543210\r\n10\r\n20\r\nRobloxPlayerLauncher.exe\r\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\r\n5\r\n5\r\n";

    #[test]
    fn parses_packages_and_stops_at_launcher() {
        let pkgs = parse(SAMPLE).unwrap();
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].name, "RobloxApp.zip");
        assert_eq!(pkgs[0].signature, "0123456789abcdef0123456789abcdef");
        assert_eq!(pkgs[0].packed_size, 100);
        assert_eq!(pkgs[0].size, 250);
        assert_eq!(pkgs[1].name, "shaders.zip");
    }

    #[test]
    fn rejects_unknown_version() {
        assert!(matches!(parse("v1\n"), Err(Error::BadManifest(_))));
        assert!(matches!(parse(""), Err(Error::BadManifest(_))));
    }

    #[test]
    fn tolerates_truncated_trailing_entry() {
        let pkgs = parse("v0\nA.zip\nabc\n1\n2\nB.zip\nabc\n").unwrap();
        assert_eq!(pkgs.len(), 1);
    }

    #[test]
    fn rejects_non_numeric_size() {
        assert!(parse("v0\nA.zip\nabc\nlots\n2\n").is_err());
    }
}
