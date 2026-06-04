use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

use crate::download::Downloader;
use crate::Result;

#[derive(Debug, Clone, Deserialize)]
pub struct Library {
    pub name: String,
    pub downloads: LibraryDownloads,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub natives: HashMap<String, String>,
    #[serde(default)]
    pub extract: Option<ExtractConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<ArtifactInfo>,
    #[serde(default)]
    pub classifiers: HashMap<String, ArtifactInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArtifactInfo {
    pub path: Option<String>,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<OsCondition>,
    #[serde(default)]
    pub features: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsCondition {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractConfig {
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedLibraries {
    pub classpath: Vec<PathBuf>,
    pub native_dir: PathBuf,
}

fn current_os_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "osx"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "linux"
    }
}

fn current_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86"
    }
}

fn library_applies(lib: &Library) -> bool {
    if lib.rules.is_empty() {
        return true;
    }

    let mut allowed = false;

    for rule in &lib.rules {
        let matches = match &rule.os {
            Some(os) => {
                let name_matches = os
                    .name
                    .as_ref()
                    .map(|n| n == current_os_name())
                    .unwrap_or(true);
                let arch_matches = os
                    .arch
                    .as_ref()
                    .map(|a| a == current_arch())
                    .unwrap_or(true);
                name_matches && arch_matches
            }
            None => {
                rule.features.is_none()
            }
        };

        if matches {
            allowed = rule.action == "allow";
        }
    }

    allowed
}

pub fn parse_libraries(raw: &[serde_json::Value]) -> Result<Vec<Library>> {
    let mut libs = Vec::new();
    for val in raw {
        let lib: Library = serde_json::from_value(val.clone())?;
        libs.push(lib);
    }
    Ok(libs)
}

pub fn count_downloads(libs: &[Library]) -> u64 {
    let mut count = 0u64;
    for lib in libs {
        if !library_applies(lib) {
            continue;
        }
        if lib.downloads.artifact.is_some() {
            count += 1;
        }
        if let Some(classifier_key) = lib.natives.get(current_os_name()) {
            if lib.downloads.classifiers.contains_key(classifier_key) {
                count += 1;
            }
        }
    }
    count
}

pub async fn resolve_libraries(
    downloader: &Downloader,
    libs: &[Library],
    libraries_dir: &Path,
    native_dir: &Path,
) -> Result<ResolvedLibraries> {
    let mut classpath = Vec::new();
    tokio::fs::create_dir_all(libraries_dir).await?;
    tokio::fs::create_dir_all(native_dir).await?;

    for lib in libs {
        if !library_applies(lib) {
            debug!("skipping library {} (rule mismatch)", lib.name);
            continue;
        }

        let mut got_native_jar_to_extract = None;

        if let Some(artifact) = &lib.downloads.artifact {
            let path_str = artifact
                .path
                .clone()
                .unwrap_or_else(|| maven_path_from_name(&lib.name));
            let dest = libraries_dir.join(&path_str);

            let expected_sha1 = if artifact.sha1.is_empty() {
                None
            } else {
                Some(artifact.sha1.as_str())
            };
            downloader
                .download(&artifact.url, &dest, expected_sha1)
                .await?;

            let is_native = is_native_classifier_artifact(&lib.name, &artifact);
            if is_native {
                got_native_jar_to_extract = Some(dest);
            } else {
                classpath.push(dest);
            }
        }

        let has_natives = lib.natives.get(current_os_name());
        if let Some(classifier_key) = has_natives {
            if let Some(classifier) = lib.downloads.classifiers.get(classifier_key) {
                let path_str = classifier
                    .path
                    .clone()
                    .unwrap_or_else(|| maven_classifier_path(&lib.name, classifier_key));
                let dest = libraries_dir.join(&path_str);

                    let expected_sha1 = if classifier.sha1.is_empty() {
                        None
                    } else {
                        Some(classifier.sha1.as_str())
                    };
                    downloader
                        .download(&classifier.url, &dest, expected_sha1)
                    .await?;

                got_native_jar_to_extract = Some(dest);
            }
        }

        if let Some(jar_path) = got_native_jar_to_extract {
            let exclude: Vec<&str> = lib
                .extract
                .as_ref()
                .map(|e| e.exclude.iter().map(|s| s.as_str()).collect())
                .unwrap_or_default();

            extract_natives(&jar_path, native_dir, &exclude).await?;
        }
    }

    info!(
        "resolved {} libraries, {} native dirs",
        classpath.len(),
        1
    );

    Ok(ResolvedLibraries {
        classpath,
        native_dir: native_dir.to_path_buf(),
    })
}

async fn extract_natives(jar_path: &Path, dest: &Path, exclude: &[&str]) -> Result<()> {
    let file = std::fs::File::open(jar_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = match entry.enclosed_name() {
            Some(n) => n.to_path_buf(),
            None => continue,
        };

        let name_str = name.to_string_lossy();

        if name_str.starts_with("META-INF/") {
            continue;
        }

        let should_exclude = exclude.iter().any(|ex| name_str.starts_with(*ex));
        if should_exclude {
            continue;
        }

        if entry.is_dir() {
            continue;
        }

        let out_path = dest.join(&name);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut out_file = std::fs::File::create(&out_path)?;
        std::io::copy(&mut entry, &mut out_file)?;
    }

    Ok(())
}

fn maven_path_from_name(name: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() == 3 {
        let group = parts[0].replace('.', "/");
        let artifact = parts[1];
        let version = parts[2];
        format!("{}/{}/{}/{}-{}.jar", group, artifact, version, artifact, version)
    } else {
        format!("{}.jar", name)
    }
}

fn maven_classifier_path(name: &str, classifier: &str) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() == 3 {
        let group = parts[0].replace('.', "/");
        let artifact = parts[1];
        let version = parts[2];
        format!(
            "{}/{}/{}/{}-{}-{}.jar",
            group, artifact, version, artifact, version, classifier
        )
    } else {
        format!("{}-{}.jar", name, classifier)
    }
}

fn is_native_classifier_artifact(name: &str, artifact: &ArtifactInfo) -> bool {
    if name.contains("natives-") {
        return true;
    }
    if let Some(ref path) = artifact.path {
        let filename = std::path::Path::new(path)
            .file_name()
            .map(|f| f.to_string_lossy())
            .unwrap_or_default();
        if filename.contains("natives-") {
            return true;
        }
    }
    false
}
