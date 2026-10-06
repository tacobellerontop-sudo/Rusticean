//! Which Roblox product we're dealing with, and where each of its packages is extracted to.
//! The directory maps come from Bloxstrap's `AppData/CommonAppData.cs`,
//! `RobloxPlayerData.cs` and `RobloxStudioData.cs`.

/// A Roblox product as named by the deployment API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryType {
    WindowsPlayer,
    WindowsStudio64,
}

const COMMON_DIRS: &[(&str, &str)] = &[
    ("Libraries.zip", ""),
    ("redist.zip", ""),
    ("shaders.zip", "shaders"),
    ("ssl.zip", "ssl"),
    ("WebView2.zip", ""),
    ("WebView2RuntimeInstaller.zip", "WebView2RuntimeInstaller"),
    ("content-avatar.zip", "content/avatar"),
    ("content-configs.zip", "content/configs"),
    ("content-fonts.zip", "content/fonts"),
    ("content-sky.zip", "content/sky"),
    ("content-sounds.zip", "content/sounds"),
    ("content-textures2.zip", "content/textures"),
    ("content-models.zip", "content/models"),
    ("content-textures3.zip", "PlatformContent/pc/textures"),
    ("content-terrain.zip", "PlatformContent/pc/terrain"),
    ("content-platform-fonts.zip", "PlatformContent/pc/fonts"),
    (
        "content-platform-dictionaries.zip",
        "PlatformContent/pc/shared_compression_dictionaries",
    ),
    ("extracontent-luapackages.zip", "ExtraContent/LuaPackages"),
    ("extracontent-translations.zip", "ExtraContent/translations"),
    ("extracontent-models.zip", "ExtraContent/models"),
    ("extracontent-textures.zip", "ExtraContent/textures"),
    ("extracontent-places.zip", "ExtraContent/places"),
];

const PLAYER_DIRS: &[(&str, &str)] = &[("RobloxApp.zip", "")];

const STUDIO_DIRS: &[(&str, &str)] = &[
    ("RobloxStudio.zip", ""),
    ("LibrariesQt5.zip", ""),
    (
        "content-studio_svg_textures.zip",
        "content/studio_svg_textures",
    ),
    ("content-qt_translations.zip", "content/qt_translations"),
    ("content-api-docs.zip", "content/api_docs"),
    ("extracontent-scripts.zip", "ExtraContent/scripts"),
    ("studiocontent-models.zip", "StudioContent/models"),
    ("studiocontent-textures.zip", "StudioContent/textures"),
    ("BuiltInPlugins.zip", "BuiltInPlugins"),
    ("BuiltInStandalonePlugins.zip", "BuiltInStandalonePlugins"),
    ("ApplicationConfig.zip", "ApplicationConfig"),
    ("Plugins.zip", "Plugins"),
    ("Qml.zip", "Qml"),
    ("StudioFonts.zip", "StudioFonts"),
    ("RibbonConfig.zip", "RibbonConfig"),
];

impl BinaryType {
    /// Name used in `clientsettings` URLs.
    pub fn api_name(self) -> &'static str {
        match self {
            BinaryType::WindowsPlayer => "WindowsPlayer",
            BinaryType::WindowsStudio64 => "WindowsStudio64",
        }
    }

    /// Name of the key under `HKCU\SOFTWARE\ROBLOX Corporation\Environments`.
    pub fn registry_name(self) -> &'static str {
        match self {
            BinaryType::WindowsPlayer => "RobloxPlayer",
            BinaryType::WindowsStudio64 => "RobloxStudio",
        }
    }

    pub fn executable_name(self) -> &'static str {
        match self {
            BinaryType::WindowsPlayer => "RobloxPlayerBeta.exe",
            BinaryType::WindowsStudio64 => "RobloxStudioBeta.exe",
        }
    }

    pub fn process_name(self) -> &'static str {
        match self {
            BinaryType::WindowsPlayer => "RobloxPlayerBeta",
            BinaryType::WindowsStudio64 => "RobloxStudioBeta",
        }
    }

    /// Directory (relative to the version folder, `/`-separated) a package extracts into,
    /// or `None` if we don't know the package.
    pub fn package_dir(self, package_name: &str) -> Option<&'static str> {
        let specific = match self {
            BinaryType::WindowsPlayer => PLAYER_DIRS,
            BinaryType::WindowsStudio64 => STUDIO_DIRS,
        };
        specific
            .iter()
            .chain(COMMON_DIRS)
            .find(|(name, _)| *name == package_name)
            .map(|(_, dir)| *dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_packages() {
        let p = BinaryType::WindowsPlayer;
        assert_eq!(p.package_dir("RobloxApp.zip"), Some(""));
        assert_eq!(p.package_dir("content-fonts.zip"), Some("content/fonts"));
        assert_eq!(p.package_dir("RobloxStudio.zip"), None);
        assert_eq!(
            BinaryType::WindowsStudio64.package_dir("Qml.zip"),
            Some("Qml")
        );
        assert_eq!(p.package_dir("nonsense.zip"), None);
    }
}
