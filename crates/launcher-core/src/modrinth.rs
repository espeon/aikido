use serde::{Deserialize, Serialize};
use specta::Type;

use crate::Result;

const MODRINTH_API: &str = "https://api.modrinth.com/v2";

#[derive(Debug, Deserialize, Serialize, Type)]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u32,
}

#[derive(Debug, Deserialize, Serialize, Type)]
pub struct SearchHit {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub downloads: u32,
    pub follows: u32,
    pub icon_url: Option<String>,
    pub categories: Vec<String>,
    pub client_side: String,
    pub server_side: String,
    pub project_id: String,
    pub project_type: String,
    pub versions: Vec<String>,
    pub date_created: String,
    pub date_modified: String,
    pub latest_version: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Type)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub body: String,
    pub icon_url: Option<String>,
    pub downloads: u32,
    pub follows: u32,
    pub categories: Vec<String>,
    pub client_side: String,
    pub server_side: String,
    pub versions: Vec<String>,
    pub gallery: Vec<GalleryImage>,
}

#[derive(Debug, Deserialize, Serialize, Type)]
pub struct GalleryImage {
    pub url: String,
    pub featured: bool,
    pub title: Option<String>,
    pub description: Option<String>,
    pub created: String,
}

#[derive(Debug, Deserialize, Serialize, Type)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub version_type: String,
    pub loaders: Vec<String>,
    pub featured: bool,
    pub files: Vec<VersionFile>,
    pub dependencies: Vec<Dependency>,
    pub downloads: u32,
    pub date_published: String,
}

#[derive(Debug, Deserialize, Serialize, Type)]
pub struct VersionFile {
    pub hashes: FileHashes,
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u32,
}

#[derive(Debug, Deserialize, Serialize, Type)]
pub struct FileHashes {
    pub sha1: String,
    pub sha512: String,
}

#[derive(Debug, Deserialize, Serialize, Type)]
pub struct Dependency {
    pub version_id: Option<String>,
    pub project_id: Option<String>,
    pub file_name: Option<String>,
    pub dependency_type: String,
}

#[derive(Debug, Default)]
pub struct SearchFilters {
    pub query: String,
    pub loader: Option<String>,
    pub game_version: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

pub async fn search(client: &reqwest::Client, filters: &SearchFilters) -> Result<SearchResult> {
    let url = format!("{}/search", MODRINTH_API);

    let mut facets: Vec<Vec<String>> = Vec::new();

    if let Some(ref loader) = filters.loader {
        facets.push(vec![format!("categories:{}", loader)]);
    }
    if let Some(ref version) = filters.game_version {
        facets.push(vec![format!("versions:{}", version)]);
    }

    let facet_query = serde_json::to_string(&facets).unwrap_or_default();

    let limit_str = filters.limit.to_string();
    let offset_str = filters.offset.to_string();

    let resp = client
        .get(&url)
        .header("User-Agent", "kidomc/0.1.0")
        .query(&[
            ("query", filters.query.as_str()),
            ("limit", limit_str.as_str()),
            ("offset", offset_str.as_str()),
            ("facets", facet_query.as_str()),
        ])
        .send()
        .await?;
    let result: SearchResult = resp.json().await?;
    Ok(result)
}

pub async fn get_project(client: &reqwest::Client, id_or_slug: &str) -> Result<Project> {
    let url = format!("{}/project/{}", MODRINTH_API, id_or_slug);
    let resp: Project = client
        .get(&url)
        .header("User-Agent", "kidomc/0.1.0")
        .send()
        .await?
        .json()
        .await?;
    Ok(resp)
}

pub async fn list_versions(
    client: &reqwest::Client,
    project_id: &str,
    loader: Option<&str>,
    game_version: Option<&str>,
) -> Result<Vec<Version>> {
    let url = format!("{}/project/{}/version", MODRINTH_API, project_id);

    let loader_json = loader.map(|l| format!(r#"["{}"]"#, l));
    let version_json = game_version.map(|v| format!(r#"["{}"]"#, v));

    let mut query_pairs: Vec<(&str, &str)> = Vec::new();
    if let Some(ref l) = loader_json {
        query_pairs.push(("loaders", l.as_str()));
    }
    if let Some(ref v) = version_json {
        query_pairs.push(("game_versions", v.as_str()));
    }

    let versions: Vec<Version> = client
        .get(&url)
        .header("User-Agent", "kidomc/0.1.0")
        .query(&query_pairs)
        .send()
        .await?
        .json()
        .await?;

    Ok(versions)
}

pub async fn get_version(client: &reqwest::Client, version_id: &str) -> Result<Version> {
    let url = format!("{}/version/{}", MODRINTH_API, version_id);
    let version: Version = client
        .get(&url)
        .header("User-Agent", "kidomc/0.1.0")
        .send()
        .await?
        .json()
        .await?;
    Ok(version)
}

pub fn pick_primary_file(version: &Version) -> Option<&VersionFile> {
    version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
}
