use std::path::{Path, PathBuf};
use tracing::info;

use crate::download::Downloader;
use crate::Result;

#[derive(Debug, Clone)]
pub struct JvmInfo {
    pub path: PathBuf,
    pub major_version: u32,
    pub java_binary: PathBuf,
}

pub async fn find_or_download_jvm(
    downloader: &Downloader,
    required_major: u32,
    jvm_dir: &Path,
) -> Result<JvmInfo> {
    let search_paths: Vec<PathBuf> = if cfg!(target_os = "macos") {
        let home = std::env::var("HOME").unwrap_or_default();
        vec![
            jvm_dir.join(format!("temurin-{}-{}", required_major, os_arch())),
            PathBuf::from("/Library/Java/JavaVirtualMachines"),
            PathBuf::from(format!("{}/.sdkman/candidates/java", home)),
            PathBuf::from("/opt/homebrew/opt"),
        ]
    } else {
        let home = std::env::var("HOME").unwrap_or_default();
        vec![
            jvm_dir.join(format!("temurin-{}-{}", required_major, os_arch())),
            PathBuf::from("/usr/lib/jvm"),
            PathBuf::from(format!("{}/.sdkman/candidates/java", home)),
            PathBuf::from("/usr/local/lib/jvm"),
        ]
    };

    for base in &search_paths {
        if let Some(jvm) = find_java_in_dir(base, required_major).await {
            info!("found suitable JVM at {}", jvm.java_binary.display());
            return Ok(jvm);
        }
    }

    download_jvm(downloader, required_major, jvm_dir).await
}

async fn find_java_in_dir(base: &Path, required_major: u32) -> Option<JvmInfo> {
    if !base.exists() {
        return None;
    }

    let mut candidates = Vec::new();

    if let Ok(mut entries) = tokio::fs::read_dir(base).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let java = if cfg!(target_os = "macos") {
                let home_with_contents = path.join("Contents").join("Home");
                let java_with_contents = home_with_contents.join("bin").join("java");
                if java_with_contents.exists() {
                    java_with_contents
                } else {
                    path.join("bin").join("java")
                }
            } else {
                path.join("bin").join("java")
            };

            if java.exists() {
                if let Some(ver) = check_java_version(&java).await {
                    candidates.push((ver, path, java));
                }
            }
        }
    }

    candidates.sort_by_key(|(ver, _, _)| *ver);

    for (ver, path, java) in candidates {
        if ver >= required_major {
            return Some(JvmInfo {
                path,
                major_version: ver,
                java_binary: java,
            });
        }
    }

    None
}

async fn check_java_version(java_bin: &Path) -> Option<u32> {
    let output = tokio::process::Command::new(java_bin)
        .arg("-version")
        .output()
        .await
        .ok()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_java_version(&stderr)
}

fn parse_java_version(output: &str) -> Option<u32> {
    for line in output.lines() {
        let lower = line.to_lowercase();
        if lower.contains("version") || lower.contains("runtime") {
            if let Some(quoted) = line.split('"').nth(1) {
                let ver = quoted.trim();
                let major: u32 = if ver.starts_with("1.") {
                    ver.split('.').nth(1).and_then(|s| s.parse().ok()).unwrap_or(8)
                } else {
                    ver.split('.').next().and_then(|s| s.parse().ok()).unwrap_or(0)
                };
                if major > 0 {
                    return Some(major);
                }
            }
        }
    }
    None
}

async fn download_jvm(
    downloader: &Downloader,
    major: u32,
    jvm_dir: &Path,
) -> Result<JvmInfo> {
    let os = if cfg!(target_os = "macos") { "mac" } else { "linux" };
    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    };

    let url = format!(
        "https://api.adoptium.net/v3/binary/latest/{}/ga/{}/{}/jre/hotspot/normal/eclipse",
        major, os, arch
    );

    info!("downloading JVM {} from adoptium", major);

    let archive_path = jvm_dir.join(format!("temurin-{}-{}.tar.gz", major, os_arch()));
    tokio::fs::create_dir_all(jvm_dir).await?;

    downloader.download(&url, &archive_path, None).await?;

    let extract_dir = jvm_dir.join(format!("temurin-{}-{}", major, os_arch()));
    if extract_dir.exists() {
        tokio::fs::remove_dir_all(&extract_dir).await.ok();
    }

    info!("extracting jvm to {}", extract_dir.display());

    extract_tar_gz(&archive_path, &extract_dir)?;

    let java_bin = find_java_in_extracted(&extract_dir)?;

    info!("jvm ready at {}", java_bin.display());

    tokio::fs::remove_file(&archive_path).await.ok();

    Ok(JvmInfo {
        path: extract_dir,
        major_version: major,
        java_binary: java_bin,
    })
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;

    let file = std::fs::File::open(archive)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    archive.unpack(dest)?;

    Ok(())
}

fn find_java_in_extracted(dir: &Path) -> Result<PathBuf> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let java = path.join("bin").join("java");
            if java.exists() {
                return Ok(java);
            }
            if let Ok(sub) = find_java_in_extracted(&path) {
                return Ok(sub);
            }
        }
    }
    Err(crate::Error::NoJavaFound(0))
}

fn os_arch() -> String {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "mac-aarch64".to_string()
        } else {
            "mac-x64".to_string()
        }
    } else {
        if cfg!(target_arch = "aarch64") {
            "linux-aarch64".to_string()
        } else {
            "linux-x64".to_string()
        }
    }
}
