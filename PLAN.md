# Rusticean: plan for a Rust reimplementation of Bloxstrap

Reference: [bloxstraplabs/bloxstrap](https://github.com/bloxstraplabs/bloxstrap) (MIT, © pizzaboxer).
Bloxstrap is written in **C# / .NET (WPF)**, ~9k lines of non-UI logic plus a large XAML UI.
This plan maps its pieces to Rust and orders the work so we have a working launcher early.

Target platform: **Windows 10/11 x64** (Roblox's own client is Windows-only; Linux/macOS are out of scope).

---

## 1. How Bloxstrap actually works (the parts we must reproduce)

1. **It registers itself as the handler** for `roblox://`, `roblox-player://` (and for Studio: `roblox-studio://`,
   `roblox-studio-auth://`, `.rbxl`/`.rbxlx`) under `HKCU\Software\Classes\<scheme>\shell\open\command`
   with the command `"<exe>" -player "%1"`. So when you click Play on roblox.com, the browser starts *our* exe.
   (`Utility/WindowsRegistry.cs`)
2. **It parses its command line** (`LaunchSettings.cs`): flags like `-player <uri>`, `-studio <uri>`, `-menu`,
   `-watcher <b64>`, `-quiet`, `-uninstall`, `-channel <name>`, `-version <guid>`, `-force`, `-nolaunch`.
3. **It finds a reachable CDN mirror** (`RobloxInterfaces/Deployment.cs`): races `GET <mirror>/versionStudio`
   against `setup.rbxcdn.com` (priority 0), `setup-aws`, `setup-ak`, `roblox-setup.cachefly.net` (priority 2),
   `s3.amazonaws.com/setup.roblox.com` (priority 4), each delayed `priority` seconds; the body must equal
   `version-012732894899482c`. First success wins.
4. **It resolves the channel** (`Bootstrapper.cs` `GetCurrentChannelFromArgs`): `-channel` flag →
   `channel:xxx` inside the launch URI → `HKCU\SOFTWARE\ROBLOX Corporation\Environments\RobloxPlayer\Channel` value
   `www.roblox.com` → default `production`. Bad channels (401/403/404) fall back to production.
5. **It asks Roblox for the current version**:
   `GET https://clientsettingscdn.roblox.com/v2/client-version/WindowsPlayer[/channel/<ch>]`
   (fallback host `clientsettings.roblox.com`) → JSON `{ version, clientVersionUpload, ... }`. `clientVersionUpload` is the
   version GUID, e.g. `version-abc123...`.
6. **If the installed GUID differs, it upgrades**:
   - downloads `<mirror>/channel/common/<guid>-rbxPkgManifest.txt` (format: `v0`, then 4 lines per package:
     name, MD5, packed size, unpacked size; stops at `RobloxPlayerLauncher.exe`);
   - for each package, reuses `Downloads/<md5>` if its MD5 matches, else copies from
     `%LOCALAPPDATA%\Roblox\Downloads\<md5>` if present, else downloads `<mirror>/channel/common/<guid>-<name>`
     (5 retries, falls back to `http://` on IO errors) and verifies MD5;
   - extracts each zip into `Versions/<guid>/<subdir>` using a fixed **package → directory map**
     (`AppData/CommonAppData.cs`, `RobloxPlayerData.cs`, `RobloxStudioData.cs`), e.g.
     `content-fonts.zip → content\fonts\`, `RobloxApp.zip → (root)`;
   - writes `AppSettings.xml` (`<ContentFolder>content</ContentFolder><BaseUrl>http://www.roblox.com</BaseUrl>`);
   - optionally installs the WebView2 runtime; then deletes old version folders not in use.
7. **It applies modifications** (`ApplyModifications`): copies everything in `Modifications/` over the version folder,
   keeps a manifest so deleted mods get restored from the original package, writes
   `Modifications/ClientSettings/ClientAppSettings.json` (FastFlags, `FastFlagManager.cs`), custom fonts, etc.
8. **It launches** `Versions/<guid>/RobloxPlayerBeta.exe <launch args>` with that folder as working dir, closes
   the process handle quickly (Hyperion/Byfron trips if a handle stays open > ~1 min), waits for a new
   `%LOCALAPPDATA%\Roblox\logs\*.log`, then starts any custom integrations and re-launches itself with
   `-watcher <base64 json {pid, logFile, autoclosePids}>`.
9. **The watcher** (`Watcher.cs`, `Integrations/ActivityWatcher.cs`) tails the Roblox log, recognises lines like
   `[FLog::Output] ! Joining game '<jobId>' place <placeId> at <ip>`, `[FLog::Network] Replicator created:`,
   `[FLog::Network] Time to disconnect replication data:` and `[BloxstrapRPC] <json>`, and drives
   **Discord Rich Presence** (`Integrations/DiscordRichPresence.cs`, app id `1005469189907173486` is Bloxstrap's own;
   we register our own), server-location notifications, and auto-closing integrations when Roblox exits.
10. **Installer/uninstaller** (`Installer.cs`): copies itself to `%LOCALAPPDATA%\Bloxstrap`, writes an
    `HKCU\...\CurrentVersion\Uninstall\<name>` entry, protocol keys, desktop/start-menu shortcuts; uninstall
    restores the stock Roblox handlers if Roblox is still installed.
11. **Self-update** (`App.xaml.cs`): checks `api.github.com/repos/<repo>/releases/latest`, downloads the new exe,
    relaunches it with `-upgrade`.

---

## 2. Module map: Bloxstrap → Rust

Single Cargo workspace. The core is UI-agnostic so we can test it headlessly and swap the GUI.

```
Rusticean/
├─ Cargo.toml                (workspace)
├─ crates/
│  ├─ rbx-deploy/            CDN mirrors, client-version API, package manifest, download+verify, extract
│  ├─ rbx-core/              paths, settings/state JSON, channel logic, mods, fastflags, launch, installer
│  ├─ rbx-win/               registry, shortcuts, mutexes/named events, process helpers (Windows-only)
│  ├─ rbx-watcher/           log tailing, activity parser, Discord RPC, BloxstrapRPC messages
│  └─ rbx-app/               the .exe: CLI parsing, GUI (bootstrapper dialog + settings), tray icon
```

| Bloxstrap (C#)                                   | Rust home                         | Notes |
|--------------------------------------------------|-----------------------------------|-------|
| `LaunchSettings.cs`, `Models/LaunchFlag.cs`      | `rbx-app::cli`                    | Hand-rolled parser; Bloxstrap flags are `-flag value` with single dash, and the first positional arg may be a raw `roblox-player:` URI. `clap` fights that style, so a small custom parser is simpler. |
| `LaunchHandler.cs`, `App.xaml.cs`                | `rbx-app::main`, `rbx-app::dispatch` | Top-level "what mode are we in" switch. |
| `Paths.cs`                                       | `rbx-core::paths`                 | `dirs` / `known-folders` for `%LOCALAPPDATA%`, Desktop, Start Menu. |
| `RobloxInterfaces/Deployment.cs`                 | `rbx-deploy::cdn`, `rbx-deploy::version` | Mirror race with `tokio::select!` / `FuturesUnordered`. |
| `Models/Manifest/*`                              | `rbx-deploy::manifest`            | Pure parser, unit-tested against a saved real manifest. |
| `AppData/*` (package→dir maps)                   | `rbx-deploy::packages`            | `static` tables via `phf` or plain `match`. |
| `Bootstrapper.cs` (Download/Extract/Upgrade)     | `rbx-deploy::install`             | Progress reported through a channel (`tokio::sync::watch`/`mpsc`) the UI subscribes to. |
| `Bootstrapper.cs` (Run/StartRoblox/channel)      | `rbx-core::bootstrap`             | Orchestrates: connectivity → version → upgrade → mods → launch → spawn watcher. |
| `Bootstrapper.cs` `ApplyModifications`           | `rbx-core::mods`                  | Mod manifest kept in state JSON. |
| `FastFlagManager.cs`                             | `rbx-core::fastflags`             | `serde_json::Map<String, Value>`, presets as consts. |
| `JsonManager.cs`, `Models/Persistable/*`         | `rbx-core::config`                | `serde` structs: `Settings.json`, `State.json`, `RobloxState`/`DistributionState`. Atomic write (temp + rename). |
| `Utility/WindowsRegistry.cs`                     | `rbx-win::registry`               | `winreg` crate. |
| `Utility/Shortcut.cs`                            | `rbx-win::shortcut`               | `windows` crate `IShellLinkW` (or `mslnk` for creation only). |
| `Utility/InterProcessLock.cs`, `AsyncMutex.cs`   | `rbx-win::sync`                   | Named mutexes/events via `windows` crate (`CreateMutexW`, `OpenMutexW`, `CreateEventW`). Also detect `ROBLOX_singletonMutex`. |
| `Utilities.cs` process helpers, `KillRobloxInstances` | `rbx-win::process`           | `sysinfo` to enumerate/kill; `std::process::Command` + `CREATE_NO_WINDOW`; `ShellExecuteExW` with `runas` for admin. |
| `Utility/MD5Hash.cs`                             | `rbx-deploy::hash`                | `md-5` crate, streaming. |
| `Installer.cs`                                   | `rbx-core::installer`             | Copy self, uninstall key, shortcuts, protocol registration, uninstall with stock-handler restore. |
| `Watcher.cs`, `Integrations/ActivityWatcher.cs`  | `rbx-watcher::activity`           | Tail log with polling reads (or `notify`), `regex` for the patterns above. |
| `Integrations/DiscordRichPresence.cs`            | `rbx-watcher::discord`            | `discord-rich-presence` crate. Needs our own Discord application id. |
| `Models/APIs/Roblox/*`, `Utility/Thumbnails.cs`  | `rbx-watcher::roblox_api`         | `games.roblox.com/v1/games`, `thumbnails.roblox.com` for RPC images. |
| `Logger.cs`                                      | `tracing` + `tracing-appender`    | File per run under `Logs/`. |
| `Locale.cs`, `Resources/Strings*.resx`           | later: `fluent` / `rust-i18n`     | English only until Phase 5. |
| `UI/*` (WPF, ~many dialogs)                      | `rbx-app::ui`                     | See GUI choice below. |
| `UI/NotifyIconWrapper.cs`                        | `rbx-app::tray`                   | `tray-icon` crate. |
| `App.xaml.cs` update check                       | `rbx-core::selfupdate`            | GitHub releases API + replace-and-relaunch. |

## 3. Crate choices

| Need            | Crate | Why |
|-----------------|-------|-----|
| Async runtime   | `tokio` (rt-multi-thread, fs, macros) | Mirror racing, concurrent download + extract. |
| HTTP            | `reqwest` (rustls-tls, stream, json) | Streaming downloads with progress; rustls avoids OpenSSL on Windows. |
| JSON / config   | `serde`, `serde_json` | Settings/State/FastFlags. |
| Zip             | `zip` | Roblox packages are standard zips; extract with `spawn_blocking`. |
| MD5             | `md-5` | Package signatures are MD5. |
| Registry        | `winreg` | Simple, mature. |
| Win32           | `windows` (official) | Mutexes, events, ShellExecuteEx, IShellLink, SetForegroundWindow, taskbar progress. |
| Process list    | `sysinfo` | Find/kill `RobloxPlayerBeta`, `RobloxCrashHandler`. |
| Paths           | `dirs` | Known folders. |
| Regex           | `regex` | Channel from URI, log parsing. |
| Discord RPC     | `discord-rich-presence` | Small IPC client. |
| Tray            | `tray-icon` + `muda` (menus) | Same family as tauri, works with any GUI. |
| Logging         | `tracing`, `tracing-subscriber`, `tracing-appender` | |
| Errors          | `thiserror` (libs), `anyhow` (app) | |
| Base64          | `base64` | `-watcher` payload. |
| GUI             | **`egui` via `eframe`** (recommended) | Pure Rust, single static exe, easy progress dialog and settings pages, no WebView2 dependency. Alternative: `slint` if we want a more polished, designer-friendly look later. Tauri is heavier and pulls in WebView2. |
| Windows build bits | `winres` / `embed-resource` | Icon + manifest (DPI awareness, `asInvoker`). |

Build with `#![windows_subsystem = "windows"]` so no console flashes on launch; add a `--console` dev feature for debugging.

## 4. Milestones

Each phase ends with something runnable.

### Phase 0: Skeleton (½ day)
- Workspace, crates above as empty libs, CI (`cargo fmt --check`, `clippy -D warnings`, `cargo test`) on `windows-latest` via GitHub Actions.
- `tracing` logging to `%LOCALAPPDATA%\Rusticean\Logs`.

### Phase 1: Minimal working launcher (CLI, no GUI) ← first real goal
Done when: `Rusticean.exe -player` installs the current Roblox and opens the app, and clicking Play on roblox.com joins a game through us.
1. `rbx-deploy::manifest` parser + unit tests.
2. Mirror race + `client-version` fetch (production channel only).
3. Download with MD5 verify, reuse cache, extract via package map, write `AppSettings.xml`.
4. Persist `State.json` with installed version GUID; skip upgrade when current.
5. Launch `RobloxPlayerBeta.exe` with the URI as args, working dir = version folder, drop the handle immediately.
6. `-register` dev flag: write `roblox` / `roblox-player` protocol keys pointing at our exe.
7. Single-instance guard with a named mutex so two clicks don't upgrade twice.

### Phase 2: Bootstrapper UI + installer
- egui progress window (status text, progress bar, cancel) fed by the progress channel; taskbar progress via `ITaskbarList3`.
- First-run installer: copy to `%LOCALAPPDATA%`, uninstall registry entry, shortcuts, protocol registration; `-uninstall` restores stock Roblox handlers.
- Error dialogs (connectivity failure, not enough disk space, Media Feature Pack missing `mfplat.dll`).
- Kill running Roblox before upgrading; clean old `Versions/` folders not in use.

### Phase 3: Channels, mods, FastFlags
- Channel resolution exactly as in §1.4, including registry write-back and bad-channel fallback; `-channel`, `-version`, `-force` flags.
- `Modifications/` overlay with manifest-based restore.
- FastFlag editor backend + `ClientAppSettings.json`; a few presets (FPS cap, rendering API, MSAA, texture quality).
- Settings window (egui) with Behaviour, Mods, FastFlags pages; `Settings.json`.

### Phase 4: Watcher + integrations
- `-watcher` mode: tail log, activity state machine (joining → joined → left), BloxstrapRPC message passthrough.
- Discord Rich Presence with game name/icon from Roblox APIs.
- Tray icon with "copy server location / invite link", custom integrations (launch + auto-close).

### Phase 5: Studio, polish, release
- Studio support (`WindowsStudio64`, Studio package map, `roblox-studio(-auth)` protocols, `.rbxl` association).
- Background updater, self-update from our GitHub releases, localisation, themes/custom bootstrapper styles.
- Signed release builds via GitHub Actions.

## 5. Risks and notes
- **Roblox changes things.** Package lists and log line formats shift; keep the package map and log patterns in one place each, and log unknown packages instead of failing.
- **Anti-cheat (Hyperion).** Never inject or touch the running process; only start it and let go of the handle, as Bloxstrap does.
- **Antivirus false positives** are common for unsigned launchers that download executables; plan for code signing before wide release.
- **Licensing.** Bloxstrap is MIT; porting logic is fine, keep attribution in our README. Don't reuse Bloxstrap's name, icons or Discord app id.
- **Testing.** Core crates are Windows-agnostic where possible (manifest parsing, channel logic, log parsing, FastFlags) and get unit tests with recorded fixtures; Windows-specific bits are tested in CI on `windows-latest`.
