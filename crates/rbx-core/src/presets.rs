//! Bloxstrap's mod presets (classic cursors, old character sounds, the old avatar
//! background, emoji fonts and a custom font), written into [`Paths::presets`] before
//! every launch. Files in the user's own Modifications folder win over these.

use std::io;
use std::path::Path;

use rbx_deploy::reqwest;

use crate::paths::Paths;
use crate::settings::{CursorStyle, EmojiStyle, Settings};

const CURSOR_DIR: &str = "content/textures/Cursors/KeyboardMouse";
const CUSTOM_FONT: &str = "content/fonts/CustomFont.ttf";
const CUSTOM_FONT_ASSET: &str = "rbxasset://fonts/CustomFont.ttf";

macro_rules! asset {
    ($path:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/mods/",
            $path
        ))
        .as_slice()
    };
}

/// Rebuild the presets folder from `settings`. `version_dir` is read for the font
/// families a custom font has to replace.
pub async fn sync(
    paths: &Paths,
    settings: &Settings,
    version_dir: &Path,
    client: &reqwest::Client,
) -> io::Result<()> {
    let dir = &paths.presets;
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    std::fs::create_dir_all(dir)?;

    let cursors = match settings.cursor {
        CursorStyle::Default => None,
        CursorStyle::From2006 => Some([
            asset!("Cursor/From2006/ArrowCursor.png"),
            asset!("Cursor/From2006/ArrowFarCursor.png"),
        ]),
        CursorStyle::From2013 => Some([
            asset!("Cursor/From2013/ArrowCursor.png"),
            asset!("Cursor/From2013/ArrowFarCursor.png"),
        ]),
    };
    if let Some([arrow, far]) = cursors {
        write(dir, &format!("{CURSOR_DIR}/ArrowCursor.png"), arrow)?;
        write(dir, &format!("{CURSOR_DIR}/ArrowFarCursor.png"), far)?;
    }

    if settings.old_character_sounds {
        let empty = asset!("Sounds/Empty.mp3");
        for (file, data) in [
            ("action_footsteps_plastic.mp3", asset!("Sounds/OldWalk.mp3")),
            ("action_jump.mp3", asset!("Sounds/OldJump.mp3")),
            ("action_get_up.mp3", asset!("Sounds/OldGetUp.mp3")),
            ("action_falling.mp3", empty),
            ("action_jump_land.mp3", empty),
            ("action_swim.mp3", empty),
            ("impact_water.mp3", empty),
        ] {
            write(dir, &format!("content/sounds/{file}"), data)?;
        }
    }

    if settings.old_avatar_background {
        write(
            dir,
            "ExtraContent/places/Mobile.rbxl",
            asset!("OldAvatarBackground.rbxl"),
        )?;
    }

    if let Some(url) = settings.emoji.url() {
        match emoji_font(paths, settings.emoji, &url, client).await {
            Ok(font) => write(dir, "content/fonts/TwemojiMozilla.ttf", &font)?,
            // keep launching with Roblox's own emoji
            Err(e) => tracing::warn!(error = %e, "could not download the emoji font"),
        }
    }

    if settings.custom_font.is_some() && paths.custom_font.is_file() {
        let font = std::fs::read(&paths.custom_font)?;
        write(dir, CUSTOM_FONT, &font)?;
        point_font_families_at_custom_font(version_dir, dir)?;
    }
    Ok(())
}

/// The emoji font, downloaded once into the cache.
async fn emoji_font(
    paths: &Paths,
    style: EmojiStyle,
    url: &str,
    client: &reqwest::Client,
) -> Result<Vec<u8>, BoxError> {
    let cached = paths.cache.join("emoji").join(format!("{style:?}.ttf"));
    if let Ok(bytes) = std::fs::read(&cached) {
        return Ok(bytes);
    }
    let bytes = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?
        .to_vec();
    if let Some(parent) = cached.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&cached, &bytes)?;
    Ok(bytes)
}

/// Copy Roblox's font family files with every face pointed at the custom font.
fn point_font_families_at_custom_font(version_dir: &Path, presets: &Path) -> io::Result<()> {
    let families = version_dir.join("content/fonts/families");
    let Ok(entries) = std::fs::read_dir(&families) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let Ok(mut family) = std::fs::read_to_string(&path)
            .map_err(|_| ())
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).map_err(|_| ()))
        else {
            continue;
        };
        let Some(faces) = family.get_mut("faces").and_then(|f| f.as_array_mut()) else {
            continue;
        };
        for face in faces {
            face["assetId"] = CUSTOM_FONT_ASSET.into();
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let json = serde_json::to_string_pretty(&family).map_err(io::Error::other)?;
        write(
            presets,
            &format!("content/fonts/families/{name}"),
            json.as_bytes(),
        )?;
    }
    Ok(())
}

fn write(dir: &Path, rel: &str, data: &[u8]) -> io::Result<()> {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, data)
}

/// Errors from the emoji download: network or disk.
type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn writes_selected_presets() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let version = dir.path().join("version");
        let families = version.join("content/fonts/families");
        std::fs::create_dir_all(&families).unwrap();
        std::fs::write(
            families.join("Arial.json"),
            r#"{"name":"Arial","faces":[{"name":"Regular","assetId":"rbxasset://fonts/arial.ttf"}]}"#,
        )
        .unwrap();
        std::fs::write(&paths.custom_font, b"font").unwrap();

        let settings = Settings {
            cursor: CursorStyle::From2013,
            old_character_sounds: true,
            custom_font: Some("Comic.ttf".into()),
            ..Settings::default()
        };
        sync(&paths, &settings, &version, &reqwest::Client::new())
            .await
            .unwrap();

        let p = &paths.presets;
        assert!(p.join(CURSOR_DIR).join("ArrowCursor.png").is_file());
        assert!(p.join("content/sounds/action_jump.mp3").is_file());
        assert!(!p.join("ExtraContent/places/Mobile.rbxl").exists());
        assert_eq!(std::fs::read(p.join(CUSTOM_FONT)).unwrap(), b"font");
        let family = std::fs::read_to_string(p.join("content/fonts/families/Arial.json")).unwrap();
        assert!(family.contains(CUSTOM_FONT_ASSET));
    }
}
