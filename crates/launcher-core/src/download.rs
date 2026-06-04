use sha1_smol::Digest;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::Result;

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Started { url: String, total_size: u64 },
    Progress { url: String, downloaded: u64 },
    Completed { url: String },
    AlreadyCached { url: String },
}

pub struct ProgressTracker {
    rx: mpsc::UnboundedReceiver<ProgressEvent>,
    total: u64,
    completed: u64,
    label: String,
}

impl ProgressTracker {
    pub fn new(rx: mpsc::UnboundedReceiver<ProgressEvent>, total: u64, label: &str) -> Self {
        Self {
            rx,
            total,
            completed: 0,
            label: label.to_string(),
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn percentage(&self) -> u8 {
        if self.total == 0 {
            return 100;
        }
        ((self.completed * 100) / self.total) as u8
    }

    pub fn is_done(&self) -> bool {
        self.completed >= self.total
    }

    pub async fn recv(&mut self) -> Option<ProgressEvent> {
        let event = self.rx.recv().await?;
        if matches!(
            event,
            ProgressEvent::Completed { .. } | ProgressEvent::AlreadyCached { .. }
        ) {
            self.completed += 1;
            info!(
                "{} [{}/{}] {}%",
                self.label,
                self.completed,
                self.total,
                self.percentage()
            );
        }
        Some(event)
    }
}

pub struct Downloader {
    client: reqwest::Client,
    progress_tx: Option<mpsc::UnboundedSender<ProgressEvent>>,
    retries: u32,
}

impl Downloader {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            progress_tx: None,
            retries: 3,
        }
    }

    pub fn with_progress(mut self, tx: mpsc::UnboundedSender<ProgressEvent>) -> Self {
        self.progress_tx = Some(tx);
        self
    }

    fn emit(&self, event: ProgressEvent) {
        if let Some(tx) = &self.progress_tx {
            let _ = tx.send(event);
        }
    }

    pub async fn download(
        &self,
        url: &str,
        dest: &Path,
        expected_sha1: Option<&str>,
    ) -> Result<()> {
        if let Some(expected) = expected_sha1 {
            if dest.exists() {
                let actual = sha1_file(dest).await?;
                if actual == expected {
                    debug!("file already cached with matching sha1: {}", url);
                    self.emit(ProgressEvent::AlreadyCached {
                        url: url.to_string(),
                    });
                    return Ok(());
                }
                warn!(
                    "cached file has wrong sha1 (expected {}, got {}), re-downloading: {}",
                    expected, actual, url
                );
                tokio::fs::remove_file(dest).await.ok();
            }
        }

        let dir = dest.parent().unwrap();
        tokio::fs::create_dir_all(dir).await?;

        let part_path = part_path(dest);
        let mut retry_count = 0;

        loop {
            match self
                .download_inner(url, &part_path, expected_sha1)
                .await
            {
                Ok(()) => {
                    tokio::fs::rename(&part_path, dest).await?;
                    self.emit(ProgressEvent::Completed {
                        url: url.to_string(),
                    });
                    return Ok(());
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= self.retries {
                        return Err(e);
                    }
                    warn!(
                        "download attempt {}/{} failed for {}: {}",
                        retry_count, self.retries, url, e
                    );
                    tokio::fs::remove_file(&part_path).await.ok();
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    }

    async fn download_inner(
        &self,
        url: &str,
        dest: &Path,
        expected_sha1: Option<&str>,
    ) -> Result<()> {
        let mut req = self
            .client
            .get(url)
            .header("User-Agent", "kidomc/0.1.0")
            .timeout(std::time::Duration::from_secs(300));

        if dest.exists() {
            let existing_size = tokio::fs::metadata(dest).await?.len();
            req = req.header("Range", format!("bytes={}-", existing_size));
        }

        let response = req.send().await?;

        let status = response.status();
        if !status.is_success() && status.as_u16() != 206 {
            return Err(crate::Error::Other(format!(
                "download failed for {}: HTTP {}",
                url, status
            )));
        }

        let total_size = response.content_length().unwrap_or(0);
        let is_resume = status.as_u16() == 206;

        self.emit(ProgressEvent::Started {
            url: url.to_string(),
            total_size,
        });

        let mut file = if is_resume {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(dest)
                .await?
        } else {
            tokio::fs::File::create(dest).await?
        };

        let mut downloaded: u64 = if is_resume {
            tokio::fs::metadata(dest).await?.len()
        } else {
            0
        };

        let mut stream = response.bytes_stream();
        let mut last_progress = 0u64;

        while let Some(chunk) = futures_util::TryStreamExt::try_next(&mut stream).await? {
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            if downloaded - last_progress > 1024 * 1024 {
                self.emit(ProgressEvent::Progress {
                    url: url.to_string(),
                    downloaded,
                });
                last_progress = downloaded;
            }
        }

        file.flush().await?;

        if let Some(expected) = expected_sha1 {
            let actual = sha1_file(dest).await?;
            if actual != expected {
                return Err(crate::Error::Sha1Mismatch {
                    url: url.to_string(),
                    expected: expected.to_string(),
                    actual,
                });
            }
        }

        Ok(())
    }
}

pub async fn sha1_file(path: &Path) -> Result<String> {
    let data = tokio::fs::read(path).await?;
    let mut hasher = sha1_smol::Sha1::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()))
}

fn part_path(path: &Path) -> PathBuf {
    let mut p = path.to_path_buf();
    let mut name = p
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    name.push_str(".part");
    p.set_file_name(name);
    p
}
