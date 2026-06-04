use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::Result;

#[derive(Debug, Deserialize)]
pub struct CfManifest {
    pub minecraft: CfMinecraft,
    pub files: Vec<CfManifestFile>,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub overrides: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CfMinecraft {
    pub version: String,
    #[serde(default, rename = "modLoaders")]
    pub mod_loaders: Vec<CfModLoader>,
}

#[derive(Debug, Deserialize)]
pub struct CfModLoader {
    pub id: String,
    pub primary: bool,
}

#[derive(Debug, Deserialize)]
pub struct CfManifestFile {
    #[serde(rename = "projectID")]
    pub project_id: u32,
    #[serde(rename = "fileID")]
    pub file_id: u32,
    pub required: bool,
}

pub async fn read_cf_zip(path: &Path) -> Result<(CfManifest, Vec<u8>)> {
    let data = tokio::fs::read(path).await?;
    let cursor = std::io::Cursor::new(&data);
    let mut archive = zip::ZipArchive::new(cursor)?;
    let overrides_data = data.clone();

    let mut manifest: Option<CfManifest> = None;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        if name == "manifest.json" {
            let mut contents = String::new();
            std::io::Read::read_to_string(&mut entry, &mut contents)?;
            manifest = Some(serde_json::from_str(&contents)?);
        }
    }

    let manifest = manifest.ok_or_else(|| crate::Error::Other("manifest.json not found in zip".to_string()))?;
    Ok((manifest, overrides_data))
}
