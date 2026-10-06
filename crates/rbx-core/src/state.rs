//! `State.json`: what we've installed. Bloxstrap's `Models/Persistable/State.cs`.

use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase", default)]
pub struct DistributionState {
    /// The version GUID currently extracted in `Versions/`.
    pub version_guid: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase", default)]
pub struct State {
    pub player: DistributionState,
}

impl State {
    /// Load state, falling back to defaults if the file is missing or unreadable.
    pub fn load(path: &Path) -> State {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "State.json is corrupt, starting fresh");
                State::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(e) => {
                tracing::warn!(error = %e, "could not read State.json, starting fresh");
                State::default()
            }
        }
    }

    /// Write atomically: temp file, then rename over the old one.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(self).expect("State always serializes");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("State.json");

        assert_eq!(State::load(&path), State::default());

        let mut state = State::default();
        state.player.version_guid = Some("version-abc".into());
        state.save(&path).unwrap();
        assert_eq!(State::load(&path), state);

        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(State::load(&path), State::default());
    }
}
