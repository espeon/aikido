use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

use crate::download::Downloader;
use crate::Result;

const ASSET_BASE_URL: &str = "https://resources.download.minecraft.net";

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    #[serde(default)]
    #[serde(rename = "virtual")]
    pub r#virtual: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

pub async fn fetch_asset_index(
    client: &reqwest::Client,
    index_info: &crate::manifest::AssetIndexInfo,
    cache_dir: &Path,
) -> Result<AssetIndex> {
    let cache_path = cache_dir.join(format!("{}.json", index_info.id));

    if let Ok(data) = tokio::fs::read_to_string(&cache_path).await {
        if let Ok(index) = serde_json::from_str::<AssetIndex>(&data) {
            info!("using cached asset index {}", index_info.id);
            return Ok(index);
        }
    }

    info!(
        "fetching asset index {} from {}",
        index_info.id, index_info.url
    );
    let response = client
        .get(&index_info.url)
        .header("User-Agent", "kidomc/0.1.0")
        .send()
        .await?;

    let body = response.text().await?;

    tokio::fs::create_dir_all(cache_dir).await.ok();
    tokio::fs::write(&cache_path, &body).await.ok();

    let index = serde_json::from_str(&body)?;
    Ok(index)
}

pub async fn resolve_assets(
    downloader: &Downloader,
    index: &AssetIndex,
    objects_dir: &Path,
) -> Result<()> {
    tokio::fs::create_dir_all(objects_dir).await?;

    info!("resolving {} assets", index.objects.len());

    for (_name, obj) in &index.objects {
        let subdir = &obj.hash[..2];
        let obj_path = objects_dir.join(subdir).join(&obj.hash);

        let url = format!("{}/{}/{}", ASSET_BASE_URL, subdir, obj.hash);

        downloader.download(&url, &obj_path, Some(&obj.hash)).await?;
    }

    info!("all assets resolved");
    Ok(())
}
