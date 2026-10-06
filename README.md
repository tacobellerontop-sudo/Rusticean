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

## Using it (Phase 1)

Run `robloxbootstrapper.exe` once. It downloads the latest Roblox into
`%LOCALAPPDATA%\robloxbootstrapper\Versions`, registers itself as the handler for
`roblox://` and `roblox-player://` links, and opens Roblox. After that, clicking **Play**
on roblox.com goes through it.

Flags: `-player [uri]`, `-channel <name>`, `-version <guid>`, `-force`, `-nolaunch`, `-help`.
Logs are in `%LOCALAPPDATA%\robloxbootstrapper\Logs`.

To go back to the stock launcher, reinstall Roblox from roblox.com.
