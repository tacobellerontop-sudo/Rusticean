//! Discord Rich Presence: the game you're in on your Discord profile, like Bloxstrap's
//! `Integrations/DiscordRichPresence.cs`. Talks to the Discord desktop app over its
//! local IPC pipe; does nothing if Discord isn't running.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use discord_rich_presence::activity::{Activity, Assets, Button, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use rbx_core::Settings;
use rbx_core::activity::{Activity as Game, ServerType};
use rbx_deploy::reqwest;

/// The Discord application whose name shows as "Playing ...". This is Bloxstrap's public
/// app (named "Roblox"), until Rusticean registers its own.
const DISCORD_APP_ID: &str = "1005469189907173486";

#[derive(Clone)]
pub struct Presence {
    enabled: bool,
    join_button: bool,
    show_account: bool,
    client: Arc<Mutex<Option<DiscordIpcClient>>>,
}

impl Presence {
    pub fn new(settings: &Settings) -> Self {
        Presence {
            enabled: settings.activity_tracking && settings.discord_presence,
            join_button: settings.discord_join_button,
            show_account: settings.discord_show_account,
            client: Arc::new(Mutex::new(None)),
        }
    }

    /// Look up the game's details, then show it, unless the game changed meanwhile.
    pub fn show(&self, http: reqwest::Client, game: Game, generation: Arc<AtomicU64>, mine: u64) {
        if !self.enabled {
            return;
        }
        let this = self.clone();
        tokio::spawn(async move {
            let universe = match rbx_core::roblox_api::universe(&http, game.universe_id).await {
                Ok(u) => u,
                Err(e) => {
                    tracing::warn!(error = %e, "could not load game details for Discord");
                    return;
                }
            };
            let user = if this.show_account {
                rbx_core::roblox_api::user(&http, game.user_id).await.ok()
            } else {
                None
            };
            if generation.load(Ordering::SeqCst) != mine {
                return;
            }

            let state = match game.server_type {
                ServerType::Private => "In a private server".to_owned(),
                ServerType::Public if universe.creator_verified => {
                    format!("by {} ☑️", universe.creator)
                }
                ServerType::Public => format!("by {}", universe.creator),
            };
            // Discord wants at least two characters
            let details = format!("{:\u{2800}<2}", universe.name);
            let (small_image, small_text) = match &user {
                Some(u) => (
                    u.headshot_url.clone().unwrap_or_else(|| "roblox".into()),
                    format!("Playing on {} (@{})", u.display_name, u.name),
                ),
                None => ("roblox".into(), "Roblox".into()),
            };
            let large_image = universe.icon_url.clone().unwrap_or_else(|| "roblox".into());
            let join_link = game.join_link();
            let game_page = game.game_page();

            let mut buttons = Vec::new();
            if this.join_button && game.server_type == ServerType::Public {
                buttons.push(Button::new("Join server", join_link.as_str()));
            }
            buttons.push(Button::new("See game page", game_page.as_str()));

            let mut activity = Activity::new()
                .details(details.as_str())
                .state(state.as_str())
                .assets(
                    Assets::new()
                        .large_image(large_image.as_str())
                        .large_text(universe.name.as_str())
                        .small_image(small_image.as_str())
                        .small_text(small_text.as_str()),
                )
                .buttons(buttons);
            if let Some(joined) = game.joined_at
                && let Ok(since) = joined.duration_since(std::time::UNIX_EPOCH)
            {
                activity = activity.timestamps(Timestamps::new().start(since.as_secs() as i64));
            }
            this.with_client(|c| c.set_activity(activity));
        });
    }

    pub fn clear(&self) {
        if self.enabled {
            self.with_client(|c| c.clear_activity());
        }
    }

    /// Run `f` on a connected client, connecting (or reconnecting) first if needed.
    fn with_client(
        &self,
        f: impl FnOnce(&mut DiscordIpcClient) -> Result<(), discord_rich_presence::error::Error>,
    ) {
        let mut guard = self.client.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            let mut client = DiscordIpcClient::new(DISCORD_APP_ID);
            if let Err(e) = client.connect() {
                tracing::info!(error = %e, "Discord isn't running");
                return;
            }
            *guard = Some(client);
        }
        if let Some(client) = guard.as_mut()
            && let Err(e) = f(client)
        {
            tracing::warn!(error = %e, "Discord update failed; will reconnect next time");
            *guard = None;
        }
    }
}
