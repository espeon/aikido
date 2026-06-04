use serde::{Deserialize, Serialize};
use tracing::info;

use crate::Result;

const XBL_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct XblAuthRequest {
    properties: XblProperties,
    relying_party: String,
    token_type: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct XblProperties {
    auth_method: String,
    site_name: String,
    rps_ticket: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XblAuthResponse {
    token: String,
    display_claims: DisplayClaims,
}

#[derive(Debug, Deserialize)]
struct DisplayClaims {
    xui: Vec<XuiClaim>,
}

#[derive(Debug, Deserialize)]
struct XuiClaim {
    uhs: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct XstsAuthRequest {
    properties: XstsProperties,
    relying_party: String,
    token_type: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct XstsProperties {
    sandbox_id: String,
    user_tokens: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
struct XstsAuthResponse {
    token: String,
    display_claims: DisplayClaims,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
struct XstsError {
    xerr: i64,
    message: String,
    redirect: Option<String>,
}

pub struct XboxTokens {
    pub xbl_token: String,
    pub xsts_token: String,
    pub uhs: String,
}

pub async fn xbl_authenticate(
    client: &reqwest::Client,
    msa_access_token: &str,
) -> Result<(String, String)> {
    let req = XblAuthRequest {
        properties: XblProperties {
            auth_method: "RPS".to_string(),
            site_name: "user.auth.xboxlive.com".to_string(),
            rps_ticket: format!("d={}", msa_access_token),
        },
        relying_party: "http://auth.xboxlive.com".to_string(),
        token_type: "JWT".to_string(),
    };

    let resp: XblAuthResponse = client
        .post(XBL_AUTH_URL)
        .json(&req)
        .header("Accept", "application/json")
        .send()
        .await?
        .json()
        .await?;

    let uhs = resp
        .display_claims
        .xui
        .first()
        .map(|x| x.uhs.clone())
        .unwrap_or_default();

    info!("xbl authentication successful");
    Ok((resp.token, uhs))
}

pub async fn xsts_authenticate(
    client: &reqwest::Client,
    xbl_token: &str,
) -> Result<String> {
    let req = XstsAuthRequest {
        properties: XstsProperties {
            sandbox_id: "RETAIL".to_string(),
            user_tokens: vec![xbl_token.to_string()],
        },
        relying_party: "rp://api.minecraftservices.com/".to_string(),
        token_type: "JWT".to_string(),
    };

    let resp = client
        .post(XSTS_AUTH_URL)
        .json(&req)
        .header("Accept", "application/json")
        .send()
        .await?;

    if !resp.status().is_success() {
        let body = resp.text().await?;
        if let Ok(err) = serde_json::from_str::<XstsError>(&body) {
            return match err.xerr {
                2148916233 => Err(crate::Error::Other(
                    "no xbox account associated with this microsoft account"
                        .to_string(),
                )),
                2148916238 => Err(crate::Error::Other(
                    "account is a child account and requires parental approval"
                        .to_string(),
                )),
                _ => Err(crate::Error::Other(format!(
                    "xsts auth failed: {}",
                    err.message
                ))),
            };
        }
        return Err(crate::Error::Other(format!(
            "xsts auth failed: {}",
            body
        )));
    }

    let xsts: XstsAuthResponse = resp.json().await?;
    info!("xsts authentication successful");
    Ok(xsts.token)
}

pub async fn full_authenticate(
    client: &reqwest::Client,
    msa_access_token: &str,
) -> Result<XboxTokens> {
    let (xbl_token, uhs) = xbl_authenticate(client, msa_access_token).await?;
    let xsts_token = xsts_authenticate(client, &xbl_token).await?;

    Ok(XboxTokens {
        xbl_token,
        xsts_token,
        uhs,
    })
}
