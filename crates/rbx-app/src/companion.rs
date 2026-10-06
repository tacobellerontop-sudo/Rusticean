//! What keeps running in the background after Roblox starts, for the features that need
//! it: holding Roblox's single-instance mutex (multi-instance) and anti-AFK. It watches
//! the process list and exits once every Roblox window is closed.

use std::time::{Duration, Instant};

use rbx_core::Settings;
use rbx_win::sync::NamedMutexGuard;

/// Only one companion runs at a time; later launches leave the work to it.
const LOCK: &str = "Rusticean-companion";
const POLL: Duration = Duration::from_secs(5);
/// How long to wait for Roblox to appear before giving up.
const STARTUP_GRACE: Duration = Duration::from_secs(120);
/// Roblox kicks after 20 idle minutes; nudge well before that.
const AFK_INTERVAL: Duration = Duration::from_secs(15 * 60);

pub struct Companion {
    _lock: NamedMutexGuard,
    singleton: Option<NamedMutexGuard>,
    anti_afk: bool,
}

impl Companion {
    /// `None` if no background feature is on, or another companion already runs.
    pub fn prepare(settings: &Settings) -> Option<Companion> {
        if !settings.multi_instance && !settings.anti_afk {
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
            anti_afk: settings.anti_afk,
        })
    }

    /// Block until no Roblox is left running.
    pub fn run(self) {
        tracing::info!(
            multi_instance = self.singleton.is_some(),
            anti_afk = self.anti_afk,
            "background helper started"
        );
        let started = Instant::now();
        let mut seen = false;
        let mut last_nudge = Instant::now();

        loop {
            std::thread::sleep(POLL);
            let pids = rbx_win::process::roblox_pids();
            if pids.is_empty() {
                if seen || started.elapsed() > STARTUP_GRACE {
                    break;
                }
                continue;
            }
            seen = true;

            let idle = rbx_win::afk::user_idle_time();
            if rbx_win::afk::is_foreground(&pids) && idle < POLL {
                // they're playing, so Roblox sees the input itself
                last_nudge = Instant::now();
            } else if self.anti_afk
                && last_nudge.elapsed() >= AFK_INTERVAL
                && idle >= rbx_win::afk::USER_IDLE
            {
                let n = rbx_win::afk::nudge(&pids);
                tracing::info!(windows = n, "anti-AFK nudge");
                last_nudge = Instant::now();
            }
        }
        tracing::info!("Roblox closed; background helper exiting");
    }
}
