use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use tokio::fs;
use tracing::info;

use crate::{Error, Result};

const MANIFEST_V2_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Clone, Deserialize)]
pub struct VersionManifestV2 {
    pub latest: LatestVersions,
    pub versions: Vec<VersionEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String,
    pub url: String,
    pub time: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    pub sha1: String,
    #[serde(rename = "complianceLevel")]
    pub compliance_level: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionManifest {
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(rename = "assetIndex")]
    pub asset_index: AssetIndexInfo,
    pub assets: String,
    #[serde(default)]
    pub compliance_level: Option<u8>,
    pub downloads: VersionDownloads,
    pub id: String,
    #[serde(rename = "javaVersion")]
    pub java_version: Option<JavaVersion>,
    pub libraries: Vec<serde_json::Value>,
    #[serde(rename = "logging", default)]
    pub logging: Option<LoggingConfig>,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    #[serde(rename = "minimumLauncherVersion")]
    pub minimum_launcher_version: u32,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    pub time: String,
    #[serde(rename = "type")]
    pub version_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Arguments {
    pub game: Vec<serde_json::Value>,
    pub jvm: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndexInfo {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    #[serde(rename = "totalSize")]
    pub total_size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionDownloads {
    pub client: DownloadInfo,
    #[serde(rename = "client_mappings")]
    pub client_mappings: Option<DownloadInfo>,
    pub server: Option<DownloadInfo>,
    #[serde(rename = "server_mappings")]
    pub server_mappings: Option<DownloadInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadInfo {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
pub struct JavaVersion {
    pub component: String,
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub client: LogConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogConfig {
    pub argument: String,
    pub file: LogFileInfo,
    #[serde(rename = "type")]
    pub log_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogFileInfo {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

pub async fn fetch_version_manifest_v2(
    client: &reqwest::Client,
    cache_dir: &PathBuf,
) -> Result<VersionManifestV2> {
    let cache_path = cache_dir.join("version_manifest_v2.json");

    if let Ok(data) = fs::read_to_string(&cache_path).await {
        if let Ok(manifest) = serde_json::from_str::<VersionManifestV2>(&data) {
            info!("using cached version manifest v2");
            return Ok(manifest);
        }
    }

    info!("fetching version manifest v2 from {}", MANIFEST_V2_URL);
    let response = client
        .get(MANIFEST_V2_URL)
        .header("User-Agent", "kidomc/0.1.0")
        .send()
        .await?;

    let body = response.text().await?;

    fs::create_dir_all(cache_dir.parent().unwrap_or(cache_dir))
        .await
        .ok();
    fs::write(&cache_path, &body).await.ok();

    let manifest = serde_json::from_str(&body)?;
    Ok(manifest)
}

pub async fn fetch_version_manifest(
    client: &reqwest::Client,
    version_entry: &VersionEntry,
    versions_cache_dir: &PathBuf,
) -> Result<VersionManifest> {
    let version_id = &version_entry.id;
    let cache_path = versions_cache_dir.join(format!("{}.json", version_id));

    if let Ok(data) = fs::read_to_string(&cache_path).await {
        if let Ok(manifest) = serde_json::from_str::<VersionManifest>(&data) {
            info!("using cached manifest for version {}", version_id);
            return Ok(manifest);
        }
    }

    info!(
        "fetching version manifest for {} from {}",
        version_id, version_entry.url
    );
    let response = client
        .get(&version_entry.url)
        .header("User-Agent", "kidomc/0.1.0")
        .send()
        .await?;

    let body = response.text().await?;

    fs::create_dir_all(versions_cache_dir).await.ok();
    fs::write(&cache_path, &body).await.ok();

    let manifest = serde_json::from_str(&body)?;
    Ok(manifest)
}

pub fn find_version<'a>(
    manifest: &'a VersionManifestV2,
    version: &str,
) -> Result<&'a VersionEntry> {
    manifest
        .versions
        .iter()
        .find(|v| v.id == version)
        .ok_or_else(|| Error::VersionNotFound(version.to_string()))
}
