use std::path::PathBuf;
use tokio::sync::mpsc;
use tracing::info;

use crate::manifest::VersionManifest;
use crate::process::{spawn_minecraft, LaunchHandle};
use crate::Result;

#[derive(Debug, Clone)]
pub struct LaunchConfig {
    pub version_name: String,
    pub game_dir: PathBuf,
    pub assets_root: PathBuf,
    pub assets_index_name: String,
    pub auth_player_name: String,
    pub auth_uuid: String,
    pub auth_access_token: String,
    pub auth_xuid: String,
    pub user_type: String,
    pub version_type: String,
    pub min_memory_mb: u32,
    pub max_memory_mb: u32,
    pub java_binary: PathBuf,
    pub classpath: Vec<PathBuf>,
    pub native_dir: PathBuf,
    pub library_dir: PathBuf,
}

impl LaunchConfig {
    pub fn offline(username: &str) -> Self {
        let uuid = offline_uuid(username);
        Self {
            version_name: "unknown".to_string(),
            game_dir: PathBuf::from("."),
            assets_root: PathBuf::from("assets"),
            assets_index_name: "0".to_string(),
            auth_player_name: username.to_string(),
            auth_uuid: uuid.to_string(),
            auth_access_token: "0".to_string(),
            auth_xuid: "0".to_string(),
            user_type: "legacy".to_string(),
            version_type: "release".to_string(),
            min_memory_mb: 512,
            max_memory_mb: 2048,
            java_binary: PathBuf::from("java"),
            classpath: Vec::new(),
            native_dir: PathBuf::from("natives"),
            library_dir: PathBuf::from("libraries"),
        }
    }
}

pub fn build_arguments(
    manifest: &VersionManifest,
    config: &LaunchConfig,
) -> Vec<String> {
    let mut args = Vec::new();

    args.push(format!("-Xms{}m", config.min_memory_mb));
    args.push(format!("-Xmx{}m", config.max_memory_mb));

    args.push("-Dlog4j2.formatMsgNoLookups=true".to_string());

    let classpath = config
        .classpath
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(if cfg!(target_os = "windows") { ";" } else { ":" });

    if let Some(ref arguments) = manifest.arguments {
        for jvm_arg in &arguments.jvm {
            match jvm_arg {
                serde_json::Value::String(s) => {
                    if let Some(substituted) = jvm_substitute(s, config, &classpath) {
                        args.push(substituted);
                    }
                }
                serde_json::Value::Object(obj) => {
                    let rules = obj.get("rules");
                    let value = obj
                        .get("value")
                        .and_then(|v| v.as_str())
                        .or_else(|| obj.get("value").and_then(|v| v.as_array()?.first()?.as_str()));

                    if let Some(val) = value {
                        if rules_allow(rules) {
                            if let Some(substituted) = jvm_substitute(val, config, &classpath) {
                                args.push(substituted);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    } else {
        args.push(format!(
            "-Djava.library.path={}",
            config.native_dir.display()
        ));
        args.push("-cp".to_string());
        args.push(classpath);
    }

    args.push(manifest.main_class.clone());

    if let Some(ref arguments) = manifest.arguments {
        for game_arg in &arguments.game {
            match game_arg {
                serde_json::Value::String(s) => {
                    if let Some(substituted) = game_substitute(s, config) {
                        args.push(substituted);
                    }
                }
                serde_json::Value::Object(obj) => {
                    let rules = obj.get("rules");
                    let value = obj
                        .get("value")
                        .and_then(|v| v.as_str())
                        .or_else(|| obj.get("value").and_then(|v| v.as_array()?.first()?.as_str()));

                    if let Some(val) = value {
                        if rules_allow(rules) {
                            if let Some(substituted) = game_substitute(val, config) {
                                args.push(substituted);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    } else if let Some(ref mc_args) = manifest.minecraft_arguments {
        for arg in mc_args.split(' ') {
            if let Some(substituted) = game_substitute(arg, config) {
                if !substituted.is_empty() {
                    args.push(substituted);
                }
            }
        }
    }

    if let Some(ref logging) = manifest.logging {
        let log_arg = logging
            .client
            .argument
            .replace("${path}", &config.game_dir.join("logs").display().to_string())
            .replace("${id}", &logging.client.file.id);
        args.push(log_arg);
    }

    args
}

fn jvm_substitute(
    arg: &str,
    config: &LaunchConfig,
    classpath: &str,
) -> Option<String> {
    let arg = arg
        .replace("${classpath}", classpath)
        .replace("${natives_directory}", &config.native_dir.display().to_string())
        .replace("${library_directory}", &config.library_dir.display().to_string())
        .replace("${launcher_name}", "kidomc")
        .replace("${launcher_version}", "0.1.0");

    if arg.contains("${") && arg.contains('}') {
        return None;
    }

    Some(arg)
}

fn game_substitute(arg: &str, config: &LaunchConfig) -> Option<String> {
    let arg = arg
        .replace("${auth_player_name}", &config.auth_player_name)
        .replace("${auth_uuid}", &config.auth_uuid)
        .replace("${auth_access_token}", &config.auth_access_token)
        .replace("${auth_xuid}", &config.auth_xuid)
        .replace("${auth_session}", &config.auth_access_token)
        .replace("${user_properties}", "{}")
        .replace("${user_type}", &config.user_type)
        .replace("${version_name}", &config.version_name)
        .replace("${version_type}", &config.version_type)
        .replace("${game_directory}", &config.game_dir.display().to_string())
        .replace("${assets_root}", &config.assets_root.display().to_string())
        .replace("${assets_index_name}", &config.assets_index_name)
        .replace("${game_assets}", &config.assets_root.display().to_string())
        .replace("${clientid}", "0")
        .replace("${auth_xuid}", "0")
        .replace("${resolution_width}", "854")
        .replace("${resolution_height}", "480");

    Some(arg)
}

fn rules_allow(rules: Option<&serde_json::Value>) -> bool {
    let rules_arr = match rules {
        Some(r) => match r.as_array() {
            Some(arr) => arr,
            None => return true,
        },
        None => return true,
    };

    let mut allowed = false;

    for rule in rules_arr {
        let action = rule
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("allow");

        let has_features = rule.get("features").is_some();
        if has_features {
            continue;
        }

        let os_name = rule
            .get("os")
            .and_then(|o| o.get("name"))
            .and_then(|v| v.as_str());

        let os_arch = rule
            .get("os")
            .and_then(|o| o.get("arch"))
            .and_then(|v| v.as_str());

        let current_os = if cfg!(target_os = "macos") {
            "osx"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else {
            "linux"
        };

        let current_arch = if cfg!(target_arch = "x86_64") {
            "x86"
        } else if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x86"
        };

        let name_matches = os_name.map(|n| n == current_os).unwrap_or(true);
        let arch_matches = os_arch.map(|a| a == current_arch).unwrap_or(true);

        if name_matches && arch_matches {
            allowed = action == "allow";
        }
    }

    allowed
}

pub fn spawn(
    config: LaunchConfig,
    args: Vec<String>,
) -> Result<LaunchHandle> {
    info!(
        "launching minecraft {} via {}",
        config.version_name,
        config.java_binary.display()
    );
    let handle = spawn_minecraft(&config.java_binary, &args, &config.game_dir)?;
    Ok(handle)
}

fn offline_uuid(username: &str) -> uuid::Uuid {
    let namespace =
        uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
    uuid::Uuid::new_v5(
        &namespace,
        format!("OfflinePlayer:{}", username).as_bytes(),
    )
}

#[derive(Debug, Clone)]
pub struct PhaseProgress {
    pub label: String,
    pub completed: u64,
    pub total: u64,
    pub percent: u8,
}

pub async fn launch_vanilla(
    client: &reqwest::Client,
    paths: &crate::LauncherPaths,
    version: &str,
    memory_mb: u32,
    account: Option<&crate::auth::MicrosoftAccount>,
    fabric_loader_version: Option<&str>,
    game_dir: &std::path::Path,
    progress_tx: mpsc::UnboundedSender<PhaseProgress>,
) -> Result<LaunchHandle> {
    use crate::{assets, download, jvm, libraries, manifest};

    let v2 = manifest::fetch_version_manifest_v2(
        client,
        &paths.meta_dir().join("manifests"),
    )
    .await?;

    let entry = manifest::find_version(&v2, version)?;

    let mut version_manifest = manifest::fetch_version_manifest(
        client,
        entry,
        &paths.meta_dir().join("versions"),
    )
    .await?;

    if let Some(loader_ver) = fabric_loader_version {
        info!("merging fabric loader {} for mc {}", loader_ver, version);
        let fabric_profile = crate::loaders::fabric::get_profile_json(
            client, version, loader_ver,
        )
        .await?;

        if let Some(fabric_libs) = fabric_profile.get("libraries").and_then(|v| v.as_array()) {
            for lib in fabric_libs {
                if let Some(converted) =
                    crate::loaders::fabric::fabric_library_to_vanilla_format(lib)
                {
                    version_manifest.libraries.push(converted);
                }
            }
        }

        if let Some(main_class) = fabric_profile
            .get("mainClass")
            .and_then(|v| v.as_str())
        {
            version_manifest.main_class = main_class.to_string();
        }
    }

    let required_java = version_manifest
        .java_version
        .as_ref()
        .map(|jv| jv.major_version)
        .unwrap_or(8);

    let (jvm_tx, jvm_rx) = mpsc::unbounded_channel();
    let jvm_downloader =
        download::Downloader::new(client.clone()).with_progress(jvm_tx);
    let jvm_dir = paths.jvm_dir();

    let jvm_handle = tokio::spawn(async move {
        jvm::find_or_download_jvm(&jvm_downloader, required_java, &jvm_dir).await
    });

    let mut jvm_tracker = download::ProgressTracker::new(jvm_rx, 1, "jvm");
    while (jvm_tracker.recv().await).is_some() {}
    let jvm = jvm_handle.await.map_err(|e| {
        crate::Error::Other(format!("jvm task panicked: {}", e))
    })??;

    let libs = libraries::parse_libraries(&version_manifest.libraries)?;
    let lib_count = libraries::count_downloads(&libs);

    let (lib_tx, lib_rx) = mpsc::unbounded_channel();
    let lib_downloader =
        download::Downloader::new(client.clone()).with_progress(lib_tx);
    let libs_dir = paths.libraries_dir();
    let native_dir = paths.base_dir.join("natives");

    let libs_handle = tokio::spawn(async move {
        libraries::resolve_libraries(&lib_downloader, &libs, &libs_dir, &native_dir).await
    });

    let mut lib_tracker = download::ProgressTracker::new(lib_rx, lib_count, "libraries");
    while let Some(_) = lib_tracker.recv().await {
        let _ = progress_tx.send(PhaseProgress {
            label: "libraries".into(),
            completed: 0,
            total: 0,
            percent: lib_tracker.percentage(),
        });
    }
    let resolved = libs_handle.await.map_err(|e| {
        crate::Error::Other(format!("library task panicked: {}", e))
    })??;

    let client_jar_path = paths
        .meta_dir()
        .join("versions")
        .join(format!("{}.jar", version));
    let client_downloader = download::Downloader::new(client.clone());
    client_downloader
        .download(
            &version_manifest.downloads.client.url,
            &client_jar_path,
            Some(&version_manifest.downloads.client.sha1),
        )
        .await?;

    let asset_index_info = &version_manifest.asset_index;
    let asset_index = assets::fetch_asset_index(
        client,
        asset_index_info,
        &paths.assets_dir().join("indexes"),
    )
    .await?;

    let asset_count = asset_index.objects.len() as u64;
    let (asset_tx, asset_rx) = mpsc::unbounded_channel();
    let asset_downloader =
        download::Downloader::new(client.clone()).with_progress(asset_tx);
    let objects_dir = paths.assets_dir().join("objects");

    let assets_handle = tokio::spawn({
        let idx = asset_index.clone();
        async move {
            assets::resolve_assets(&asset_downloader, &idx, &objects_dir).await
        }
    });

    let mut asset_tracker = download::ProgressTracker::new(asset_rx, asset_count, "assets");
    while let Some(_) = asset_tracker.recv().await {
        let _ = progress_tx.send(PhaseProgress {
            label: "assets".into(),
            completed: 0,
            total: 0,
            percent: asset_tracker.percentage(),
        });
    }
    assets_handle.await.map_err(|e| {
        crate::Error::Other(format!("asset task panicked: {}", e))
    })??;

    let game_dir = game_dir.to_path_buf();
    tokio::fs::create_dir_all(&game_dir).await?;
    tokio::fs::create_dir_all(game_dir.join("logs")).await?;

    let mut classpath = resolved.classpath.clone();
    classpath.push(client_jar_path);

    let (auth_player_name, auth_uuid, auth_token, auth_xuid, user_type) =
        match account {
            Some(acc) => (
                acc.username.clone(),
                acc.uuid.clone(),
                acc.access_token.clone(),
                acc.xuid.clone(),
                "msa".to_string(),
            ),
            None => (
                "Player".to_string(),
                LaunchConfig::offline("Player").auth_uuid,
                "0".to_string(),
                "0".to_string(),
                "legacy".to_string(),
            ),
        };

    let config = LaunchConfig {
        version_name: version.to_string(),
        game_dir,
        assets_root: paths.assets_dir(),
        assets_index_name: asset_index_info.id.clone(),
        auth_player_name,
        auth_uuid,
        auth_access_token: auth_token,
        auth_xuid,
        user_type,
        version_type: entry.version_type.clone(),
        min_memory_mb: 512,
        max_memory_mb: memory_mb,
        java_binary: jvm.java_binary.clone(),
        classpath,
        native_dir: resolved.native_dir.clone(),
        library_dir: paths.libraries_dir(),
    };

    let args = build_arguments(&version_manifest, &config);

    spawn(config, args)
}
