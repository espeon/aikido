use serde::Deserialize;
use tracing::info;

use crate::Result;

const MINECRAFT_AUTH_URL: &str =
    "https://api.minecraftservices.com/authentication/login_with_xbox";
const ENTITLEMENTS_URL: &str =
    "https://api.minecraftservices.com/entitlements/mcstore";
const PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

#[derive(Debug, Deserialize)]
struct MinecraftTokenResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct EntitlementsResponse {
    items: Vec<EntitlementItem>,
}

#[derive(Debug, Deserialize)]
struct EntitlementItem {
    name: String,
}

#[derive(Debug, Deserialize)]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
}

pub async fn authenticate_with_xbox(
    client: &reqwest::Client,
    uhs: &str,
    xsts_token: &str,
) -> Result<(String, u64)> {
    let identity_token = format!("XBL3.0 x={};{}", uhs, xsts_token);

    #[derive(serde::Serialize)]
    struct LoginRequest {
        #[serde(rename = "identityToken")]
        identity_token: String,
    }

    let resp: MinecraftTokenResponse = client
        .post(MINECRAFT_AUTH_URL)
        .json(&LoginRequest {
            identity_token: identity_token,
        })
        .send()
        .await?
        .json()
        .await?;

    info!(
        "minecraft authentication successful, expires in {}s",
        resp.expires_in
    );
    Ok((resp.access_token, resp.expires_in))
}

pub async fn check_entitlements(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<bool> {
    let resp: EntitlementsResponse = client
        .get(ENTITLEMENTS_URL)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?
        .json()
        .await?;

    let owns_game = resp
        .items
        .iter()
        .any(|item| item.name == "product_minecraft" || item.name == "game_minecraft");

    if !owns_game {
        return Err(crate::Error::Other(
            "this account does not own minecraft java edition".to_string(),
        ));
    }

    info!("entitlement check passed");
    Ok(true)
}

pub async fn get_profile(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<MinecraftProfile> {
    let profile: MinecraftProfile = client
        .get(PROFILE_URL)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?
        .json()
        .await?;

    info!("got profile: {} ({})", profile.name, profile.id);
    Ok(profile)
}
