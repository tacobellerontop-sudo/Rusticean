//! Talking to Roblox's deployment infrastructure: finding a CDN mirror, asking which
//! client version is current, and downloading/extracting its packages.
//!
//! Ported from Bloxstrap's `RobloxInterfaces/Deployment.cs`, `Models/Manifest/*`,
//! `AppData/*` and the download/extract half of `Bootstrapper.cs`.

pub mod binary;
pub mod cdn;
pub mod error;
pub mod hash;
pub mod install;
pub mod manifest;
pub mod version;

pub use binary::BinaryType;
pub use error::{Error, Result};
pub use manifest::Package;
pub use reqwest;
