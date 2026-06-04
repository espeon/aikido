use serde::Deserialize;
use tracing::info;

use crate::Result;

const FABRIC_META: &str = "https://meta.fabricmc.net/v2";

#[derive(Debug, Deserialize)]
pub struct LoaderVersion {
    pub separator: String,
    pub build: u32,
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

pub async fn list_loader_versions(
    client: &reqwest::Client,
) -> Result<Vec<LoaderVersion>> {
    let url = format!("{}/versions/loader", FABRIC_META);
    let versions: Vec<LoaderVersion> = client.get(&url).send().await?.json().await?;
    Ok(versions)
}

#[derive(Debug, Deserialize)]
struct LoaderListEntry {
    loader: LoaderVersion,
}

pub async fn get_loader_versions_for_mc(
    client: &reqwest::Client,
    mc_version: &str,
) -> Result<Vec<LoaderVersion>> {
    let url = format!("{}/versions/loader/{}", FABRIC_META, mc_version);
    let resp = client.get(&url).send().await?;
    let body = resp.text().await?;
    let entries: Vec<LoaderListEntry> = serde_json::from_str(&body)
        .map_err(|e| crate::Error::Other(format!("fabric meta parse error: {}", e)))?;
    Ok(entries.into_iter().map(|e| e.loader).collect())
}

pub async fn get_profile_json(
    client: &reqwest::Client,
    mc_version: &str,
    loader_version: &str,
) -> Result<serde_json::Value> {
    let url = format!(
        "{}/versions/loader/{}/{}/profile/json",
        FABRIC_META, mc_version, loader_version
    );
    info!("fetching fabric profile: {}", url);
    let profile: serde_json::Value = client.get(&url).send().await?.json().await?;
    Ok(profile)
}

fn maven_path_from_name(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() >= 3 {
        let group = parts[0].replace('.', "/");
        let artifact = parts[1];
        let version = parts[2];
        format!(
            "{}/{}/{}/{}-{}.jar",
            group, artifact, version, artifact, version
        )
    } else {
        format!("{}.jar", name)
    }
}

pub fn fabric_library_to_vanilla_format(
    lib: &serde_json::Value,
) -> Option<serde_json::Value> {
    let name = lib.get("name")?.as_str()?;
    let url = lib.get("url")?.as_str()?;
    let url_base = url.trim_end_matches('/');

    let maven_path = maven_path_from_name(name);
    let download_url = format!("{}/{}", url_base, maven_path);

    let artifact = serde_json::json!({
        "path": maven_path,
        "sha1": "",
        "size": 0,
        "url": download_url,
    });

    let natives = lib.get("natives").cloned().unwrap_or(serde_json::Value::Null);
    let rules = lib.get("rules").cloned().unwrap_or(serde_json::Value::Null);

    let mut vanilla = serde_json::json!({
        "name": name,
        "downloads": {
            "artifact": artifact,
        },
    });

    if !natives.is_null() {
        vanilla["natives"] = natives;
    }
    if !rules.is_null() {
        vanilla["rules"] = rules;
    }

    Some(vanilla)
}

pub fn merge_fabric_profile(
    mut vanilla_manifest: serde_json::Value,
    fabric_profile: &serde_json::Value,
) -> serde_json::Value {
    if let Some(main_class) = fabric_profile.get("mainClass").and_then(|v| v.as_str()) {
        vanilla_manifest["mainClass"] = serde_json::Value::String(main_class.to_string());
    }

    if let Some(libraries) = fabric_profile.get("libraries").and_then(|v| v.as_array()) {
        let vanilla_libs = vanilla_manifest["libraries"]
            .as_array_mut()
            .expect("vanilla manifest missing libraries");

        for lib in libraries {
            if let Some(converted) = fabric_library_to_vanilla_format(lib) {
                vanilla_libs.push(converted);
            }
        }
    }

    if let Some(args) = fabric_profile.get("arguments") {
        if let Some(jvm_args) = args.get("jvm").and_then(|v| v.as_array()) {
            if let Some(vanilla_args) = vanilla_manifest["arguments"]["jvm"].as_array_mut() {
                for arg in jvm_args {
                    vanilla_args.push(arg.clone());
                }
            }
        }
    }

    vanilla_manifest
}
