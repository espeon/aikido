use std::env::VarError;

use crate::Result;
use serde::Deserialize;

const CF_BASE: &str = "https://api.curseforge.com";

const CURSEFORGE_KEYS: [&str; 4] = [
    "$2a$10$bL4bIL5pUWqfcO7KQtnMReakwtfHbNKh6v1uTpKlzhwoueEJQnPnm",
    "$2a$10$jK7YyZHdUNTDlcME9Egd6.Zt5RananLQKn/tpIhmRDezd2.wHGU9G",
    "$2a$10$6utA1UNSmFPrE/Lh7b7ndeeGmiOkjKNY8kpFB0fsmE/d42ZAfFgCe",
    "$2a$10$wuAJuNZuted3NORVmpgUC.m8sI.pv1tOPKZyBgLFGjxFp/br0lZCC",
];

fn random_key(_st: VarError) -> String {
    return CURSEFORGE_KEYS[1].to_string();
}

fn api_key() -> String {
    std::env::var("CURSEFORGE_API_KEY").unwrap_or_else(random_key)
}

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub data: Vec<ModInfo>,
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize)]
pub struct Pagination {
    #[serde(rename = "totalCount")]
    pub total_count: u32,
}

#[derive(Debug, Deserialize)]
pub struct ModInfo {
    pub id: u32,
    pub name: String,
    pub summary: String,
    pub authors: Vec<Author>,
    #[serde(rename = "downloadCount")]
    pub download_count: f64,
    #[serde(rename = "dateCreated")]
    pub date_created: String,
    #[serde(rename = "dateModified")]
    pub date_modified: String,
    #[serde(rename = "dateReleased")]
    pub date_released: String,
    #[serde(default)]
    pub logo: Option<ModAsset>,
    pub slug: String,
    #[serde(rename = "allowModDistribution")]
    pub allow_mod_distribution: Option<bool>,
    pub categories: Vec<Category>,
}

#[derive(Debug, Deserialize)]
pub struct Author {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ModAsset {
    #[serde(rename = "thumbnailUrl")]
    pub thumbnail_url: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct Category {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct FilesResponse {
    pub data: Vec<ModFile>,
}

#[derive(Debug, Deserialize)]
pub struct ModFile {
    pub id: u32,
    #[serde(rename = "modId")]
    pub mod_id: u32,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "fileDate")]
    pub file_date: String,
    #[serde(rename = "fileLength")]
    pub file_length: u64,
    #[serde(rename = "downloadUrl")]
    pub download_url: Option<String>,
    #[serde(rename = "gameVersions")]
    pub game_versions: Vec<String>,
    pub hashes: Vec<FileHash>,
    #[serde(rename = "isAvailable")]
    pub is_available: bool,
    pub dependencies: Vec<FileDependency>,
}

#[derive(Debug, Deserialize)]
pub struct FileHash {
    pub value: String,
    #[serde(rename = "algo")]
    pub algorithm: u8,
}

#[derive(Debug, Deserialize)]
pub struct FileDependency {
    #[serde(rename = "modId")]
    pub mod_id: u32,
    #[serde(rename = "relationType")]
    pub relation_type: u8,
}

pub async fn search_mods(
    client: &reqwest::Client,
    query: &str,
    loader_type: u32,
    game_version: Option<&str>,
    index: u32,
) -> Result<SearchResponse> {
    let key = api_key();
    let url = format!("{}/v1/mods/search", CF_BASE);
    let mut params: Vec<(&str, String)> = vec![
        ("gameId", "432".into()),
        ("classId", "6".into()),
        ("searchFilter", query.into()),
        ("sortField", "2".into()),
        ("sortOrder", "desc".into()),
        ("index", index.to_string()),
        ("pageSize", "30".into()),
    ];
    if loader_type > 0 {
        params.push(("modLoaderType", loader_type.to_string()));
    }
    if let Some(v) = game_version {
        params.push(("gameVersion", v.into()));
    }

    let resp = client
        .get(&url)
        .header("x-api-key", &key)
        .query(&params)
        .send()
        .await?
        .json()
        .await?;
    Ok(resp)
}

pub async fn get_mod(client: &reqwest::Client, mod_id: u32) -> Result<ModInfo> {
    let key = api_key();
    let url = format!("{}/v1/mods/{}", CF_BASE, mod_id);
    let resp: serde_json::Value = client
        .get(&url)
        .header("x-api-key", &key)
        .send()
        .await?
        .json()
        .await?;
    let mod_info: ModInfo = serde_json::from_value(resp["data"].clone())
        .map_err(|e| crate::Error::Other(format!("cf mod parse: {}", e)))?;
    Ok(mod_info)
}

pub async fn get_files(client: &reqwest::Client, mod_id: u32) -> Result<Vec<ModFile>> {
    let key = api_key();
    let url = format!("{}/v1/mods/{}/files", CF_BASE, mod_id);
    let resp: FilesResponse = client
        .get(&url)
        .header("x-api-key", &key)
        .send()
        .await?
        .json()
        .await?;
    Ok(resp.data)
}

pub async fn get_download_url(
    client: &reqwest::Client,
    mod_id: u32,
    file_id: u32,
) -> Result<String> {
    let key = api_key();
    let url = format!(
        "{}/v1/mods/{}/files/{}/download-url",
        CF_BASE, mod_id, file_id
    );
    let resp: serde_json::Value = client
        .get(&url)
        .header("x-api-key", &key)
        .send()
        .await?
        .json()
        .await?;
    let dl = resp["data"]
        .as_str()
        .ok_or_else(|| crate::Error::Other("no download url returned".into()))?;
    Ok(dl.to_string())
}
