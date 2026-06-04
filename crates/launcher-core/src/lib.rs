use std::path::PathBuf;

pub mod manifest;
pub mod download;
pub mod libraries;
pub mod assets;
pub mod jvm;
pub mod launch;
pub mod process;
pub mod auth;
pub mod instance;
pub mod loaders;
pub mod modrinth;
pub mod modpack;
pub mod curseforge;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("sha1 mismatch for {url}: expected {expected}, got {actual}")]
    Sha1Mismatch {
        url: String,
        expected: String,
        actual: String,
    },

    #[error("version {0} not found")]
    VersionNotFound(String),

    #[error("no suitable java runtime found (need java {0}+)")]
    NoJavaFound(u32),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("launch failed: {0}")]
    Launch(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct LauncherPaths {
    pub base_dir: PathBuf,
}

impl LauncherPaths {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn meta_dir(&self) -> PathBuf {
        self.base_dir.join("meta")
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.base_dir.join("assets")
    }

    pub fn libraries_dir(&self) -> PathBuf {
        self.base_dir.join("libraries")
    }

    pub fn jvm_dir(&self) -> PathBuf {
        self.base_dir.join("jvm")
    }

    pub fn instances_dir(&self) -> PathBuf {
        self.base_dir.join("instances")
    }
}

pub fn default_paths() -> LauncherPaths {
    let base = dirs_data_dir().unwrap_or_else(|| PathBuf::from(".kidomc"));
    LauncherPaths::new(base)
}

fn dirs_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME").ok().map(|h| {
            PathBuf::from(h)
                .join("Library")
                .join("Application Support")
                .join("kidomc")
        })
    }

    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".local").join("share"))
            })
            .map(|p| p.join("kidomc"))
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}
