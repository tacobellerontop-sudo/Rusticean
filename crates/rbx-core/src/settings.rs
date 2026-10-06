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

/// A settings dropdown: an enum with a label per variant, defaulting to the first one.
macro_rules! choice {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $label:expr),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name { $($variant),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub fn label(self) -> &'static str {
                match self { $($name::$variant => $label),+ }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::ALL[0]
            }
        }
    };
}

choice! {
    /// Bloxstrap's classic cursor presets.
    CursorStyle { Default => "Default", From2013 => "2013 (Angular)", From2006 => "2006 (Cartoony)" }
}

choice! {
    /// Emoji fonts from bloxstraplabs/rbxcustom-fontemojis.
    EmojiStyle {
        Default => "Default (Twemoji)",
        Catmoji => "Catmoji",
        Windows11 => "Windows 11",
        Windows10 => "Windows 10",
        Windows8 => "Windows 8",
    }
}

const EMOJI_BASE: &str = "https://github.com/bloxstraplabs/rbxcustom-fontemojis/releases/download/my-phone-is-78-percent/";

impl EmojiStyle {
    /// Download URL for the font, `None` for Roblox's own.
    pub fn url(self) -> Option<String> {
        let file = match self {
            EmojiStyle::Default => return None,
            EmojiStyle::Catmoji => "Catmoji.ttf",
            EmojiStyle::Windows11 => "Win1122H2SegoeUIEmoji.ttf",
            EmojiStyle::Windows10 => "Win10April2018SegoeUIEmoji.ttf",
            EmojiStyle::Windows8 => "Win8.1SegoeUIEmoji.ttf",
        };
        Some(format!("{EMOJI_BASE}{file}"))
    }
}

choice! {
    Theme { Dark => "Dark", Light => "Light", System => "System default" }
}

choice! {
    /// How the progress window looks.
    BootstrapperStyle {
        Rusticean => "Rusticean",
        Compact => "Compact",
        Classic => "Classic dialog",
    }
}

choice! {
    BootstrapperIcon {
        Rusticean => "Rusticean",
        Outline => "Rusticean outline",
        Mono => "Rusticean mono",
        Custom => "Custom",
    }
}

/// Another program to start alongside Roblox (Bloxstrap's custom integrations).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct CustomIntegration {
    pub name: String,
    pub path: String,
    pub args: String,
    /// Close it again when Roblox closes.
    pub auto_close: bool,
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
    /// Windows' "disable fullscreen optimizations" compatibility flag on RobloxPlayerBeta.exe.
    pub disable_fullscreen_optimizations: bool,

    // Risky: these can get an account warned or banned. Off by default.
    /// Hold Roblox's single-instance mutex so several clients can run at once.
    pub multi_instance: bool,
    /// Press a key in Roblox every few minutes so it doesn't kick you for being idle.
    pub anti_afk: bool,
    /// Remove the lighting lookup texture so everything renders fully lit.
    pub fullbright: bool,

    // Integrations
    /// Read Roblox's log to know which game you're in. Needed by the options below.
    pub activity_tracking: bool,
    /// Show a notification with the server's location when you join.
    pub server_location: bool,
    /// Close Roblox when you leave a game instead of going back to the desktop app.
    pub close_on_leave: bool,
    /// Show the game you're playing on your Discord profile.
    pub discord_presence: bool,
    /// Add a "Join server" button to the Discord activity.
    pub discord_join_button: bool,
    /// Show your Roblox avatar and name on the Discord activity.
    pub discord_show_account: bool,
    pub custom_integrations: Vec<CustomIntegration>,

    // Mod presets
    pub cursor: CursorStyle,
    pub old_avatar_background: bool,
    pub old_character_sounds: bool,
    pub emoji: EmojiStyle,
    /// Original name of the font file copied into the data folder, if one is set.
    pub custom_font: Option<String>,
    /// Windows' "override high DPI scaling" compatibility option on Roblox.
    pub dpi_override: bool,

    // Engine
    /// Off leaves ClientAppSettings.json alone entirely.
    pub manage_fast_flags: bool,
    /// Keep Roblox rendering at full resolution when Windows display scaling is above 100%.
    pub preserve_rendering_quality: bool,

    // Appearance
    pub theme: Theme,
    pub bootstrapper_style: BootstrapperStyle,
    pub bootstrapper_icon: BootstrapperIcon,
    /// Path to a .png or .ico for [`BootstrapperIcon::Custom`].
    pub custom_icon: Option<String>,
    pub bootstrapper_title: String,

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
            disable_fullscreen_optimizations: false,
            multi_instance: false,
            anti_afk: false,
            fullbright: false,
            activity_tracking: true,
            server_location: false,
            close_on_leave: false,
            discord_presence: true,
            discord_join_button: false,
            discord_show_account: false,
            custom_integrations: Vec::new(),
            cursor: CursorStyle::default(),
            old_avatar_background: false,
            old_character_sounds: false,
            emoji: EmojiStyle::default(),
            custom_font: None,
            dpi_override: false,
            manage_fast_flags: true,
            preserve_rendering_quality: false,
            theme: Theme::default(),
            bootstrapper_style: BootstrapperStyle::default(),
            bootstrapper_icon: BootstrapperIcon::default(),
            custom_icon: None,
            bootstrapper_title: "Rusticean".into(),
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
        if self.preserve_rendering_quality {
            flags.insert("DFFlagDisableDPIScale".into(), Value::from(true));
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
