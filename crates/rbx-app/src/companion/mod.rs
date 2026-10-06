//! What keeps running in the background while Roblox is open, for the features that need
//! it (Bloxstrap calls this its watcher):
//!
//! - activity tracking from Roblox's log: server location notices, Discord Rich Presence,
//!   and closing Roblox instead of returning to its desktop app;
//! - custom integrations (other programs started alongside Roblox);
//! - holding Roblox's single-instance mutex (multi-instance) and anti-AFK.
//!
//! It finds Roblox through the process list and exits once every Roblox window is closed.

mod discord;

use std::process::Child;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use rbx_core::Settings;
use rbx_core::activity::{Event, LogTail, ServerType, Tracker};
use rbx_deploy::reqwest;
use rbx_win::sync::NamedMutexGuard;

/// Only one companion runs at a time; later launches leave the work to it.
const LOCK: &str = "Rusticean-companion";
const TICK: Duration = Duration::from_secs(1);
/// How long to wait for Roblox to appear before giving up.
const STARTUP_GRACE: Duration = Duration::from_secs(120);
/// Roblox kicks after 20 idle minutes; nudge well before that.
const AFK_INTERVAL: Duration = Duration::from_secs(15 * 60);
const NOTICE_TIME: Duration = Duration::from_secs(10);

pub struct Companion {
    _lock: NamedMutexGuard,
    singleton: Option<NamedMutexGuard>,
    settings: Settings,
    started: SystemTime,
}

impl Companion {
    /// `None` if no background feature is on, or another companion already runs.
    pub fn prepare(settings: &Settings) -> Option<Companion> {
        let needed = settings.multi_instance
            || settings.anti_afk
            || settings.activity_tracking
            || !settings.custom_integrations.is_empty();
        if !needed {
            return None;
        }
        let lock = match rbx_win::sync::try_acquire(LOCK) {
            Ok(Some(lock)) => lock,
            Ok(None) => {
                tracing::info!("background helper already running");
                return None;
            }
            Err(e) => {
                tracing::warn!(error = %e, "could not start background helper");
                return None;
            }
        };
        let singleton = settings
            .multi_instance
            .then(|| match rbx_win::sync::hold_roblox_singleton() {
                Ok(guard) => Some(guard),
                Err(e) => {
                    tracing::warn!(error = %e, "could not hold Roblox's mutex; multi-instance is off");
                    None
                }
            })
            .flatten();
        Some(Companion {
            _lock: lock,
            singleton,
            settings: settings.clone(),
            // a little slack so the log Roblox creates as it starts still counts
            started: SystemTime::now() - Duration::from_secs(10),
        })
    }

    /// Block until no Roblox is left running.
    pub fn run(self) {
        tracing::info!(
            multi_instance = self.singleton.is_some(),
            anti_afk = self.settings.anti_afk,
            activity = self.settings.activity_tracking,
            "background helper started"
        );
        let mut integrations = start_integrations(&self.settings);

        match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime.block_on(self.watch()),
            Err(e) => tracing::warn!(error = %e, "could not start the background helper"),
        }

        for (auto_close, child) in &mut integrations {
            if *auto_close {
                let _ = child.kill();
            }
        }
        tracing::info!("Roblox closed; background helper exiting");
    }

    async fn watch(&self) {
        let s = &self.settings;
        let client = rbx_core::bootstrap::http_client().ok();
        let presence = discord::Presence::new(s);
        // bumped on every game change so slow lookups for an old game are dropped
        let generation = Arc::new(AtomicU64::new(0));
        let logs_dir = dirs::data_local_dir().map(|d| d.join("Roblox").join("logs"));

        let started = Instant::now();
        let mut seen = false;
        let mut last_nudge = Instant::now();
        let mut tail: Option<LogTail> = None;
        let mut tracker = Tracker::default();

        loop {
            tokio::time::sleep(TICK).await;
            let pids = rbx_win::process::roblox_pids();
            if pids.is_empty() {
                if seen || started.elapsed() > STARTUP_GRACE {
                    break;
                }
                continue;
            }
            seen = true;

            if s.activity_tracking {
                if tail.is_none()
                    && let Some(dir) = &logs_dir
                    && let Some((path, t)) = LogTail::open_newest(dir, self.started)
                {
                    tracing::info!(log = %path.display(), "following Roblox's log");
                    tail = Some(t);
                }
                let lines = match tail.as_mut().map(LogTail::read_lines) {
                    Some(Ok(lines)) => lines,
                    Some(Err(e)) => {
                        tracing::warn!(error = %e, "could not read Roblox's log");
                        Vec::new()
                    }
                    None => Vec::new(),
                };
                for line in lines {
                    let Some(event) = tracker.feed(&line) else {
                        continue;
                    };
                    tracing::info!(?event, "activity");
                    let gen_now = generation.fetch_add(1, Ordering::SeqCst) + 1;
                    match event {
                        Event::Joined(activity) => {
                            if let Some(client) = client.clone() {
                                if s.server_location {
                                    tokio::spawn(notify_location(
                                        client.clone(),
                                        activity.address.clone(),
                                        activity.server_type,
                                    ));
                                }
                                presence.show(client, activity, generation.clone(), gen_now);
                            }
                        }
                        Event::Left => presence.clear(),
                        Event::BackToApp if s.close_on_leave => {
                            // with several clients open we can't tell which one left
                            if pids.len() == 1 {
                                tracing::info!("left the game; closing Roblox");
                                rbx_win::process::close_windows(&pids);
                            }
                        }
                        Event::BackToApp => {}
                    }
                }
            }

            let idle = rbx_win::afk::user_idle_time();
            if rbx_win::afk::is_foreground(&pids) && idle < TICK * 5 {
                // they're playing, so Roblox sees the input itself
                last_nudge = Instant::now();
            } else if s.anti_afk
                && last_nudge.elapsed() >= AFK_INTERVAL
                && idle >= rbx_win::afk::USER_IDLE
            {
                let n = rbx_win::afk::nudge(&pids);
                tracing::info!(windows = n, "anti-AFK nudge");
                last_nudge = Instant::now();
            }
        }
        presence.clear();
    }
}

async fn notify_location(client: reqwest::Client, address: String, server_type: ServerType) {
    let title = match server_type {
        ServerType::Public => "Connected to public server",
        ServerType::Private => "Connected to private server",
    };
    match rbx_core::roblox_api::server_location(&client, &address).await {
        Ok(location) => {
            rbx_win::notify::show(title, &format!("Located at {location}"), NOTICE_TIME)
        }
        Err(e) => {
            tracing::warn!(error = %e, "could not look up the server location");
            rbx_win::notify::show(title, "Couldn't find where this server is.", NOTICE_TIME);
        }
    }
}

/// Start the user's custom integrations. Returns each one with its auto-close choice.
fn start_integrations(settings: &Settings) -> Vec<(bool, Child)> {
    settings
        .custom_integrations
        .iter()
        .filter(|i| !i.path.trim().is_empty())
        .filter_map(|i| {
            let mut command = std::process::Command::new(i.path.trim());
            command.args(split_args(&i.args));
            match command.spawn() {
                Ok(child) => {
                    tracing::info!(name = %i.name, "started integration");
                    Some((i.auto_close, child))
                }
                Err(e) => {
                    tracing::warn!(name = %i.name, error = %e, "could not start integration");
                    None
                }
            }
        })
        .collect()
}

/// Split launch arguments on spaces, keeping "quoted parts" together.
fn split_args(args: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in args.chars() {
        match c {
            '"' => quoted = !quoted,
            ' ' if !quoted => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_quoted_args() {
        assert_eq!(
            split_args(r#"-a "C:\My Folder\x.txt"  -b"#),
            vec!["-a", r"C:\My Folder\x.txt", "-b"]
        );
        assert!(split_args("  ").is_empty());
    }
}
