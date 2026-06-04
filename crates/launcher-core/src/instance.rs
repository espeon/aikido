use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use tracing::info;
use ulid::Ulid;
use chrono::Utc;

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct InstanceSummary {
    pub id: String,
    pub name: String,
    pub minecraft_version: String,
    pub loader: LoaderInfo,
    pub icon_path: Option<String>,
    pub last_played: Option<f64>,
    pub play_time_seconds: u32,
    pub mod_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct InstanceDetail {
    pub summary: InstanceSummary,
    pub path: PathBuf,
    pub memory_mb: u32,
    pub java_path: Option<String>,
    pub extra_jvm_args: Vec<String>,
    pub mods: Vec<InstalledMod>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum LoaderInfo {
    Vanilla,
    Fabric { loader_version: String },
    Quilt { loader_version: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct InstalledMod {
    pub filename: String,
    pub source: ModSource,
    pub mod_id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub version: Option<String>,
    pub sha1: Option<String>,
    pub disabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum ModSource {
    Modrinth { project_id: String, version_id: String },
    CurseForge { project_id: String, file_id: String },
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NewInstanceSpec {
    pub name: String,
    pub minecraft_version: String,
    pub loader: LoaderInfo,
    pub memory_mb: u32,
}

pub fn instance_dir(paths: &crate::LauncherPaths, id: &str) -> PathBuf {
    paths.instances_dir().join(id)
}

pub fn instance_json(paths: &crate::LauncherPaths, id: &str) -> PathBuf {
    instance_dir(paths, id).join("instance.json")
}

pub async fn list_instances(paths: &crate::LauncherPaths) -> Result<Vec<InstanceSummary>> {
    let dir = paths.instances_dir();
    tokio::fs::create_dir_all(&dir).await?;

    let mut instances = Vec::new();
    let mut entries = tokio::fs::read_dir(&dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let json_path = entry.path().join("instance.json");
        if json_path.exists() {
            match tokio::fs::read_to_string(&json_path).await {
                Ok(data) => {
                    if let Ok(detail) = serde_json::from_str::<InstanceDetail>(&data) {
                        instances.push(detail.summary);
                    } else if let Ok(summary) = serde_json::from_str::<InstanceSummary>(&data) {
                        instances.push(summary);
                    }
                }
                Err(e) => {
                    tracing::warn!("failed to read {}: {}", json_path.display(), e);
                }
            }
        }
    }

    instances.sort_by(|a, b| b.last_played.partial_cmp(&a.last_played).unwrap_or(std::cmp::Ordering::Equal));

    Ok(instances)
}

pub async fn get_instance(
    paths: &crate::LauncherPaths,
    id: &str,
) -> Result<InstanceDetail> {
    let json_path = instance_json(paths, id);
    let data = tokio::fs::read_to_string(&json_path).await?;
    let detail: InstanceDetail = serde_json::from_str(&data)?;
    Ok(detail)
}

pub async fn create_instance(
    paths: &crate::LauncherPaths,
    spec: &NewInstanceSpec,
) -> Result<String> {
    let id = Ulid::new().to_string();
    let dir = instance_dir(paths, &id);
    let dot_minecraft = dir.join(".minecraft");

    tokio::fs::create_dir_all(&dot_minecraft).await?;
    tokio::fs::create_dir_all(dot_minecraft.join("mods")).await?;
    tokio::fs::create_dir_all(dot_minecraft.join("logs")).await?;

    let detail = InstanceDetail {
        summary: InstanceSummary {
            id: id.clone(),
            name: spec.name.clone(),
            minecraft_version: spec.minecraft_version.clone(),
            loader: spec.loader.clone(),
            icon_path: None,
            last_played: None,
            play_time_seconds: 0,
            mod_count: 0,
        },
        path: dot_minecraft,
        memory_mb: spec.memory_mb,
        java_path: None,
        extra_jvm_args: Vec::new(),
        mods: Vec::new(),
    };

    let json_path = instance_json(paths, &id);
    let json = serde_json::to_string_pretty(&detail)?;
    let tmp = json_path.with_extension("json.tmp");
    tokio::fs::write(&tmp, &json).await?;
    tokio::fs::rename(&tmp, &json_path).await?;

    info!("created instance {}: {}", id, spec.name);
    Ok(id)
}

pub async fn delete_instance(
    paths: &crate::LauncherPaths,
    id: &str,
) -> Result<()> {
    let dir = instance_dir(paths, id);
    if dir.exists() {
        tokio::fs::remove_dir_all(&dir).await?;
        info!("deleted instance {}", id);
    }
    Ok(())
}

pub async fn record_launch(
    paths: &crate::LauncherPaths,
    id: &str,
) -> Result<()> {
    let json_path = instance_json(paths, id);
    if !json_path.exists() {
        return Ok(());
    }

    let data = tokio::fs::read_to_string(&json_path).await?;
    let mut detail: InstanceDetail = serde_json::from_str(&data)?;
    detail.summary.last_played = Some(Utc::now().timestamp() as f64);

    let json = serde_json::to_string_pretty(&detail)?;
    let tmp = json_path.with_extension("json.tmp");
    tokio::fs::write(&tmp, &json).await?;
    tokio::fs::rename(&tmp, &json_path).await?;

    Ok(())
}
