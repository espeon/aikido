use std::path::PathBuf;
use tokio::fs;
use tracing::info;

use super::{MicrosoftAccount, StoredAccounts};
use crate::Result;

fn accounts_path() -> PathBuf {
    super::super::default_paths()
        .base_dir
        .join("accounts.json")
}

pub async fn load_accounts() -> Result<StoredAccounts> {
    let path = accounts_path();
    if !path.exists() {
        return Ok(StoredAccounts {
            accounts: Vec::new(),
            default_id: None,
        });
    }

    let data = fs::read_to_string(&path).await?;
    let stored: StoredAccounts = serde_json::from_str(&data)?;
    info!("loaded {} accounts", stored.accounts.len());
    Ok(stored)
}

pub async fn save_accounts(accounts: &StoredAccounts) -> Result<()> {
    let path = accounts_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(accounts)?;
    fs::write(&tmp, &json).await?;
    fs::rename(&tmp, &path).await?;

    info!("saved {} accounts", accounts.accounts.len());
    Ok(())
}

pub async fn save_account(account: &MicrosoftAccount) -> Result<()> {
    let mut stored = load_accounts().await?;

    if let Some(existing) = stored
        .accounts
        .iter_mut()
        .find(|a| a.id == account.id)
    {
        *existing = account.clone();
    } else {
        stored.accounts.push(account.clone());
    }

    if account.is_default {
        stored.default_id = Some(account.id.clone());
    }

    save_accounts(&stored).await
}

pub async fn get_default_account() -> Result<Option<MicrosoftAccount>> {
    let stored = load_accounts().await?;
    let default_id = stored.default_id.as_ref();

    Ok(stored
        .accounts
        .iter()
        .find(|a| default_id.map_or(false, |id| a.id == *id))
        .or_else(|| stored.accounts.first())
        .cloned())
}

pub async fn get_account_by_id(id: &str) -> Result<Option<MicrosoftAccount>> {
    let stored = load_accounts().await?;
    Ok(stored.accounts.into_iter().find(|a| a.id == id))
}
