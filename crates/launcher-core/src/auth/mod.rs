use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::info;

use crate::Result;

pub mod minecraft;
pub mod msa;
pub mod store;
pub mod xbox;

// while access request is processing this is from SJMC launcher
// https://github.com/UNIkeEN/SJMCL
const MSA_CLIENT_ID: &str = "b2468fd2-7996-4f42-8857-5b65d834ef5c";

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MicrosoftAccount {
    pub id: String,
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub expires_at: f64,
    pub refresh_token: String,
    pub xuid: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct StoredAccounts {
    pub accounts: Vec<MicrosoftAccount>,
    pub default_id: Option<String>,
}

pub fn client_id() -> String {
    std::env::var("KIDOMC_CLIENT_ID").unwrap_or_else(|_| MSA_CLIENT_ID.to_string())
}

pub async fn full_login(client: &reqwest::Client) -> Result<MicrosoftAccount> {
    let cid = client_id();

    let device = msa::request_device_code(client, &cid).await?;

    eprintln!(
        "Go to {} and enter the code: {}",
        device.verification_uri, device.user_code
    );
    eprintln!("Waiting for you to sign in...");

    let token = msa::poll_for_token(client, &cid, &device.device_code, device.interval).await?;

    let refresh_token = token
        .refresh_token
        .clone()
        .ok_or_else(|| crate::Error::Other("no refresh token returned".to_string()))?;

    let xbox_tokens = xbox::full_authenticate(client, &token.access_token).await?;

    let (mc_token, expires_in) =
        minecraft::authenticate_with_xbox(client, &xbox_tokens.uhs, &xbox_tokens.xsts_token)
            .await?;

    minecraft::check_entitlements(client, &mc_token).await?;

    let profile = minecraft::get_profile(client, &mc_token).await?;

    let expires_at = (chrono::Utc::now().timestamp() + expires_in as i64) as f64;

    let account = MicrosoftAccount {
        id: profile.id.clone(),
        username: profile.name.clone(),
        uuid: undashed_uuid(&profile.id),
        access_token: mc_token,
        expires_at,
        refresh_token,
        xuid: xbox_tokens.uhs.clone(),
        is_default: true,
    };

    store::save_account(&account).await?;

    eprintln!("Logged in as {} ({})", account.username, account.uuid);

    Ok(account)
}

pub async fn get_login_account(client: &reqwest::Client) -> Result<Option<MicrosoftAccount>> {
    let account = store::get_default_account().await?;

    match account {
        Some(mut acc) => {
            let now = chrono::Utc::now().timestamp() as f64;
            if now < acc.expires_at - 300.0 {
                return Ok(Some(acc));
            }

            info!("access token expired, refreshing...");
            match msa::refresh_token(client, &client_id(), &acc.refresh_token).await {
                Ok(token) => {
                    let xbox_tokens = xbox::full_authenticate(client, &token.access_token).await?;

                    let (mc_token, expires_in) = minecraft::authenticate_with_xbox(
                        client,
                        &xbox_tokens.uhs,
                        &xbox_tokens.xsts_token,
                    )
                    .await?;

                    acc.access_token = mc_token;
                    acc.expires_at = (chrono::Utc::now().timestamp() + expires_in as i64) as f64;

                    if let Some(ref new_refresh) = token.refresh_token {
                        acc.refresh_token = new_refresh.clone();
                    }

                    store::save_account(&acc).await?;
                    info!("token refreshed for {}", acc.username);
                    Ok(Some(acc))
                }
                Err(e) => {
                    info!("token refresh failed: {}, must re-login", e);
                    Ok(None)
                }
            }
        }
        None => Ok(None),
    }
}

fn undashed_uuid(id: &str) -> String {
    if id.len() == 32 {
        format!(
            "{}-{}-{}-{}-{}",
            &id[..8],
            &id[8..12],
            &id[12..16],
            &id[16..20],
            &id[20..]
        )
    } else {
        id.to_string()
    }
}
