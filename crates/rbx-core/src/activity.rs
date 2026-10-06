//! Following what the player is doing from Roblox's own log file, the way Bloxstrap's
//! `Integrations/ActivityWatcher.cs` does: which place and server they join, when they
//! leave, and when Roblox goes back to its desktop app.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Instant, SystemTime};

use regex::Regex;

const JOINING: &str = "[FLog::Output] ! Joining game";
const JOINING_UNIVERSE: &str = "[FLog::GameJoinLoadTime] Report game_join_loadtime:";
const JOINING_UDMUX: &str = "[FLog::Network] UDMUX Address = ";
const JOINED: &str = "[FLog::Network] Replicator created: ";
const DISCONNECTED: &str = "[FLog::Network] Time to disconnect replication data:";
const LEAVING_TO_APP: &str = "[FLog::SingleSurfaceApp] leaveUGCGameInternal";

static JOINING_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"! Joining game '([0-9a-f\-]{36})' place ([0-9]+) at ([0-9\.]+)").unwrap()
});
static UNIVERSE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"universeid:([0-9]+).*userid:([0-9]+)").unwrap());
static REFERRAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"referral_page:([^,]+)").unwrap());
static UDMUX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"UDMUX Address = ([0-9\.]+), Port = [0-9]+ \| RCC Server Address = ([0-9\.]+), Port = [0-9]+").unwrap()
});

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ServerType {
    #[default]
    Public,
    Private,
}

/// The game being joined or played.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Activity {
    pub place_id: u64,
    pub job_id: String,
    /// The server's public address (the UDMUX proxy when Roblox uses one).
    pub address: String,
    pub universe_id: u64,
    pub user_id: u64,
    pub server_type: ServerType,
    pub joined_at: Option<SystemTime>,
}

impl Activity {
    /// A link that opens Roblox in this exact server.
    pub fn join_link(&self) -> String {
        format!(
            "roblox://experiences/start?placeId={}&gameInstanceId={}",
            self.place_id, self.job_id
        )
    }

    pub fn game_page(&self) -> String {
        format!("https://www.roblox.com/games/{}", self.place_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Fully connected to a server.
    Joined(Activity),
    /// Left or lost the server.
    Left,
    /// Roblox left the game and went back to its desktop app.
    BackToApp,
}

/// Turns log lines into [`Event`]s.
#[derive(Debug, Default)]
pub struct Tracker {
    current: Option<Activity>,
    in_game: bool,
}

impl Tracker {
    pub fn current(&self) -> Option<&Activity> {
        self.current.as_ref().filter(|_| self.in_game)
    }

    pub fn feed(&mut self, line: &str) -> Option<Event> {
        // lines are "<timestamp>,<thread>,<flags> <message>"
        let message = line.split_once(' ')?.1;

        if message.starts_with(LEAVING_TO_APP) {
            if !self.in_game {
                self.current = None;
            }
            return Some(Event::BackToApp);
        }

        match (self.in_game, &mut self.current) {
            (false, None) if message.starts_with(JOINING) => {
                let caps = JOINING_RE.captures(message)?;
                self.current = Some(Activity {
                    job_id: caps[1].to_owned(),
                    place_id: caps[2].parse().ok()?,
                    address: caps[3].to_owned(),
                    ..Activity::default()
                });
                None
            }
            (false, Some(activity)) => {
                if message.starts_with(JOINING_UNIVERSE) {
                    let caps = UNIVERSE_RE.captures(message)?;
                    activity.universe_id = caps[1].parse().ok()?;
                    activity.user_id = caps[2].parse().ok()?;
                    if let Some(referral) = REFERRAL_RE.captures(message) {
                        let r = referral[1].to_ascii_lowercase();
                        if r.contains("requestprivategame")
                            || r.contains("gamedetailpagejshybridevent")
                        {
                            activity.server_type = ServerType::Private;
                        }
                    }
                } else if message.starts_with(JOINING_UDMUX) {
                    let caps = UDMUX_RE.captures(message)?;
                    if caps[2] == activity.address {
                        activity.address = caps[1].to_owned();
                    }
                } else if message.starts_with(JOINED) {
                    self.in_game = true;
                    activity.joined_at = Some(SystemTime::now());
                    return Some(Event::Joined(activity.clone()));
                }
                None
            }
            (true, Some(_)) if message.starts_with(DISCONNECTED) => {
                self.in_game = false;
                self.current = None;
                Some(Event::Left)
            }
            _ => None,
        }
    }
}

/// Reads the lines Roblox appends to its newest log file.
pub struct LogTail {
    file: File,
    pending: String,
}

impl LogTail {
    /// The newest Player log in `dir` written since `since`, if there is one yet.
    pub fn open_newest(dir: &Path, since: SystemTime) -> Option<(PathBuf, LogTail)> {
        let newest = std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("Player"))
            .filter_map(|e| {
                let modified = e.metadata().ok()?.modified().ok()?;
                (modified >= since).then_some((modified, e.path()))
            })
            .max_by_key(|(modified, _)| *modified)?
            .1;
        let file = File::open(&newest).ok()?;
        Some((
            newest,
            LogTail {
                file,
                pending: String::new(),
            },
        ))
    }

    /// Complete lines added since the last call.
    pub fn read_lines(&mut self) -> io::Result<Vec<String>> {
        let mut bytes = Vec::new();
        self.file.read_to_end(&mut bytes)?;
        self.pending.push_str(&String::from_utf8_lossy(&bytes));
        let Some(last_newline) = self.pending.rfind('\n') else {
            return Ok(Vec::new());
        };
        let rest = self.pending.split_off(last_newline + 1);
        let lines = std::mem::replace(&mut self.pending, rest)
            .lines()
            .map(|l| l.trim_end_matches('\r').to_owned())
            .collect();
        Ok(lines)
    }
}

/// Throttle for things that should happen at most every so often.
pub struct Every {
    period: std::time::Duration,
    last: Option<Instant>,
}

impl Every {
    pub fn new(period: std::time::Duration) -> Self {
        Every { period, last: None }
    }

    pub fn ready(&mut self) -> bool {
        if self.last.is_none_or(|t| t.elapsed() >= self.period) {
            self.last = Some(Instant::now());
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JOB: &str = "4b3a2c1d-0000-4000-8000-1234567890ab";

    fn feed(t: &mut Tracker, message: &str) -> Option<Event> {
        t.feed(&format!("2026-10-06T08:00:00.000Z,0.1,abc,6 {message}"))
    }

    #[test]
    fn tracks_a_join_and_leave() {
        let mut t = Tracker::default();
        assert_eq!(
            feed(
                &mut t,
                &format!("[FLog::Output] ! Joining game '{JOB}' place 1818 at 10.0.0.1")
            ),
            None
        );
        feed(
            &mut t,
            "[FLog::GameJoinLoadTime] Report game_join_loadtime: placeid:1818, universeid:13058, referral_page:Home, userid:42",
        );
        feed(
            &mut t,
            "[FLog::Network] UDMUX Address = 128.116.1.2, Port = 1 | RCC Server Address = 10.0.0.1, Port = 2",
        );
        let Some(Event::Joined(a)) = feed(
            &mut t,
            "[FLog::Network] Replicator created: serverId: 10.0.0.1|5",
        ) else {
            panic!("expected a join");
        };
        assert_eq!((a.place_id, a.universe_id, a.user_id), (1818, 13058, 42));
        assert_eq!(a.address, "128.116.1.2");
        assert_eq!(a.server_type, ServerType::Public);
        assert_eq!(t.current().map(|a| a.job_id.as_str()), Some(JOB));

        assert_eq!(
            feed(
                &mut t,
                "[FLog::Network] Time to disconnect replication data: 0.5"
            ),
            Some(Event::Left)
        );
        assert!(t.current().is_none());
        assert_eq!(
            feed(&mut t, "[FLog::SingleSurfaceApp] leaveUGCGameInternal"),
            Some(Event::BackToApp)
        );
    }

    #[test]
    fn private_server_from_referral() {
        let mut t = Tracker::default();
        feed(
            &mut t,
            &format!("[FLog::Output] ! Joining game '{JOB}' place 1 at 10.0.0.1"),
        );
        feed(
            &mut t,
            "[FLog::GameJoinLoadTime] Report game_join_loadtime: universeid:2, referral_page:RequestPrivateGame, userid:3",
        );
        let Some(Event::Joined(a)) = feed(&mut t, "[FLog::Network] Replicator created: x") else {
            panic!("expected a join");
        };
        assert_eq!(a.server_type, ServerType::Private);
    }

    #[test]
    fn tail_returns_only_complete_new_lines() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("0.1_Player_abc_last.log");
        std::fs::write(&log, "one\ntw").unwrap();
        let (_, mut tail) = LogTail::open_newest(dir.path(), SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(tail.read_lines().unwrap(), vec!["one"]);
        use std::io::Write;
        std::fs::OpenOptions::new()
            .append(true)
            .open(&log)
            .unwrap()
            .write_all(b"o\nthree\n")
            .unwrap();
        assert_eq!(tail.read_lines().unwrap(), vec!["two", "three"]);
        assert!(tail.read_lines().unwrap().is_empty());
    }
}
