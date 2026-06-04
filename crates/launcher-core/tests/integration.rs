use launcher_core::modpack::mrpack;
use std::path::Path;

#[tokio::test]
async fn test_parse_fabulously_optimized() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Fabulously.Optimized-v13.2.0-beta.4.mrpack");
    if !path.exists() {
        eprintln!("skipping: mrpack not found at {:?}", path);
        return;
    }

    let (index, _data) = mrpack::read_mrpack(&path).await.expect("failed to parse mrpack");

    assert_eq!(index.game, "minecraft");
    assert!(index.name.contains("Fabulously"));
    assert!(index.dependencies.contains_key("minecraft"));
    assert!(index.dependencies.contains_key("fabric-loader"));
    assert!(!index.files.is_empty(), "should have mod files");

    let mc_version = index.dependencies.get("minecraft").unwrap();
    let loader_version = index.dependencies.get("fabric-loader").unwrap();
    eprintln!(
        "pack: {} (mc {}, fabric {}) — {} mods",
        index.name, mc_version, loader_version, index.files.len()
    );

    for file in &index.files {
        let url = &file.downloads[0];
        mrpack::validate_download_url(url).expect(&format!("invalid url: {}", url));

        if let Some(ref env) = file.env {
            assert!(!env.client.is_empty());
        }
    }

    eprintln!("all {} files have valid download urls", index.files.len());
}

#[tokio::test]
async fn test_instance_create_and_delete() {
    let tmp = std::env::temp_dir().join("kidomc-test");
    let paths = launcher_core::LauncherPaths::new(tmp.clone());

    let spec = launcher_core::instance::NewInstanceSpec {
        name: "test instance".into(),
        minecraft_version: "1.21.1".into(),
        loader: launcher_core::instance::LoaderInfo::Vanilla,
        memory_mb: 2048,
    };

    let id = launcher_core::instance::create_instance(&paths, &spec)
        .await
        .expect("create failed");
    assert!(!id.is_empty());

    let instances = launcher_core::instance::list_instances(&paths)
        .await
        .expect("list failed");
    assert!(instances.iter().any(|i| i.id == id));

    let detail = launcher_core::instance::get_instance(&paths, &id)
        .await
        .expect("get failed");
    assert_eq!(detail.summary.name, "test instance");
    assert_eq!(detail.memory_mb, 2048);

    launcher_core::instance::delete_instance(&paths, &id)
        .await
        .expect("delete failed");

    let remaining = launcher_core::instance::list_instances(&paths)
        .await
        .expect("list after delete failed");
    assert!(!remaining.iter().any(|i| i.id == id));

    tokio::fs::remove_dir_all(&tmp).await.ok();
}
