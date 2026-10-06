//! Command-line parsing. Bloxstrap-style single-dash flags, because that's what the
//! protocol handler and Bloxstrap users expect: `-player "roblox-player:1+..."`.

use rbx_core::LaunchOptions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Install/update the player and launch it.
    Player(Options),
    Help,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
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
            launch_args: o.launch_args,
            channel: o.channel,
            version_guid: o.version_guid,
            force: o.force,
            no_launch: o.no_launch,
        }
    }
}

fn is_roblox_uri(arg: &str) -> bool {
    let lower = arg.to_ascii_lowercase();
    lower.starts_with("roblox-player:") || lower.starts_with("roblox:")
}

pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Command, String> {
    let mut args = args.into_iter().peekable();
    let mut opts = Options::default();

    // value of a flag: the next argument, unless it's another flag
    fn value(args: &mut std::iter::Peekable<impl Iterator<Item = String>>) -> Option<String> {
        args.next_if(|a| !a.starts_with('-'))
    }

    while let Some(arg) = args.next() {
        if is_roblox_uri(&arg) {
            opts.launch_args = arg;
            continue;
        }

        let Some(flag) = arg.strip_prefix('-') else {
            return Err(format!("unexpected argument '{arg}'"));
        };

        match flag.to_ascii_lowercase().as_str() {
            "player" => opts.launch_args = value(&mut args).unwrap_or_default(),
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

    Ok(Command::Player(opts))
}

pub const USAGE: &str = "\
robloxbootstrapper: installs, updates and launches Roblox

USAGE:
    robloxbootstrapper [-player [<roblox-player: URI>]] [FLAGS]

With no arguments, installs or updates Roblox and opens the Roblox app.
Running it also registers it as the handler for roblox:// and roblox-player:// links.

FLAGS:
    -player [URI]      Launch the player, optionally with a roblox-player: URI
    -channel <name>    Use this deployment channel instead of the default
    -version <guid>    Install this exact version (e.g. version-1a2b3c4d5e6f7a8b)
    -force             Reinstall even if the version is already present
    -nolaunch          Install or update, but don't start Roblox
    -quiet             Don't show the progress window
    -help              Show this message
";

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Command, String> {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn no_args_launches_app() {
        assert_eq!(p(&[]), Ok(Command::Player(Options::default())));
    }

    #[test]
    fn protocol_handler_invocation() {
        let uri = "roblox-player:1+launchmode:play+gameinfo:abc+placelauncherurl:x";
        let Ok(Command::Player(o)) = p(&["-player", uri]) else {
            panic!()
        };
        assert_eq!(o.launch_args, uri);
    }

    #[test]
    fn bare_uri_and_flags() {
        let Ok(Command::Player(o)) = p(&[
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
        let Ok(Command::Player(o)) = p(&["-player", "-force"]) else {
            panic!()
        };
        assert_eq!(o.launch_args, "");
        assert!(o.force);
    }

    #[test]
    fn errors() {
        assert!(p(&["-channel"]).is_err());
        assert!(p(&["-bogus"]).is_err());
        assert!(p(&["stray"]).is_err());
        assert_eq!(p(&["-help"]), Ok(Command::Help));
    }
}
