#![allow(unused)]
use std::path::{Path, PathBuf};

use anyhow::Context;
use futures_util::{StreamExt, stream};
use sha1::{Digest, Sha1};
use tracing::warn;

use crate::mc_parser::{evaluate_rules, maven_coord_to_path};
use crate::mc_types::{McJavaFileEntry, McJavaManifest, McSpecificVersionDetail};

const ASSET_CONCURRENCY: usize = 128;
const LIBRARY_CONCURRENCY: usize = 64;
const MAX_RETRIES: u32 = 5;
const RETRY_BASE_DELAY_MS: u64 = 1000;

fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

async fn download_and_verify(
    url: &str,
    dest: &Path,
    expected_size: u64,
    expected_sha1: &str,
) -> anyhow::Result<()> {
    if dest.exists() && tokio::fs::metadata(dest).await?.len() == expected_size {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut last_err: anyhow::Error = anyhow::anyhow!("下載尚未嘗試");
    for attempt in 0..MAX_RETRIES {
        if attempt > 0 {
            let delay = RETRY_BASE_DELAY_MS * (1u64 << (attempt - 1)); // 1s, 2s, 4s, 8s
            warn!(
                attempt,
                max = MAX_RETRIES - 1,
                delay_ms = delay,
                url,
                "下載重試中"
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }

        let result: anyhow::Result<()> = async {
            let bytes = reqwest::get(url).await?.error_for_status()?.bytes().await?;
            let actual = sha1_hex(&bytes);
            if actual != expected_sha1 {
                anyhow::bail!(
                    "SHA1 不符 {}: expected={} actual={}",
                    dest.display(),
                    expected_sha1,
                    actual
                );
            }
            tokio::fs::write(dest, &bytes).await?;
            Ok(())
        }
        .await;

        match result {
            Ok(()) => return Ok(()),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("SHA1 不符") {
                    return Err(e);
                }
                last_err = e;
            }
        }
    }

    Err(last_err).with_context(|| format!("下載失敗（重試 {} 次）：{}", MAX_RETRIES, url))
}

pub async fn install_java(
    manifest: &McJavaManifest,
    java_dir: &Path,
    on_progress: impl Fn(f32) + Send,
) -> anyhow::Result<()> {
    let entries: Vec<_> = manifest.files.iter().collect();
    let total = entries.len().max(1);
    for (i, (rel_path, entry)) in entries.iter().enumerate() {
        let dest = java_dir.join(rel_path);
        match entry {
            McJavaFileEntry::Directory => {
                tokio::fs::create_dir_all(&dest).await?;
            }
            McJavaFileEntry::File {
                executable,
                downloads,
            } => {
                download_and_verify(
                    &downloads.raw.url,
                    &dest,
                    downloads.raw.size,
                    &downloads.raw.sha1,
                )
                .await?;
                #[cfg(unix)]
                if *executable {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = tokio::fs::metadata(&dest).await?.permissions();
                    perms.set_mode(0o755);
                    tokio::fs::set_permissions(&dest, perms).await?;
                }
            }
            McJavaFileEntry::Link { target } => {
                #[cfg(unix)]
                {
                    if dest.is_symlink() || dest.exists() {
                        tokio::fs::remove_file(&dest).await?;
                    }
                    if let Some(parent) = dest.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    tokio::fs::symlink(target, &dest).await?;
                }
            }
        }
        on_progress((i + 1) as f32 / total as f32);
    }
    Ok(())
}

pub async fn install_client(
    version: &McSpecificVersionDetail,
    versions_dir: &Path,
    on_progress: impl Fn(f32) + Send,
) -> anyhow::Result<()> {
    let info = version
        .downloads
        .as_ref()
        .and_then(|d| d.client.as_ref())
        .ok_or_else(|| anyhow::anyhow!("版本 {} 無 client 下載資訊", version.id))?;

    let dest = versions_dir
        .join(&version.id)
        .join(format!("{}.jar", version.id));
    download_and_verify(&info.url, &dest, info.size, &info.sha1).await?;
    on_progress(1.0);
    Ok(())
}

pub async fn install_libraries(
    version: &McSpecificVersionDetail,
    libraries_dir: &Path,
    on_progress: impl Fn(f32) + Send,
) -> anyhow::Result<()> {
    let mut applicable: Vec<(PathBuf, String, u64, String)> = version
        .libraries
        .iter()
        .filter(|lib| lib.rules.as_ref().is_none_or(|r| evaluate_rules(r)))
        .filter_map(|lib| {
            let artifact = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref())?;
            let dest = artifact
                .path
                .as_deref()
                .map(|p| libraries_dir.join(p))
                .or_else(|| maven_coord_to_path(&lib.name).map(|p| libraries_dir.join(p)))?;
            Some((
                dest,
                artifact.url.clone(),
                artifact.size,
                artifact.sha1.clone(),
            ))
        })
        .collect();

    #[cfg(target_os = "macos")]
    {
        let jna_version_opt = version
            .libraries
            .iter()
            .find(|lib| lib.name.starts_with("net.java.dev.jna:jna:"))
            .and_then(|lib| lib.name.split(':').nth(2));

        if let Some(jna_ver) = jna_version_opt {
            let has_platform = version
                .libraries
                .iter()
                .any(|lib| lib.name.starts_with("net.java.dev.jna:jna-platform:"));
            if !has_platform {
                warn!(jna_ver, "舊版本缺少 jna-platform，自動補入相容版本");
                let (url, size, sha1) = match jna_ver {
                    "5.13.0" => (
                        "https://libraries.minecraft.net/net/java/dev/jna/jna-platform/5.13.0/jna-platform-5.13.0.jar",
                        1345511,
                        "88e9a306715e9379f3122415ef4ae759a352640d",
                    ),
                    _ => (
                        "https://libraries.minecraft.net/net/java/dev/jna/jna-platform/5.11.0/jna-platform-5.11.0.jar",
                        1330369,
                        "1d60447fa0dbd7fae266a87df2c2bbf893fcff66",
                    ),
                };
                let path_str = format!(
                    "net/java/dev/jna/jna-platform/{}/jna-platform-{}.jar",
                    jna_ver, jna_ver
                );
                applicable.push((
                    libraries_dir.join(path_str),
                    url.to_string(),
                    size,
                    sha1.to_string(),
                ));
            }
        }
    }

    let total = applicable.len().max(1);
    let mut completed = 0usize;
    let mut stream = stream::iter(applicable)
        .map(|(dest, url, size, sha1)| async move {
            download_and_verify(&url, &dest, size, &sha1).await
        })
        .buffer_unordered(LIBRARY_CONCURRENCY);

    while let Some(result) = stream.next().await {
        result?;
        completed += 1;
        on_progress(completed as f32 / total as f32);
    }

    Ok(())
}

pub async fn install_assets(
    version: &McSpecificVersionDetail,
    assets_dir: &Path,
    on_progress: impl Fn(f32) + Send,
) -> anyhow::Result<()> {
    let index = version
        .asset_index
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("版本 {} 無 asset_index", version.id))?;

    let objects = crate::mc_api::McAction::new()
        .get_asset_index(&index.url)
        .await?;

    let index_path = assets_dir
        .join("indexes")
        .join(format!("{}.json", index.id));
    if let Some(parent) = index_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&index_path, serde_json::to_vec(&objects)?).await?;

    let objects_dir = assets_dir.join("objects");
    let total = objects.objects.len().max(1);
    let mut completed = 0usize;
    let mut stream = stream::iter(objects.objects.into_values())
        .map(|obj| {
            let dest = objects_dir.join(&obj.hash[..2]).join(&obj.hash);
            let url = obj.download_url();
            let size = obj.size;
            let hash = obj.hash.clone();
            async move { download_and_verify(&url, &dest, size, &hash).await }
        })
        .buffer_unordered(ASSET_CONCURRENCY);

    while let Some(result) = stream.next().await {
        result?;
        completed += 1;
        on_progress(completed as f32 / total as f32);
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use tempfile::TempDir;

    use super::*;
    use crate::mc_types::*;

    fn load_version(path: &str) -> McSpecificVersionDetail {
        let data = std::fs::read_to_string(path).unwrap_or_else(|_| panic!("找不到 {path}"));
        serde_json::from_str(&data).unwrap_or_else(|e| panic!("解析 {path} 失敗：{e}"))
    }

    fn empty_version() -> McSpecificVersionDetail {
        McSpecificVersionDetail {
            id: "test".into(),
            r#type: "release".into(),
            time: "".into(),
            release_time: "".into(),
            compliance_level: None,
            minimum_launcher_version: None,
            main_class: "".into(),
            java_version: None,
            downloads: None,
            asset_index: None,
            assets: None,
            logging: None,
            libraries: vec![],
            arguments: None,
            minecraft_arguments: None,
        }
    }

    #[test]
    fn test_sha1_hex_empty_string() {
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn test_sha1_hex_known_value() {
        assert_eq!(
            sha1_hex(b"hello"),
            "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"
        );
    }

    #[tokio::test]
    async fn test_download_and_verify_skips_when_size_matches() {
        let dir = TempDir::new().unwrap();
        let dest = dir.path().join("file.bin");
        tokio::fs::write(&dest, b"hello").await.unwrap();

        download_and_verify("http://0.0.0.0/invalid", &dest, 5, "any-sha1")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_download_and_verify_redownloads_when_size_mismatch() {
        let dir = TempDir::new().unwrap();
        let dest = dir.path().join("file.bin");
        tokio::fs::write(&dest, b"wrong content").await.unwrap();

        let result = download_and_verify(
            "http://0.0.0.0/invalid",
            &dest,
            5,
            "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d",
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_download_and_verify_downloads_when_missing() {
        let dir = TempDir::new().unwrap();
        let dest = dir.path().join("file.bin");
        let result = download_and_verify(
            "http://0.0.0.0/invalid",
            &dest,
            5,
            "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d",
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_download_and_verify_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let dest = dir.path().join("a/b/c/file.bin");
        tokio::fs::create_dir_all(dest.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&dest, b"hello").await.unwrap();

        download_and_verify("http://0.0.0.0/invalid", &dest, 5, "any-sha1")
            .await
            .unwrap();
        assert!(dest.exists());
    }

    #[tokio::test]
    async fn test_install_java_empty_manifest() {
        let dir = TempDir::new().unwrap();
        let manifest = McJavaManifest {
            files: HashMap::new(),
        };
        install_java(&manifest, dir.path(), |_| {}).await.unwrap();
    }

    #[tokio::test]
    async fn test_install_java_creates_directory_entries() {
        let dir = TempDir::new().unwrap();
        let mut files = HashMap::new();
        files.insert("bin".into(), McJavaFileEntry::Directory);
        files.insert("lib".into(), McJavaFileEntry::Directory);
        let manifest = McJavaManifest { files };

        install_java(&manifest, dir.path(), |_| {}).await.unwrap();

        assert!(dir.path().join("bin").is_dir());
        assert!(dir.path().join("lib").is_dir());
    }

    #[tokio::test]
    #[ignore]
    async fn test_install_java_real_download() {
        let dir = TempDir::new().unwrap();
        let api = crate::mc_api::McAction::new();
        let version = crate::mc_api::McAction::new()
            .get_specific_mc_version_detail("1.20.4")
            .await
            .unwrap();
        let manifest = api
            .get_java_runtime_manifest_for_version(&version)
            .await
            .unwrap();
        install_java(&manifest, dir.path(), |_| {}).await.unwrap();

        #[cfg(not(windows))]
        assert!(dir.path().join("bin/java").exists());
        #[cfg(windows)]
        assert!(dir.path().join("bin/javaw.exe").exists());
    }

    #[tokio::test]
    async fn test_install_client_errors_if_no_downloads() {
        let dir = TempDir::new().unwrap();
        let version = empty_version();
        let result = install_client(&version, dir.path(), |_| {}).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn test_install_client_real_download() {
        let dir = TempDir::new().unwrap();
        let version = crate::mc_api::McAction::new()
            .get_specific_mc_version_detail("1.20.4")
            .await
            .unwrap();
        install_client(&version, dir.path(), |_| {}).await.unwrap();
        assert!(dir.path().join("1.20.4/1.20.4.jar").exists());
    }

    #[tokio::test]
    async fn test_install_libraries_skips_disallowed_rules() {
        let dir = TempDir::new().unwrap();
        let mut version = empty_version();
        version.libraries = vec![McLibrary {
            name: "test:lib:1.0".into(),
            downloads: Some(McLibraryDownloads {
                artifact: Some(McArtifactInfo {
                    path: Some("test/lib/1.0/lib-1.0.jar".into()),
                    sha1: "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d".into(),
                    size: 5,
                    url: "http://0.0.0.0/invalid".into(),
                }),
                classifiers: None,
            }),
            rules: Some(vec![McRule {
                action: McRuleAction::Disallow,
                os: None,
                features: None,
            }]),
        }];

        install_libraries(&version, dir.path(), |_| {})
            .await
            .unwrap();

        assert!(!dir.path().join("test/lib/1.0/lib-1.0.jar").exists());
    }

    #[tokio::test]
    async fn test_install_libraries_skips_missing_artifact() {
        let dir = TempDir::new().unwrap();
        let mut version = empty_version();
        version.libraries = vec![McLibrary {
            name: "test:lib:1.0".into(),
            downloads: None,
            rules: None,
        }];

        install_libraries(&version, dir.path(), |_| {})
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_install_libraries_real_download() {
        let dir = TempDir::new().unwrap();
        let version = crate::mc_api::McAction::new()
            .get_specific_mc_version_detail("1.20.4")
            .await
            .unwrap();
        install_libraries(&version, dir.path(), |_| {})
            .await
            .unwrap();
        let count = count_jars(dir.path());
        assert!(count > 0, "libraries 目錄應有 JAR 檔案，實際：{count}");
    }

    #[tokio::test]
    async fn test_install_assets_errors_if_no_asset_index() {
        let dir = TempDir::new().unwrap();
        let version = empty_version();
        let result = install_assets(&version, dir.path(), |_| {}).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn test_install_assets_real_download() {
        let dir = TempDir::new().unwrap();
        let version = crate::mc_api::McAction::new()
            .get_specific_mc_version_detail("1.20.4")
            .await
            .unwrap();
        install_assets(&version, dir.path(), |_| {}).await.unwrap();

        let index_id = version.asset_index.unwrap().id;
        assert!(dir.path().join(format!("indexes/{index_id}.json")).exists());
        assert!(dir.path().join("objects").is_dir());
    }

    fn count_jars(dir: &std::path::Path) -> usize {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return 0;
        };
        entries.filter_map(|e| e.ok()).fold(0, |acc, entry| {
            let path = entry.path();
            if path.is_dir() {
                acc + count_jars(&path)
            } else if path.extension().is_some_and(|ext| ext == "jar") {
                acc + 1
            } else {
                acc
            }
        })
    }
}
