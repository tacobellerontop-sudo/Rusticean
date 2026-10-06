// release builds are GUI-subsystem so clicking Play doesn't flash a console window
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod ui;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use anyhow::Context;
use rbx_core::{LaunchOptions, Paths, Settings, Status, StatusFn};
use rbx_deploy::install::Stage;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

const TITLE: &str = "robloxbootstrapper";

fn main() {
    let command = match cli::parse(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            rbx_win::dialog::error(TITLE, &format!("{e}\n\n{}", cli::USAGE));
            std::process::exit(2);
        }
    };

    if command == cli::Command::Help {
        println!("{}", cli::USAGE);
        return;
    }

    let paths = match Paths::default_base() {
        Some(base) => Paths::new(base),
        None => fail(anyhow::anyhow!("could not find %LOCALAPPDATA%")),
    };
    if let Err(e) = paths.ensure_dirs() {
        fail(anyhow::Error::new(e).context("creating data folders"));
    }

    // held for the whole run so the error below still reaches the log file
    let log_guard = init_logging(&paths);
    std::panic::set_hook(Box::new(|info| tracing::error!("panic: {info}")));

    let result = std::panic::catch_unwind(|| run(&paths, command))
        .unwrap_or_else(|_| Err(anyhow::anyhow!("the bootstrapper crashed (see the log)")));

    match result {
        Ok(true) => {}
        // already shown in the window
        Ok(false) => {
            drop(log_guard);
            std::process::exit(1);
        }
        Err(e) => {
            tracing::error!("{e:#}");
            drop(log_guard);
            fail(e);
        }
    }
}

fn fail(e: anyhow::Error) -> ! {
    rbx_win::dialog::error(TITLE, &format!("Roblox could not be started.\n\n{e:#}"));
    std::process::exit(1);
}

/// Returns `Ok(false)` for a failure the window has already shown.
fn run(paths: &Paths, command: cli::Command) -> anyhow::Result<bool> {
    tracing::info!(version = env!("CARGO_PKG_VERSION"), ?command, "starting");

    // re-point roblox:// links at this exe on every run, in case the stock launcher took them back
    match std::env::current_exe() {
        Ok(exe) => {
            if let Err(e) = rbx_win::registry::register_player_protocol(&exe) {
                tracing::warn!(error = %e, "could not register protocol handler");
            }
        }
        Err(e) => tracing::warn!(error = %e, "could not find our own path"),
    }

    let opts = match command {
        cli::Command::Player(opts) => opts,
        cli::Command::Settings => {
            if !ui::settings::run(paths.clone())? {
                return Ok(true);
            }
            cli::Options::default()
        }
        cli::Command::Help => return Ok(true),
    };
    launch(paths, opts)
}

fn launch(paths: &Paths, opts: cli::Options) -> anyhow::Result<bool> {
    let show_window = !opts.quiet && Settings::load(&paths.settings_file).show_progress_window;
    let opts: LaunchOptions = opts.into();

    if show_window {
        match ui::bootstrapper::run(paths.clone(), opts.clone()) {
            Ok(success) => return Ok(success),
            Err(e) => tracing::warn!("{e:#}; continuing without a window"),
        }
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("starting async runtime")?;
    runtime
        .block_on(rbx_core::run(
            paths,
            &opts,
            status_logger(),
            Arc::new(AtomicBool::new(false)),
        ))
        .context("bootstrapping Roblox")?;
    Ok(true)
}

/// Progress for `-quiet` runs: status changes, and downloads in 10% steps.
fn status_logger() -> StatusFn {
    let last_decile = AtomicU64::new(u64::MAX);
    Arc::new(move |status: Status| match status {
        Status::Installing { progress: p, .. } => match p.stage {
            Stage::Downloading { .. } if p.total > 0 => {
                let decile = p.downloaded * 10 / p.total;
                if last_decile.swap(decile, Ordering::Relaxed) != decile {
                    tracing::info!("downloading: {}%", decile * 10);
                }
            }
            Stage::Extracting { done, total } => {
                tracing::info!("extracting: {done}/{total} packages")
            }
            Stage::Done => tracing::info!("install finished"),
            _ => {}
        },
        other => tracing::info!(status = ?other, "status"),
    })
}

fn init_logging(paths: &Paths) -> tracing_appender::non_blocking::WorkerGuard {
    let file_name = format!(
        "{TITLE}_{}.log",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default()
    );
    let (writer, guard) =
        tracing_appender::non_blocking(tracing_appender::rolling::never(&paths.logs, file_name));

    let filter = EnvFilter::try_from_env("RBXB_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(writer),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();
    guard
}
