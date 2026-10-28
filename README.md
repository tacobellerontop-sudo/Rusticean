# Rusticean

A Roblox bootstrapper for Windows written in Rust, inspired by [Bloxstrap](https://github.com/bloxstraplabs/bloxstrap) (MIT, © pizzaboxer).

It installs and updates the Roblox client itself, launches games from `roblox-player://` links, and adds extras like FastFlag editing, mods and Discord Rich Presence.

See [PLAN.md](PLAN.md) for the architecture and milestones. (It was called
`robloxbootstrapper` before; existing data in `%LOCALAPPDATA%\robloxbootstrapper` moves over
on first run.)

## Installing

Download `Rusticean-Setup.exe` from the latest CI run and run it. It installs for your user
only (no admin prompt) to `%LOCALAPPDATA%\Programs\Rusticean`, adds a Start menu entry
(and optionally a desktop shortcut), and registers `roblox://` links. Uninstall from
Windows Settings > Apps; it asks whether to delete downloaded Roblox versions and settings too.

## Building

Requires Rust (stable) on Windows.

```
cargo build --release
```

The exe lands in `target/release/Rusticean.exe`. To build the installer too, install
[Inno Setup 6](https://jrsoftware.org/isinfo.php) and run
`iscc /DAppVersion=0.1.0 installer\Rusticean.iss`; it writes `target\installer\Rusticean-Setup.exe`.
CI builds both for every pull request.

## Using it

Run `Rusticean.exe` to open its settings, then click **Launch Roblox**. That
downloads the latest Roblox into `%LOCALAPPDATA%\Rusticean\Versions`, registers
it as the handler for `roblox://` and `roblox-player://` links, and opens Roblox. After
that, clicking **Play** on roblox.com goes through it.

Settings (saved to `Settings.json`):

- **Launch:** progress window on/off, ask before closing a running Roblox, process priority,
  automatic updates (off pins the installed version), release channel.
- **Integrations:** activity tracking (reads Roblox's log to see which game you're in), a
  notification with the server's location, closing Roblox when you leave a game instead of
  returning to its home screen, Discord Rich Presence (game, join button, account), and
  custom programs started alongside Roblox.
- **Mods:** files in `%LOCALAPPDATA%\Rusticean\Modifications` are copied over the
  Roblox install on every launch (e.g. `content/sounds/ouch.ogg`). Removing a file restores
  the original. Built-in presets: 2006/2013 cursors, old avatar editor background, old
  character sounds, emoji style, custom font. Compatibility: disable fullscreen
  optimizations, override high DPI scaling.
- **Engine:** FastFlags (frame-rate cap, graphics API, anti-aliasing, texture quality,
  preserve rendering quality with display scaling, remove grass, exclusive fullscreen,
  custom flags), written to the version's `ClientSettings/ClientAppSettings.json` on every
  launch unless "Let Rusticean manage FastFlags" is off. Also resets every setting.
- **Appearance:** dark, light or system theme; progress window style (Rusticean, compact,
  classic dialog), icon (including your own image) and title.
- **Storage:** delete old downloads and logs automatically or on demand, reinstall Roblox.
- **Risky** (off by default; may get an account warned or banned): multiple Roblox windows,
  anti-AFK, fullbright. Multi-instance and anti-AFK keep a small background helper running
  until every Roblox window is closed.

Some of these come from the Bloxstrap forks [Voidstrap](https://github.com/voldstrap/Voidstrap)
and [Fishstrap](https://github.com/fishstrap/fishstrap).

A small window shows progress while Roblox downloads and starts, and explains any error.

Flags: `-settings`, `-preview` (show the progress window without launching), `-player [uri]`, `-channel <name>`, `-version <guid>`, `-force`, `-nolaunch`, `-quiet` (no window), `-help`.
Logs are in `%LOCALAPPDATA%\Rusticean\Logs`.

To go back to the stock launcher, reinstall Roblox from roblox.com.

## Working on the UI

Debug builds can play a fake install so the window can be tweaked without downloading Roblox:

```
RUSTICEAN_UI_PREVIEW=1 cargo run        # full install animation
RUSTICEAN_UI_PREVIEW=error cargo run    # error screen
```
