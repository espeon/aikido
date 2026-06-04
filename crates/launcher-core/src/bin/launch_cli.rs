use launcher_core::{auth, launch};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info,launcher_core=info")
        .init();

    let args: Vec<String> = std::env::args().collect();
    let do_login = args.iter().any(|a| a == "--login");

    if do_login {
        let client = reqwest::Client::builder()
            .user_agent("kidomc/0.1.0")
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("failed to build http client");

        auth::full_login(&client).await.expect("login failed");
        return;
    }

    let version = parse_arg(&args, "--version", "1.21.1");
    let memory_mb: u32 = parse_arg(&args, "--memory", "4096")
        .parse()
        .unwrap_or(4096);

    let paths = launcher_core::default_paths();
    let client = reqwest::Client::builder()
        .user_agent("kidomc/0.1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("failed to build http client");

    let account = auth::get_login_account(&client)
        .await
        .unwrap_or(None);

    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();

    let game_dir = paths.instances_dir().join(&version);
    let launch_future = launch::launch_vanilla(
        &client,
        &paths,
        &version,
        memory_mb,
        account.as_ref(),
        None,
        &game_dir,
        progress_tx,
    );

    tokio::spawn(async move {
        while let Some(p) = progress_rx.recv().await {
            tracing::info!("{}: {}%", p.label, p.percent);
        }
    });

    let mut handle = launch_future.await.expect("launch failed");

    let mut log_rx = handle.take_log_receiver();
    let ctrl_c = tokio::signal::ctrl_c();

    tokio::select! {
        _ = async {
            while let Some(log) = log_rx.recv().await {
                let prefix = match log.stream {
                    launcher_core::process::OutputStream::Stdout => "O",
                    launcher_core::process::OutputStream::Stderr => "E",
                };
                eprintln!("[{}] {}", prefix, log.text);
            }
        } => {}

        _ = async {
            let code = handle.wait().await.unwrap_or(-1);
            tracing::info!("minecraft exited with code {}", code);
        } => {}

        _ = ctrl_c => {
            tracing::info!("received ctrl-c, killing minecraft...");
            handle.kill().await.ok();
        }
    }

    tracing::info!("done");
}

fn parse_arg(args: &[String], flag: &str, default: &str) -> String {
    let idx = args.iter().position(|a| a == flag);
    idx.and_then(|i| args.get(i + 1).cloned())
        .unwrap_or_else(|| default.to_string())
}
