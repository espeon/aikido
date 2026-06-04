use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::Result;

#[derive(Debug, Deserialize)]
pub struct MrPackIndex {
    #[serde(rename = "formatVersion")]
    pub format_version: u32,
    pub game: String,
    #[serde(rename = "versionId")]
    pub version_id: String,
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub files: Vec<MrPackFile>,
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct MrPackFile {
    pub path: String,
    pub hashes: MrPackHashes,
    pub downloads: Vec<String>,
    #[serde(rename = "fileSize")]
    pub file_size: u64,
    #[serde(default)]
    pub env: Option<MrPackEnv>,
}

#[derive(Debug, Deserialize)]
pub struct MrPackHashes {
    pub sha1: Option<String>,
    pub sha512: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MrPackEnv {
    pub client: String,
    pub server: String,
}

const ALLOWED_DOMAINS: &[&str] = &[
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
    "cdn.azuriom.net",
    "edge.forgecdn.net",
    "media.forgecdn.net",
    "mediafilez.forgecdn.net",
];

pub fn parse_index(json: &str) -> Result<MrPackIndex> {
    let index: MrPackIndex = serde_json::from_str(json)?;
    if index.game != "minecraft" {
        return Err(crate::Error::Other(format!(
            "unsupported game: {}",
            index.game
        )));
    }
    Ok(index)
}

pub async fn read_mrpack(path: &Path) -> Result<(MrPackIndex, Vec<u8>)> {
    let data = tokio::fs::read(path).await?;
    let cursor = std::io::Cursor::new(&data);
    let mut archive = zip::ZipArchive::new(cursor)?;

    let mut index: Option<MrPackIndex> = None;
    let overrides_data = data.clone();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();

        if name == "modrinth.index.json" {
            let mut contents = String::new();
            std::io::Read::read_to_string(&mut entry, &mut contents)?;
            index = Some(parse_index(&contents)?);
        }
    }

    let index = index.ok_or_else(|| crate::Error::Other("modrinth.index.json not found in mrpack".to_string()))?;

    Ok((index, overrides_data))
}

pub fn validate_download_url(url: &str) -> Result<()> {
    let parsed = url::Url::parse(url)
        .map_err(|_| crate::Error::Other(format!("invalid url: {}", url)))?;

    let host = parsed
        .host_str()
        .ok_or_else(|| crate::Error::Other(format!("no host in url: {}", url)))?;

    if ALLOWED_DOMAINS.iter().any(|d| host == *d || host.ends_with(&format!(".{}", d))) {
        Ok(())
    } else {
        Err(crate::Error::Other(format!(
            "download domain not allowed: {}. must be one of: {:?}",
            host, ALLOWED_DOMAINS
        )))
    }
}

pub async fn extract_overrides(data: &[u8], dest: &Path, prefix: &str) -> Result<()> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();

        if !name.starts_with(prefix) {
            continue;
        }

        let relative = name.strip_prefix(prefix).unwrap_or(&name);
        if relative.is_empty() {
            continue;
        }

        let out_path = dest.join(relative);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out_file = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out_file)?;
        }
    }

    Ok(())
}
