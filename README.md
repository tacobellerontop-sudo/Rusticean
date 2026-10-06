# robloxbootstrapper

A Roblox bootstrapper for Windows written in Rust, inspired by [Bloxstrap](https://github.com/bloxstraplabs/bloxstrap) (MIT, © pizzaboxer).

It installs and updates the Roblox client itself, launches games from `roblox-player://` links, and adds extras like FastFlag editing, mods and Discord Rich Presence.

Status: planning. See [PLAN.md](PLAN.md) for the architecture and milestones.

## Building

Requires Rust (stable) on Windows.

```
cargo build --release
```

The exe lands in `target/release/robloxbootstrapper.exe`. CI also uploads a build for every pull request.

## Using it

Run `robloxbootstrapper.exe` to open its settings, then click **Launch Roblox**. That
downloads the latest Roblox into `%LOCALAPPDATA%\robloxbootstrapper\Versions`, registers
it as the handler for `roblox://` and `roblox-player://` links, and opens Roblox. After
that, clicking **Play** on roblox.com goes through it.

Settings (saved to `Settings.json`): progress window on/off, release channel, reinstall,
frame-rate cap, graphics API, and custom FastFlags. FastFlags are written to the version's
`ClientSettings/ClientAppSettings.json` on every launch.

A small window shows progress while Roblox downloads and starts, and explains any error.

Flags: `-settings`, `-player [uri]`, `-channel <name>`, `-version <guid>`, `-force`, `-nolaunch`, `-quiet` (no window), `-help`.
Logs are in `%LOCALAPPDATA%\robloxbootstrapper\Logs`.

To go back to the stock launcher, reinstall Roblox from roblox.com.

## Working on the UI

Debug builds can play a fake install so the window can be tweaked without downloading Roblox:

```
RBXB_UI_PREVIEW=1 cargo run        # full install animation
RBXB_UI_PREVIEW=error cargo run    # error screen
```
