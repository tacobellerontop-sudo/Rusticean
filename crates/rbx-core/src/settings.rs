//! `Settings.json`: what the user chose in the settings window. Bloxstrap keeps the
//! equivalent in `Models/Persistable/Settings.cs` and FastFlags in `FastFlagManager.cs`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Which graphics API Roblox should prefer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderingApi {
    /// Let Roblox decide.
    #[default]
    Automatic,
    Direct3D11,
    Vulkan,
    OpenGL,
}

impl RenderingApi {
    pub const ALL: [RenderingApi; 4] = [
        RenderingApi::Automatic,
        RenderingApi::Direct3D11,
        RenderingApi::Vulkan,
        RenderingApi::OpenGL,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RenderingApi::Automatic => "Automatic",
            RenderingApi::Direct3D11 => "Direct3D 11",
            RenderingApi::Vulkan => "Vulkan",
            RenderingApi::OpenGL => "OpenGL",
        }
    }

    fn flag(self) -> Option<&'static str> {
        match self {
            RenderingApi::Automatic => None,
            RenderingApi::Direct3D11 => Some("FFlagDebugGraphicsPreferD3D11"),
            RenderingApi::Vulkan => Some("FFlagDebugGraphicsPreferVulkan"),
            RenderingApi::OpenGL => Some("FFlagDebugGraphicsPreferOpenGL"),
        }
    }
}

/// Priority Roblox's process is started with (from Voidstrap's CPU priority option).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessPriority {
    Low,
    BelowNormal,
    #[default]
    Normal,
    AboveNormal,
    High,
}

impl ProcessPriority {
    pub const ALL: [ProcessPriority; 5] = [
        ProcessPriority::Low,
        ProcessPriority::BelowNormal,
        ProcessPriority::Normal,
        ProcessPriority::AboveNormal,
        ProcessPriority::High,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ProcessPriority::Low => "Low",
            ProcessPriority::BelowNormal => "Below normal",
            ProcessPriority::Normal => "Normal",
            ProcessPriority::AboveNormal => "Above normal",
            ProcessPriority::High => "High",
        }
    }

    /// The `CreateProcess` priority-class flag.
    pub fn creation_flag(self) -> u32 {
        match self {
            ProcessPriority::Low => 0x0000_0040, // IDLE_PRIORITY_CLASS
            ProcessPriority::BelowNormal => 0x0000_4000, // BELOW_NORMAL_PRIORITY_CLASS
            ProcessPriority::Normal => 0x0000_0020, // NORMAL_PRIORITY_CLASS
            ProcessPriority::AboveNormal => 0x0000_8000, // ABOVE_NORMAL_PRIORITY_CLASS
            ProcessPriority::High => 0x0000_0080, // HIGH_PRIORITY_CLASS
        }
    }
}

/// Anti-aliasing samples Roblox should force.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Msaa {
    #[default]
    Automatic,
    Off,
    X1,
    X2,
    X4,
}

impl Msaa {
    pub const ALL: [Msaa; 5] = [Msaa::Automatic, Msaa::Off, Msaa::X1, Msaa::X2, Msaa::X4];

    pub fn label(self) -> &'static str {
        match self {
            Msaa::Automatic => "Automatic",
            Msaa::Off => "Off",
            Msaa::X1 => "1×",
            Msaa::X2 => "2×",
            Msaa::X4 => "4×",
        }
    }

    fn samples(self) -> Option<u32> {
        match self {
            Msaa::Automatic => None,
            Msaa::Off => Some(0),
            Msaa::X1 => Some(1),
            Msaa::X2 => Some(2),
            Msaa::X4 => Some(4),
        }
    }
}

/// Texture quality override.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextureQuality {
    #[default]
    Automatic,
    Lowest,
    Low,
    Medium,
    High,
}

impl TextureQuality {
    pub const ALL: [TextureQuality; 5] = [
        TextureQuality::Automatic,
        TextureQuality::Lowest,
        TextureQuality::Low,
        TextureQuality::Medium,
        TextureQuality::High,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TextureQuality::Automatic => "Automatic",
            TextureQuality::Lowest => "Lowest",
            TextureQuality::Low => "Low",
            TextureQuality::Medium => "Medium",
            TextureQuality::High => "High",
        }
    }

    fn level(self) -> Option<u32> {
        match self {
            TextureQuality::Automatic => None,
            TextureQuality::Lowest => Some(0),
            TextureQuality::Low => Some(1),
            TextureQuality::Medium => Some(2),
            TextureQuality::High => Some(3),
        }
    }
}

/// How old cached downloads and logs must be before they're deleted (Fishstrap's cleaner).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CleanupAge {
    #[default]
    Never,
    OneDay,
    OneWeek,
    TwoWeeks,
    OneMonth,
}

impl CleanupAge {
    pub const ALL: [CleanupAge; 5] = [
        CleanupAge::Never,
        CleanupAge::OneDay,
        CleanupAge::OneWeek,
        CleanupAge::TwoWeeks,
        CleanupAge::OneMonth,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CleanupAge::Never => "Never",
            CleanupAge::OneDay => "After 1 day",
            CleanupAge::OneWeek => "After 1 week",
            CleanupAge::TwoWeeks => "After 2 weeks",
            CleanupAge::OneMonth => "After 1 month",
        }
    }

    pub fn max_age(self) -> Option<std::time::Duration> {
        const DAY: u64 = 24 * 60 * 60;
        let days = match self {
            CleanupAge::Never => return None,
            CleanupAge::OneDay => 1,
            CleanupAge::OneWeek => 7,
            CleanupAge::TwoWeeks => 14,
            CleanupAge::OneMonth => 30,
        };
        Some(std::time::Duration::from_secs(days * DAY))
    }
}

const FPS_FLAG: &str = "DFIntTaskSchedulerTargetFps";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Settings {
    /// Show the progress window while launching (off = like `-quiet`).
    pub show_progress_window: bool,
    /// Ask before launching when Roblox is already open (a second launch closes it).
    pub confirm_launches: bool,
    /// Check for a new Roblox version on every launch; off launches the installed one.
    pub auto_update: bool,
    pub process_priority: ProcessPriority,
    /// Delete old downloads and logs after launching.
    pub cleanup: CleanupAge,
    /// Deployment channel to always use; `None` follows the launch link and Roblox's own setting.
    pub channel: Option<String>,
    /// Frame rate cap; `None` keeps Roblox's default of 60.
    pub fps_limit: Option<u32>,
    pub rendering_api: RenderingApi,
    pub msaa: Msaa,
    pub texture_quality: TextureQuality,
    /// Stop Roblox from drawing grass.
    pub disable_grass: bool,
    /// Make Alt+Enter switch to real (exclusive) fullscreen instead of a borderless window.
    pub exclusive_fullscreen: bool,
    /// Extra FastFlags added by hand. Presets above win if they set the same flag.
    pub fast_flags: BTreeMap<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            show_progress_window: true,
            confirm_launches: true,
            auto_update: true,
            process_priority: ProcessPriority::Normal,
            cleanup: CleanupAge::Never,
            channel: None,
            fps_limit: None,
            rendering_api: RenderingApi::Automatic,
            msaa: Msaa::Automatic,
            texture_quality: TextureQuality::Automatic,
            disable_grass: false,
            exclusive_fullscreen: false,
            fast_flags: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// Load settings, falling back to defaults if the file is missing or unreadable.
    pub fn load(path: &Path) -> Settings {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Settings.json is corrupt, using defaults");
                Settings::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(e) => {
                tracing::warn!(error = %e, "could not read Settings.json, using defaults");
                Settings::default()
            }
        }
    }

    /// Write atomically: temp file, then rename over the old one.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(self).expect("Settings always serialize");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)
    }

    /// The FastFlags Roblox should see: hand-added ones, then presets on top.
    pub fn effective_fast_flags(&self) -> BTreeMap<String, Value> {
        let mut flags = self.fast_flags.clone();
        if let Some(fps) = self.fps_limit {
            flags.insert(FPS_FLAG.into(), Value::from(fps));
        }
        if let Some(flag) = self.rendering_api.flag() {
            flags.insert(flag.into(), Value::from(true));
        }
        if let Some(samples) = self.msaa.samples() {
            flags.insert("FIntDebugForceMSAASamples".into(), Value::from(samples));
        }
        if let Some(level) = self.texture_quality.level() {
            flags.insert(
                "DFFlagTextureQualityOverrideEnabled".into(),
                Value::from(true),
            );
            flags.insert("DFIntTextureQualityOverride".into(), Value::from(level));
        }
        if self.disable_grass {
            for flag in [
                "FIntFRMMinGrassDistance",
                "FIntFRMMaxGrassDistance",
                "FIntRenderGrassDetailStrands",
            ] {
                flags.insert(flag.into(), Value::from(0));
            }
        }
        if self.exclusive_fullscreen {
            flags.insert(
                "FFlagHandleAltEnterFullscreenManually".into(),
                Value::from(false),
            );
        }
        flags
    }
}

/// Parse what the user typed as a flag value: `true`/`false`, a whole number, or a string.
/// Roblox reads FastFlags as JSON, and only these three kinds matter.
pub fn parse_flag_value(text: &str) -> Value {
    let trimmed = text.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    match trimmed.parse::<i64>() {
        Ok(n) => Value::from(n),
        Err(_) => Value::String(trimmed.to_owned()),
    }
}

/// Write `ClientSettings/ClientAppSettings.json` into a version folder, or remove it when
/// there are no flags so Roblox runs untouched.
pub fn write_client_settings(
    version_dir: &Path,
    flags: &BTreeMap<String, Value>,
) -> std::io::Result<()> {
    let dir = version_dir.join("ClientSettings");
    let file = dir.join("ClientAppSettings.json");
    if flags.is_empty() {
        return match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(flags).expect("flags always serialize");
    std::fs::write(file, json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Settings.json");
        assert_eq!(Settings::load(&path), Settings::default());

        let mut s = Settings {
            fps_limit: Some(144),
            channel: Some("zbeta".into()),
            ..Default::default()
        };
        s.fast_flags.insert("FFlagX".into(), Value::Bool(true));
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);

        std::fs::write(&path, "nope").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
    }

    #[test]
    fn older_files_get_new_fields_defaulted() {
        let s: Settings = serde_json::from_str(r#"{"FpsLimit":240}"#).unwrap();
        assert_eq!(s.fps_limit, Some(240));
        assert!(s.show_progress_window);
    }

    #[test]
    fn presets_override_hand_added_flags() {
        let mut s = Settings {
            fps_limit: Some(120),
            rendering_api: RenderingApi::Vulkan,
            ..Default::default()
        };
        s.fast_flags.insert(FPS_FLAG.into(), Value::from(30));
        s.fast_flags.insert("FIntSomething".into(), Value::from(5));

        let flags = s.effective_fast_flags();
        assert_eq!(flags[FPS_FLAG], Value::from(120));
        assert_eq!(flags["FFlagDebugGraphicsPreferVulkan"], Value::Bool(true));
        assert_eq!(flags["FIntSomething"], Value::from(5));
        assert!(Settings::default().effective_fast_flags().is_empty());
    }

    #[test]
    fn graphics_presets_map_to_flags() {
        let s = Settings {
            msaa: Msaa::X4,
            texture_quality: TextureQuality::Lowest,
            disable_grass: true,
            exclusive_fullscreen: true,
            ..Default::default()
        };
        let flags = s.effective_fast_flags();
        assert_eq!(flags["FIntDebugForceMSAASamples"], Value::from(4));
        assert_eq!(
            flags["DFFlagTextureQualityOverrideEnabled"],
            Value::Bool(true)
        );
        assert_eq!(flags["DFIntTextureQualityOverride"], Value::from(0));
        assert_eq!(flags["FIntFRMMaxGrassDistance"], Value::from(0));
        assert_eq!(
            flags["FFlagHandleAltEnterFullscreenManually"],
            Value::Bool(false)
        );
    }

    #[test]
    fn cleanup_ages() {
        assert_eq!(CleanupAge::Never.max_age(), None);
        assert_eq!(
            CleanupAge::OneWeek.max_age(),
            Some(std::time::Duration::from_secs(7 * 24 * 3600))
        );
    }

    #[test]
    fn parses_flag_values() {
        assert_eq!(parse_flag_value(" True "), Value::Bool(true));
        assert_eq!(parse_flag_value("false"), Value::Bool(false));
        assert_eq!(parse_flag_value("-12"), Value::from(-12));
        assert_eq!(parse_flag_value("hello"), Value::String("hello".into()));
    }

    #[test]
    fn writes_and_removes_client_settings() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ClientSettings/ClientAppSettings.json");

        let mut flags = BTreeMap::new();
        flags.insert(FPS_FLAG.to_owned(), Value::from(144));
        write_client_settings(dir.path(), &flags).unwrap();
        let written: BTreeMap<String, Value> =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(written, flags);

        write_client_settings(dir.path(), &BTreeMap::new()).unwrap();
        assert!(!file.exists());
        // removing twice is fine
        write_client_settings(dir.path(), &BTreeMap::new()).unwrap();
    }
}
