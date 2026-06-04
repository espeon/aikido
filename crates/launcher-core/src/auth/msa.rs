use serde::Deserialize;
use tracing::info;

use crate::Result;

const DEVICE_CODE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

#[derive(Debug, Deserialize)]
pub struct DeviceCodeResponse {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub token_type: String,
    pub scope: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
}

pub async fn request_device_code(
    client: &reqwest::Client,
    client_id: &str,
) -> Result<DeviceCodeResponse> {
    let resp = client
        .post(DEVICE_CODE_URL)
        .form(&[
            ("client_id", client_id),
            ("scope", "XboxLive.signin offline_access"),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let body = resp.text().await?;
        return Err(crate::Error::Other(format!(
            "device code request failed: {}",
            body
        )));
    }

    let device_code: DeviceCodeResponse = resp.json().await?;
    Ok(device_code)
}

pub async fn poll_for_token(
    client: &reqwest::Client,
    client_id: &str,
    device_code: &str,
    interval_secs: u64,
) -> Result<TokenResponse> {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;

        let resp = client
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id),
                ("device_code", device_code),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await?;

            if body.contains("authorization_pending") {
                continue;
            }
            if body.contains("slow_down") {
                continue;
            }
            if body.contains("expired_token") {
                return Err(crate::Error::Other(
                    "device code expired, start over".to_string(),
                ));
            }

            return Err(crate::Error::Other(format!(
                "token request failed: {}",
                body
            )));
        }

        let token: TokenResponse = resp.json().await?;
        info!("got msa access token, expires in {}s", token.expires_in);
        return Ok(token);
    }
}

pub async fn refresh_token(
    client: &reqwest::Client,
    client_id: &str,
    refresh_token: &str,
) -> Result<TokenResponse> {
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let body = resp.text().await?;
        return Err(crate::Error::Other(format!(
            "token refresh failed: {}",
            body
        )));
    }

    let token: TokenResponse = resp.json().await?;
    info!("refreshed msa token, expires in {}s", token.expires_in);
    Ok(token)
}
