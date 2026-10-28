//! Public web APIs used for Discord Rich Presence and the server location notice. None
//! of these need a login; they're the same ones Bloxstrap calls.

use rbx_deploy::reqwest;
use serde_json::Value;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone, Default)]
pub struct Universe {
    pub name: String,
    pub creator: String,
    pub creator_verified: bool,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct User {
    pub name: String,
    pub display_name: String,
    pub headshot_url: Option<String>,
}

async fn json(client: &reqwest::Client, url: &str) -> Result<Value, Error> {
    Ok(client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

fn str_at(v: &Value, pointer: &str) -> String {
    v.pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

pub async fn universe(client: &reqwest::Client, id: u64) -> Result<Universe, Error> {
    let games = json(
        client,
        &format!("https://games.roblox.com/v1/games?universeIds={id}"),
    )
    .await?;
    let icons = json(
        client,
        &format!(
            "https://thumbnails.roblox.com/v1/games/icons?universeIds={id}\
             &returnPolicy=PlaceHolder&size=128x128&format=Png&isCircular=false"
        ),
    )
    .await
    .unwrap_or_default();
    Ok(Universe {
        name: str_at(&games, "/data/0/name"),
        creator: str_at(&games, "/data/0/creator/name"),
        creator_verified: games
            .pointer("/data/0/creator/hasVerifiedBadge")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        icon_url: Some(str_at(&icons, "/data/0/imageUrl")).filter(|u| !u.is_empty()),
    })
}

pub async fn user(client: &reqwest::Client, id: u64) -> Result<User, Error> {
    let user = json(client, &format!("https://users.roblox.com/v1/users/{id}")).await?;
    let headshot = json(
        client,
        &format!(
            "https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={id}\
             &size=180x180&format=Png&isCircular=false"
        ),
    )
    .await
    .unwrap_or_default();
    Ok(User {
        name: str_at(&user, "/name"),
        display_name: str_at(&user, "/displayName"),
        headshot_url: Some(str_at(&headshot, "/data/0/imageUrl")).filter(|u| !u.is_empty()),
    })
}

/// "City, Region, Country" for a server address, from ipinfo.io.
pub async fn server_location(client: &reqwest::Client, address: &str) -> Result<String, Error> {
    let info = json(client, &format!("https://ipinfo.io/{address}/json")).await?;
    let (city, region, country) = (
        str_at(&info, "/city"),
        str_at(&info, "/region"),
        str_at(&info, "/country"),
    );
    if city.is_empty() {
        return Err("ipinfo.io didn't return a city".into());
    }
    Ok(if city == region {
        format!("{region}, {country}")
    } else {
        format!("{city}, {region}, {country}")
    })
}
