use serde::Serialize;
use specta::Type;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tracing::info;

use crate::Result;

#[derive(Debug, Clone, Serialize, Type)]
pub struct LogLine {
    pub stream: OutputStream,
    pub text: String,
    pub level: LogLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
    Trace,
}

pub struct LaunchHandle {
    child: tokio::process::Child,
    log_rx: mpsc::UnboundedReceiver<LogLine>,
}

impl LaunchHandle {
    pub fn log_receiver(&mut self) -> &mut mpsc::UnboundedReceiver<LogLine> {
        &mut self.log_rx
    }

    pub fn take_log_receiver(&mut self) -> mpsc::UnboundedReceiver<LogLine> {
        std::mem::replace(&mut self.log_rx, mpsc::unbounded_channel().1)
    }

    pub async fn wait(&mut self) -> Result<i32> {
        let status = self.child.wait().await?;
        Ok(status.code().unwrap_or(-1))
    }

    pub async fn kill(&mut self) -> Result<()> {
        self.child.kill().await?;
        Ok(())
    }
}

pub fn spawn_minecraft(
    java_binary: &std::path::Path,
    args: &[String],
    current_dir: &std::path::Path,
) -> Result<LaunchHandle> {
    info!("spawning minecraft: {} {}", java_binary.display(), args[..].join(" "));

    let mut cmd = tokio::process::Command::new(java_binary);
    cmd.args(args)
        .current_dir(current_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn()?;

    let stdout = child.stdout.take().expect("stdout not piped");
    let stderr = child.stderr.take().expect("stderr not piped");

    let (log_tx, log_rx) = mpsc::unbounded_channel();

    tokio::spawn(read_stream(stdout, OutputStream::Stdout, log_tx.clone()));
    tokio::spawn(read_stream(stderr, OutputStream::Stderr, log_tx));

    Ok(LaunchHandle { child, log_rx })
}

async fn read_stream(
    stream: impl tokio::io::AsyncRead + Unpin + Send + 'static,
    source: OutputStream,
    tx: mpsc::UnboundedSender<LogLine>,
) {
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();

    let mut xml_buffer = String::new();
    let mut in_xml = false;

    while let Ok(Some(line)) = lines.next_line().await {
        if line.starts_with("<log4j:Event") {
            in_xml = true;
            xml_buffer = line;
        } else if in_xml {
            xml_buffer.push('\n');
            xml_buffer.push_str(&line);
            if line.contains("</log4j:Event>") {
                in_xml = false;
                if let Some(log_line) = parse_log4j_xml(&xml_buffer, source) {
                    let _ = tx.send(log_line);
                }
                xml_buffer.clear();
            }
        } else {
            let level = detect_log_level(&line);
            let _ = tx.send(LogLine {
                stream: source,
                text: line,
                level,
            });
        }
    }
}

fn detect_log_level(line: &str) -> LogLevel {
    let lower = line.to_lowercase();
    if lower.contains("error") || lower.contains("fail") || lower.contains("exception") {
        LogLevel::Error
    } else if lower.contains("warn") {
        LogLevel::Warn
    } else if lower.contains("debug") {
        LogLevel::Debug
    } else if lower.contains("trace") {
        LogLevel::Trace
    } else {
        LogLevel::Info
    }
}

fn parse_log4j_xml(xml: &str, source: OutputStream) -> Option<LogLine> {
    let level = if xml.contains("ERROR") {
        LogLevel::Error
    } else if xml.contains("WARN") {
        LogLevel::Warn
    } else if xml.contains("DEBUG") {
        LogLevel::Debug
    } else if xml.contains("TRACE") {
        LogLevel::Trace
    } else {
        LogLevel::Info
    };

    let message = extract_xml_content(xml, "Message")
        .or_else(|| extract_xml_content(xml, "Throwable"))
        .unwrap_or_else(|| xml.to_string());

    Some(LogLine {
        stream: source,
        text: message,
        level,
    })
}

fn extract_xml_content(xml: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);

    let start = xml.find(&start_tag)? + start_tag.len();
    let end = xml.find(&end_tag)?;

    Some(xml[start..end].to_string())
}
