//! The install-then-launch flow, ported from Bloxstrap's `Bootstrapper.Run`.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rbx_deploy::install::{InstallJob, Progress};
use rbx_deploy::{BinaryType, cdn, manifest, reqwest, version};

use crate::channel;
use crate::cleanup;
use crate::mods;
use crate::paths::{APP_NAME, Paths};
use crate::presets;
use crate::settings::{self, ProcessPriority, Settings};
use crate::state::State;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Deploy(#[from] rbx_deploy::Error),

    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
}

fn io_err(context: impl Into<String>) -> impl FnOnce(std::io::Error) -> Error {
    move |source| Error::Io {
        context: context.into(),
        source,
    }
}

/// What the bootstrapper is doing, for a UI to display.
#[derive(Debug, Clone)]
pub enum Status {
    Connecting,
    CheckingForUpdates,
    /// Downloading or extracting a new version.
    Installing {
        upgrading: bool,
        progress: Progress,
    },
    Starting,
    /// Roblox is running (or, with `no_launch`, installed); the UI can close.
    Finished,
}

pub type StatusFn = Arc<dyn Fn(Status) + Send + Sync>;

/// What the user (or the browser, via the protocol handler) asked for.
#[derive(Debug, Clone, Default)]
pub struct LaunchOptions {
    /// Passed through to the Roblox client, usually a `roblox-player:` URI. Empty opens the app.
    pub launch_args: String,
    /// `-channel`: overrides the URI and registry.
    pub channel: Option<String>,
    /// `-version`: install this exact version GUID instead of the latest.
    pub version_guid: Option<String>,
    /// `-force`: reinstall even if the version is already present.
    pub force: bool,
    /// `-nolaunch`: install or update, then exit without starting Roblox.
    pub no_launch: bool,
}

const BINARY: BinaryType = BinaryType::WindowsPlayer;

pub fn http_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("Rusticean/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .build()
}

/// Make sure the latest Roblox Player is installed, then start it.
///
/// Setting `cancel` stops at the next safe point and returns `rbx_deploy::Error::Cancelled`.
pub async fn run(
    paths: &Paths,
    opts: &LaunchOptions,
    on_status: StatusFn,
    cancel: Arc<AtomicBool>,
) -> Result<(), Error> {
    let check_cancel = || {
        if cancel.load(Ordering::Relaxed) {
            Err(Error::Deploy(rbx_deploy::Error::Cancelled))
        } else {
            Ok(())
        }
    };

    on_status(Status::Connecting);
    paths
        .ensure_dirs()
        .map_err(io_err("creating data folders"))?;

    // one bootstrapper at a time, so two clicks never upgrade concurrently
    let mutex_name = format!("{APP_NAME}-bootstrapper-{}", BINARY.api_name());
    let _guard = tokio::task::spawn_blocking(move || rbx_win::sync::acquire(&mutex_name))
        .await
        .expect("mutex task panicked")
        .map_err(io_err("acquiring the bootstrapper lock"))?;

    // state may have changed while we waited for another instance
    let mut state = State::load(&paths.state_file);
    let settings = Settings::load(&paths.settings_file);
    let client = http_client().map_err(rbx_deploy::Error::from)?;

    // with auto-update off, launch what's installed without asking Roblox for anything
    let pinned = (!settings.auto_update && !opts.force && opts.version_guid.is_none())
        .then(|| state.player.version_guid.clone())
        .flatten()
        .filter(|guid| {
            paths
                .version_dir(guid)
                .join(BINARY.executable_name())
                .exists()
        });

    on_status(Status::CheckingForUpdates);

    let version_guid = match (&opts.version_guid, pinned) {
        (Some(guid), _) => {
            tracing::info!(%guid, "version set from arguments");
            guid.clone()
        }
        (None, Some(guid)) => {
            tracing::info!(%guid, "auto-update is off, launching the installed version");
            guid
        }
        (None, None) => latest_version_guid(&client, opts, &settings).await?,
    };
    check_cancel()?;

    let version_dir = paths.version_dir(&version_guid);
    let exe = version_dir.join(BINARY.executable_name());

    let installed = state.player.version_guid.as_deref() == Some(version_guid.as_str());
    if opts.force || !installed || !exe.exists() {
        tracing::info!(%version_guid, "installing");
        let mirror = cdn::find_mirror(&client).await?;
        check_cancel()?;

        let manifest_text = client
            .get(mirror.package_manifest_url(&version_guid))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(rbx_deploy::Error::from)?
            .text()
            .await
            .map_err(rbx_deploy::Error::from)?;

        InstallJob {
            client: client.clone(),
            mirror: mirror.clone(),
            binary: BINARY,
            version_guid: version_guid.clone(),
            packages: manifest::parse(&manifest_text)?,
            downloads_dir: paths.downloads.clone(),
            version_dir: version_dir.clone(),
            stock_downloads_dir: Paths::stock_downloads(),
            on_progress: {
                let on_status = on_status.clone();
                let upgrading = state.player.version_guid.is_some();
                Arc::new(move |progress| {
                    on_status(Status::Installing {
                        upgrading,
                        progress,
                    })
                })
            },
            cancel: cancel.clone(),
        }
        .run()
        .await?;

        state.player.version_guid = Some(version_guid.clone());
        state
            .save(&paths.state_file)
            .map_err(io_err("saving State.json"))?;

        cleanup_old_versions(paths, &version_guid);
    } else {
        tracing::info!(%version_guid, "already up to date");
    }

    // FastFlags are rewritten every launch so settings changes apply without a reinstall
    if settings.manage_fast_flags {
        settings::write_client_settings(&version_dir, &settings.effective_fast_flags())
            .map_err(io_err("writing ClientAppSettings.json"))?;
    }

    if let Err(e) = presets::sync(paths, &settings, &version_dir, &client).await {
        tracing::warn!(error = %e, "could not prepare mod presets");
    }

    // a broken mod shouldn't stop the game from starting
    match mods::apply(&[&paths.presets, &paths.modifications], &version_dir) {
        Ok(0) => {}
        Ok(n) => tracing::info!(files = n, "applied modifications"),
        Err(e) => tracing::warn!(error = %e, "could not apply modifications"),
    }
    if let Err(e) = mods::set_fullbright(&version_dir, settings.fullbright) {
        tracing::warn!(error = %e, "could not change fullbright");
    }
    if let Err(e) = rbx_win::registry::set_compat_flags(
        &exe,
        settings.disable_fullscreen_optimizations,
        settings.dpi_override,
    ) {
        tracing::warn!(error = %e, "could not set fullscreen optimizations");
    }

    if !opts.no_launch {
        check_cancel()?;
        on_status(Status::Starting);
        start_roblox(
            &exe,
            &version_dir,
            &opts.launch_args,
            settings.process_priority,
        )?;
    }
    on_status(Status::Finished);

    // housekeeping after Roblox is up, so it never delays the launch
    if let Some(max_age) = settings.cleanup.max_age() {
        let report = cleanup::run(paths, max_age);
        tracing::info!(
            files = report.files,
            bytes = report.bytes,
            "cleaned up old files"
        );
    }
    Ok(())
}

async fn latest_version_guid(
    client: &reqwest::Client,
    opts: &LaunchOptions,
    settings: &Settings,
) -> Result<String, Error> {
    let mut channel = channel::resolve(
        opts.channel.as_deref().or(settings.channel.as_deref()),
        &opts.launch_args,
        rbx_win::registry::read_channel(BINARY.registry_name()),
    );
    tracing::info!(%channel, "resolved channel");

    let info = match version::fetch(client, BINARY, &channel).await {
        Err(rbx_deploy::Error::InvalidChannel(bad)) => {
            tracing::warn!(%bad, "channel refused, falling back to production");
            channel = version::DEFAULT_CHANNEL.to_owned();
            version::fetch(client, BINARY, &channel).await?
        }
        other => other?,
    };

    // keep Roblox's own registry value in sync, as the stock launcher does
    let stored = if version::is_default_channel(&channel) {
        ""
    } else {
        &channel
    };
    if let Err(e) = rbx_win::registry::write_channel(BINARY.registry_name(), stored) {
        tracing::warn!(error = %e, "could not write channel to registry");
    }

    tracing::info!(version = %info.version, guid = %info.version_guid, "latest version");
    Ok(info.version_guid)
}

/// Delete version folders other than `keep`, skipping any whose executable is in use
/// (Windows refuses to delete a running exe, which is how we detect it).
fn cleanup_old_versions(paths: &Paths, keep: &str) {
    let Ok(entries) = std::fs::read_dir(&paths.versions) else {
        return;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() || entry.file_name() == keep {
            continue;
        }
        let exe = dir.join(BINARY.executable_name());
        match std::fs::remove_file(&exe) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                tracing::info!(dir = %dir.display(), error = %e, "version still in use, keeping it");
                continue;
            }
        }
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            tracing::warn!(dir = %dir.display(), error = %e, "could not remove old version");
        }
    }
}

fn start_roblox(
    exe: &Path,
    working_dir: &Path,
    launch_args: &str,
    priority: ProcessPriority,
) -> Result<(), Error> {
    let mut command = std::process::Command::new(exe);
    command.current_dir(working_dir);
    if !launch_args.is_empty() {
        command.arg(launch_args);
    }

    // set at creation, so we never need a handle to the running game to change it
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(priority.creation_flag());
    }
    #[cfg(not(windows))]
    let _ = priority;

    // drop the child (and its process handle) immediately: Roblox's anti-cheat
    // trips if a launcher keeps a handle open to it
    let pid = command
        .spawn()
        .map_err(io_err(format!("starting {}", exe.display())))?
        .id();
    tracing::info!(pid, "started Roblox");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_keeps_current_version() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        paths.ensure_dirs().unwrap();
        for guid in ["version-old", "version-new"] {
            let v = paths.version_dir(guid);
            std::fs::create_dir_all(&v).unwrap();
            std::fs::write(v.join(BINARY.executable_name()), b"").unwrap();
        }
        cleanup_old_versions(&paths, "version-new");
        assert!(!paths.version_dir("version-old").exists());
        assert!(paths.version_dir("version-new").exists());
    }
}
