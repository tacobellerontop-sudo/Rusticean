//! Protocol registration and Roblox's channel key, ported from Bloxstrap's
//! `Utility/WindowsRegistry.cs` and `Bootstrapper.cs`.

use std::io;
use std::path::Path;

/// The value Roblox stores the selected channel under.
const CHANNEL_VALUE: &str = "www.roblox.com";

fn channel_key(registry_name: &str) -> String {
    format!(r"SOFTWARE\ROBLOX Corporation\Environments\{registry_name}\Channel")
}

/// Make `roblox://` and `roblox-player://` links open `exe -player "<uri>"`.
pub fn register_player_protocol(exe: &Path) -> io::Result<()> {
    let handler = exe.display().to_string();
    for scheme in ["roblox", "roblox-player"] {
        register_protocol(scheme, "Roblox", &handler, r#"-player "%1""#)?;
    }
    Ok(())
}

#[cfg(windows)]
fn register_protocol(scheme: &str, name: &str, handler: &str, param: &str) -> io::Result<()> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(format!(r"Software\Classes\{scheme}"))?;
    let (icon, _) = key.create_subkey("DefaultIcon")?;
    let (command, _) = key.create_subkey(r"shell\open\command")?;

    if key.get_value::<String, _>("").is_err() {
        key.set_value("", &format!("URL: {name} Protocol"))?;
        key.set_value("URL Protocol", &"")?;
    }
    icon.set_value("", &handler)?;
    command.set_value("", &format!("\"{handler}\" {param}"))?;
    Ok(())
}

#[cfg(not(windows))]
fn register_protocol(_scheme: &str, _name: &str, _handler: &str, _param: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "protocol registration is only supported on Windows",
    ))
}

/// The channel Roblox last used for this product, if one is set.
#[cfg(windows)]
pub fn read_channel(registry_name: &str) -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(channel_key(registry_name))
        .ok()?
        .get_value::<String, _>(CHANNEL_VALUE)
        .ok()
        .filter(|v| !v.is_empty())
}

#[cfg(not(windows))]
pub fn read_channel(registry_name: &str) -> Option<String> {
    let _ = (channel_key(registry_name), CHANNEL_VALUE);
    None
}

/// Store the channel the way Roblox does (empty string means production).
#[cfg(windows)]
pub fn write_channel(registry_name: &str, channel: &str) -> io::Result<()> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(channel_key(registry_name))?;
    key.set_value(CHANNEL_VALUE, &channel)
}

#[cfg(not(windows))]
pub fn write_channel(_registry_name: &str, _channel: &str) -> io::Result<()> {
    Ok(())
}
