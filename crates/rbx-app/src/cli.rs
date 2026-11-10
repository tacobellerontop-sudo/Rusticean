//! Command-line parsing. Bloxstrap-style single-dash flags, because that's what the
//! protocol handler and Bloxstrap users expect: `-player "roblox-player:1+..."`.

use rbx_core::{LaunchMode, LaunchOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Install/update the player or Studio and launch it.
    Launch(Options),
    /// Open the settings window (the default when run with no arguments).
    Settings,
    /// Point roblox:// links at this exe and exit (run by the installer).
    Register,
    /// Undo `Register` (run by the uninstaller).
    Unregister,
    /// Show the progress window with a fake install (settings' Preview button).
    Preview,
    Help,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    pub mode: LaunchMode,
    pub launch_args: String,
    pub channel: Option<String>,
    pub version_guid: Option<String>,
    pub force: bool,
    pub no_launch: bool,
    /// No window: errors only go to the log and a message box.
    pub quiet: bool,
}

impl From<Options> for LaunchOptions {
    fn from(o: Options) -> Self {
        LaunchOptions {
            mode: o.mode,
            launch_args: o.launch_args,
            channel: o.channel,
            version_guid: o.version_guid,
            force: o.force,
            no_launch: o.no_launch,
        }
    }
}

/// Which app a bare `roblox…:` URI is for, or `None` if `arg` isn't one.
fn uri_mode(arg: &str) -> Option<LaunchMode> {
    let lower = arg.to_ascii_lowercase();
    if lower.starts_with("roblox-studio:") || lower.starts_with("roblox-studio-auth:") {
        Some(LaunchMode::Studio)
    } else if lower.starts_with("roblox-player:") || lower.starts_with("roblox:") {
        Some(LaunchMode::Player)
    } else {
        None
    }
}

pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Command, String> {
    let mut args = args.into_iter().peekable();
    let mut opts = Options::default();
    let mut any_player_arg = false;

    // value of a flag: the next argument, unless it's another flag
    fn value(args: &mut std::iter::Peekable<impl Iterator<Item = String>>) -> Option<String> {
        args.next_if(|a| !a.starts_with('-'))
    }

    while let Some(arg) = args.next() {
        if let Some(mode) = uri_mode(&arg) {
            opts.mode = mode;
            opts.launch_args = arg;
            any_player_arg = true;
            continue;
        }

        let Some(flag) = arg.strip_prefix('-') else {
            return Err(format!("unexpected argument '{arg}'"));
        };

        let flag = flag.to_ascii_lowercase();
        match flag.as_str() {
            "settings" | "menu" | "preferences" => return Ok(Command::Settings),
            "register" => return Ok(Command::Register),
            "unregister" | "uninstall" => return Ok(Command::Unregister),
            "preview" => return Ok(Command::Preview),
            _ => {}
        }
        any_player_arg = true;

        match flag.as_str() {
            "player" => {
                opts.mode = LaunchMode::Player;
                opts.launch_args = value(&mut args).unwrap_or_default();
            }
            "studio" => {
                opts.mode = LaunchMode::Studio;
                opts.launch_args = value(&mut args).unwrap_or_default();
            }
            "channel" => {
                opts.channel = Some(value(&mut args).ok_or("-channel needs a channel name")?)
            }
            "version" => {
                opts.version_guid = Some(value(&mut args).ok_or("-version needs a version GUID")?)
            }
            "force" => opts.force = true,
            "nolaunch" => opts.no_launch = true,
            "quiet" => opts.quiet = true,
            "help" | "h" | "?" | "-help" => return Ok(Command::Help),
            other => return Err(format!("unknown flag '-{other}'")),
        }
    }

    if any_player_arg {
        Ok(Command::Launch(opts))
    } else {
        Ok(Command::Settings)
    }
}

pub const USAGE: &str = "\
Rusticean: installs, updates and launches Roblox

USAGE:
    Rusticean                          Open settings
    Rusticean -player [URI] [FLAGS]    Install/update Roblox and launch it
    Rusticean -studio [URI|FILE]       Install/update Roblox Studio and launch it

Launching registers it as the handler for roblox:// and roblox-player:// links; launching
Studio also takes roblox-studio:// links and .rbxl/.rbxlx files.

FLAGS:
    -player [URI]      Launch the player, optionally with a roblox-player: URI
    -studio [URI|FILE] Launch Studio, optionally with a roblox-studio: URI or a place file
    -settings          Open the settings window
    -channel <name>    Use this deployment channel instead of the default
    -version <guid>    Install this exact version (e.g. version-1a2b3c4d5e6f7a8b)
    -force             Reinstall even if the version is already present
    -nolaunch          Install or update, but don't start Roblox
    -quiet             Don't show the progress window
    -register          Only register the roblox:// link handler
    -unregister        Remove the link handler (used by the uninstaller)
    -help              Show this message
";

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Command, String> {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn no_args_opens_settings() {
        assert_eq!(p(&[]), Ok(Command::Settings));
        assert_eq!(p(&["-settings"]), Ok(Command::Settings));
        assert_eq!(p(&["-player"]), Ok(Command::Launch(Options::default())));
    }

    #[test]
    fn installer_commands() {
        assert_eq!(p(&["-register"]), Ok(Command::Register));
        assert_eq!(p(&["-uninstall"]), Ok(Command::Unregister));
    }

    #[test]
    fn protocol_handler_invocation() {
        let uri = "roblox-player:1+launchmode:play+gameinfo:abc+placelauncherurl:x";
        let Ok(Command::Launch(o)) = p(&["-player", uri]) else {
            panic!()
        };
        assert_eq!(o.launch_args, uri);
    }

    #[test]
    fn bare_uri_and_flags() {
        let Ok(Command::Launch(o)) = p(&[
            "roblox-player:1",
            "-channel",
            "ZBeta",
            "-force",
            "-nolaunch",
            "-quiet",
        ]) else {
            panic!()
        };
        assert_eq!(o.launch_args, "roblox-player:1");
        assert_eq!(o.channel.as_deref(), Some("ZBeta"));
        assert!(o.force && o.no_launch && o.quiet);
    }

    #[test]
    fn player_without_uri() {
        let Ok(Command::Launch(o)) = p(&["-player", "-force"]) else {
            panic!()
        };
        assert_eq!(o.launch_args, "");
        assert!(o.force);
    }

    #[test]
    fn studio() {
        let Ok(Command::Launch(o)) = p(&["-studio"]) else {
            panic!()
        };
        assert_eq!(o.mode, LaunchMode::Studio);
        assert_eq!(o.launch_args, "");

        let uri = "roblox-studio:1+launchmode:edit+task:EditPlace+placeId:1";
        let Ok(Command::Launch(o)) = p(&["-studio", uri]) else {
            panic!()
        };
        assert_eq!((o.mode, o.launch_args.as_str()), (LaunchMode::Studio, uri));

        let place = r"C:\Users\me\Place.rbxl";
        let Ok(Command::Launch(o)) = p(&["-studio", place, "-quiet"]) else {
            panic!()
        };
        assert_eq!(o.launch_args, place);
        assert!(o.quiet);

        let Ok(Command::Launch(o)) = p(&["roblox-studio-auth:1+code:abc"]) else {
            panic!()
        };
        assert_eq!(o.mode, LaunchMode::Studio);

        let Ok(Command::Launch(o)) = p(&["roblox-player:1"]) else {
            panic!()
        };
        assert_eq!(o.mode, LaunchMode::Player);
    }

    #[test]
    fn errors() {
        assert!(p(&["-channel"]).is_err());
        assert!(p(&["-bogus"]).is_err());
        assert!(p(&["stray"]).is_err());
        assert_eq!(p(&["-help"]), Ok(Command::Help));
    }
}
