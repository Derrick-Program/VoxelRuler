#![allow(unused)]
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use std::marker::PhantomData;
use tracing::warn;

use crate::mc_types::{
    McAssetObjects, McJavaAll, McJavaManifest, McLatestVersion, McSpecificVersionDetail, McVersion,
    McVersionInfo,
};
use futures_util::{StreamExt, stream};

const NEW_MC_SERVER: &str = "https://api.minecraftservices.com";
const JAVA_RUNTIME_ALL_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

const API_MAX_RETRIES: u32 = 4;
const API_RETRY_BASE_MS: u64 = 1000;

async fn retry_get(client: &reqwest::Client, url: &str) -> anyhow::Result<reqwest::Response> {
    use anyhow::Context;
    let mut last_err: anyhow::Error = anyhow::anyhow!("尚未嘗試");
    let mut delay_ms = 0;

    for attempt in 0..API_MAX_RETRIES {
        if delay_ms > 0 {
            warn!(
                attempt,
                max = API_MAX_RETRIES - 1,
                delay_ms,
                url,
                "API 重試中，暫停等待..."
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
        }

        match client.get(url).send().await {
            Ok(resp) => {
                if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    last_err = anyhow::anyhow!("請求過於頻繁 (429 Too Many Requests)");
                    
                    delay_ms = API_RETRY_BASE_MS * (1u64 << attempt);
                    if let Some(retry_after) = resp.headers().get(reqwest::header::RETRY_AFTER) {
                        if let Ok(retry_str) = retry_after.to_str() {
                            if let Ok(secs) = retry_str.parse::<u64>() {
                                delay_ms = secs * 1000;
                            }
                        }
                    }
                    continue;
                }
                
                if resp.status().is_server_error() {
                    last_err = anyhow::anyhow!("伺服器錯誤 ({})", resp.status());
                    delay_ms = API_RETRY_BASE_MS * (1u64 << attempt);
                    continue;
                }

                return Ok(resp);
            }
            Err(e) => {
                last_err = e.into();
                delay_ms = API_RETRY_BASE_MS * (1u64 << attempt);
            }
        }
    }
    Err(last_err).with_context(|| format!("API 請求失敗（重試 {} 次）：{}", API_MAX_RETRIES, url))
}

pub struct Unauthenticated;
pub struct Authenticated;

pub struct McAction<S> {
    client: reqwest::Client,
    _state: PhantomData<S>,
}

impl Default for McAction<Unauthenticated> {
    fn default() -> Self {
        Self::new()
    }
}

impl McAction<Unauthenticated> {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .user_agent(format!("VoxelRulerLauncher/{} (https://github.com/Derrick-Program/VoxelRuler)", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("Failed to build client"),
            _state: PhantomData,
        }
    }

    pub fn authenticate(self, token: &str) -> McAction<Authenticated> {
        let bearer = format!("Bearer {}", token);
        let mut header_value =
            HeaderValue::from_str(&bearer).expect("Failed to create header value");
        header_value.set_sensitive(true);
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, header_value);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build auth client");

        McAction {
            client,
            _state: PhantomData,
        }
    }

    // === Public APIs (no token needed) ===

    pub async fn get_player_uuid(&self, username: &str) -> anyhow::Result<String> {
        let url = format!(
            "{}/minecraft/profile/lookup/name/{}",
            NEW_MC_SERVER, username
        );
        let json: serde_json::Value = retry_get(&self.client, &url)
            .await?
            .error_for_status()?
            .json()
            .await?;
        json.get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Failed to get UUID for username: {}", username))
    }

    pub async fn get_player_name(&self, uuid: &str) -> anyhow::Result<String> {
        let url = format!("{}/minecraft/profile/lookup/{}", NEW_MC_SERVER, uuid);
        let json: serde_json::Value = retry_get(&self.client, &url)
            .await?
            .error_for_status()?
            .json()
            .await?;
        json.get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Failed to get username for UUID: {}", uuid))
    }

    async fn get_mc_manifest(&self) -> anyhow::Result<McVersionInfo> {
        let url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
        Ok(retry_get(&self.client, url)
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    pub async fn get_specific_mc_version(&self, version_id: &str) -> anyhow::Result<McVersion> {
        self.get_mc_manifest()
            .await?
            .versions
            .into_iter()
            .find(|v| v.id == version_id)
            .ok_or_else(|| anyhow::anyhow!("Minecraft version {} not found", version_id))
    }

    pub async fn get_all_mc_versions(&self) -> anyhow::Result<Vec<McVersion>> {
        Ok(self.get_mc_manifest().await?.versions)
    }

    pub async fn get_latest_snapshot_mc_version(&self) -> anyhow::Result<McVersion> {
        let manifest = self.get_mc_manifest().await?;
        let snapshot_id = manifest.latest.snapshot;
        manifest
            .versions
            .into_iter()
            .find(|v| v.id == snapshot_id)
            .ok_or_else(|| anyhow::anyhow!("Latest snapshot version not found"))
    }

    pub async fn get_latest_release_mc_version(&self) -> anyhow::Result<McVersion> {
        let manifest = self.get_mc_manifest().await?;
        let release_id = manifest.latest.release;
        manifest
            .versions
            .into_iter()
            .find(|v| v.id == release_id)
            .ok_or_else(|| anyhow::anyhow!("Latest release version not found"))
    }
    pub async fn get_specific_mc_version_detail(
        &self,
        version_id: &str,
    ) -> anyhow::Result<crate::mc_types::McSpecificVersionDetail> {
        let version = self.get_specific_mc_version(version_id).await?;
        let datail: serde_json::Value = retry_get(&self.client, &version.url)
            .await?
            .error_for_status()?
            .json()
            .await?;
        let detail: crate::mc_types::McSpecificVersionDetail = serde_json::from_value(datail)?;
        Ok(detail)
    }

    pub async fn get_java_runtimes(&self) -> anyhow::Result<McJavaAll> {
        Ok(self
            .client
            .get(JAVA_RUNTIME_ALL_URL)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    pub async fn get_java_runtime_manifest(
        &self,
        component: &str,
    ) -> anyhow::Result<McJavaManifest> {
        self.get_java_runtime_manifest_for_platform(
            component,
            crate::mc_parser::get_mojang_os_arch(),
        )
        .await
    }

    /// 指定 Mojang 平台字串（如 `mac-os` / `mac-os-arm64`）取得 Java runtime manifest。
    /// Apple Silicon 跑 1.18.x 以前的版本時需強制抓 x64（`mac-os`）經 Rosetta 執行。
    pub async fn get_java_runtime_manifest_for_platform(
        &self,
        component: &str,
        os_arch: &str,
    ) -> anyhow::Result<McJavaManifest> {
        let runtimes = self.get_java_runtimes().await?;
        let manifest_url = runtimes
            .get(os_arch)
            .and_then(|by_component| by_component.get(component))
            .and_then(|entries| entries.first())
            .map(|entry| entry.manifest.url.clone())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Java runtime '{}' not found for platform '{}'",
                    component,
                    os_arch
                )
            })?;

        Ok(self
            .client
            .get(&manifest_url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    pub async fn get_java_runtime_manifest_for_version(
        &self,
        version: &McSpecificVersionDetail,
    ) -> anyhow::Result<McJavaManifest> {
        let component = version
            .java_version
            .as_ref()
            .map(|j| j.component.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Version '{}' does not specify a javaVersion component",
                    version.id
                )
            })?;
        self.get_java_runtime_manifest(component).await
    }

    pub async fn get_asset_index(&self, url: &str) -> anyhow::Result<McAssetObjects> {
        Ok(self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    pub async fn download_all_release_details(&self) -> anyhow::Result<()> {
        let versions = self.get_all_mc_versions().await?;
        let releases: Vec<_> = versions
            .into_iter()
            .filter(|v| v.r#type == "release")
            .collect();
        let mut stream = stream::iter(releases)
            .map(|v| async move {
                (
                    v.id.clone(),
                    self.get_specific_mc_version_detail(&v.id).await,
                )
            })
            .buffer_unordered(5);
        while let Some((id, result)) = stream.next().await {
            match result {
                Ok(_) => println!("Successfully downloaded: {}", id),
                Err(e) => eprintln!("Failed to download {}: {}", id, e),
            }
        }

        Ok(())
    }
}

impl McAction<Authenticated> {
    // === Authenticated APIs (Bearer token required) ===

    pub async fn get_user_profile(&self) -> anyhow::Result<crate::mc_types::McProfile> {
        let url = format!("{}/minecraft/profile", NEW_MC_SERVER);
        Ok(self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .inspect_err(|e| println!("{:#?}", e))?)
    }

    pub async fn upload_skin_from_url(&self, url: &str, variant: &str) -> anyhow::Result<()> {
        // Mojang rejects arbitrary external URLs; download the image first then upload as file
        let img_bytes = reqwest::get(url)
            .await
            .map_err(|e| anyhow::anyhow!("下載皮膚失敗：{e}"))?
            .error_for_status()
            .map_err(|e| anyhow::anyhow!("下載皮膚失敗：{e}"))?
            .bytes()
            .await
            .map_err(|e| anyhow::anyhow!("讀取皮膚資料失敗：{e}"))?;

        let endpoint = format!("{}/minecraft/profile/skins", NEW_MC_SERVER);
        let part = reqwest::multipart::Part::bytes(img_bytes.to_vec())
            .file_name("skin.png")
            .mime_str("image/png")?;
        let form = reqwest::multipart::Form::new()
            .text("variant", variant.to_string())
            .part("file", part);
        let resp = self.client
            .post(&endpoint)
            .multipart(form)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("HTTP {} — {}", status, body_text);
        }
        Ok(())
    }

    pub async fn upload_skin_from_file(
        &self,
        path: &std::path::Path,
        variant: &str,
    ) -> anyhow::Result<()> {
        let endpoint = format!("{}/minecraft/profile/skins", NEW_MC_SERVER);
        let file_bytes = tokio::fs::read(path).await?;
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("skin.png")
            .to_string();
        let part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(file_name)
            .mime_str("image/png")?;
        let form = reqwest::multipart::Form::new()
            .text("variant", variant.to_string())
            .part("file", part);
        let resp = self.client
            .post(&endpoint)
            .multipart(form)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("HTTP {} — {}", status, body_text);
        }
        Ok(())
    }

    pub async fn check_game_ownership(&self) -> anyhow::Result<bool> {
        let url = format!("{}/entitlements/mcstore", NEW_MC_SERVER);
        let entitlements: serde_json::Value = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let owns_games = entitlements
            .get("items")
            .and_then(|items| items.as_array())
            .map(|arr| {
                arr.iter().any(|item| {
                    item.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s == "game_minecraft")
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        Ok(owns_games)
    }

    pub async fn set_active_cape(&self, cape_id: &str) -> anyhow::Result<()> {
        let endpoint = format!("{}/minecraft/profile/capes/active", NEW_MC_SERVER);
        let body = serde_json::json!({ "capeId": cape_id });
        let resp = self.client.put(&endpoint).json(&body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("HTTP {} — {}", status, body_text);
        }
        Ok(())
    }

    pub async fn hide_cape(&self) -> anyhow::Result<()> {
        let endpoint = format!("{}/minecraft/profile/capes/active", NEW_MC_SERVER);
        let resp = self.client.delete(&endpoint).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("HTTP {} — {}", status, body_text);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_get_player_uuid() {
        let mc = McAction::new();
        let username = "derrick921213";
        match mc.get_player_uuid(username).await {
            Ok(uuid) => println!("UUID for {}: {}", username, uuid),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_get_player_name() {
        let mc = McAction::new();
        let uuid = "derrick921213";
        match mc.get_player_name(uuid).await {
            Ok(name) => println!("Name for {}: {}", uuid, name),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_get_latest_release_mc_version() {
        let mc = McAction::new();
        match mc.get_latest_release_mc_version().await {
            Ok(v) => println!("Latest release: {:?}", v),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_get_all_mc_versions() {
        let mc = McAction::new();
        match mc.get_all_mc_versions().await {
            Ok(versions) => {
                dbg!(&versions);
                println!("Total Minecraft versions: {}", versions.len())
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_get_specific_mc_version() {
        let mc = McAction::new();
        match mc.get_specific_mc_version("1.20.4").await {
            Ok(version) => println!("Minecraft version 1.20.4 info: {:?}", version),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_get_user_profile() {
        let session = match crate::mc_token::SessionData::load_session() {
            Ok(Some(s)) => s,
            Ok(None) => {
                eprintln!("跳過：本機 session 為空，請先登入");
                return;
            }
            Err(_) => {
                eprintln!("跳過：找不到本機 session，請先登入");
                return;
            }
        };

        let mc = McAction::new().authenticate(session.minecraft_access_token());
        match mc.get_user_profile().await {
            Ok(profile) => println!("Profile: {:#?}", profile),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_get_specific_mc_version_detail() {
        let mc = McAction::new();
        match mc.get_specific_mc_version_detail("1.20.4").await {
            Ok(detail) => {
                println!("Detail for version 1.20.4: {:#?}", detail)
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_all_release_version_details() {
        let mc = McAction::new();
        match mc.download_all_release_details().await {
            Ok(_) => println!("Successfully downloaded all release details"),
            Err(e) => eprintln!("Error: {}", e),
        }
    }
}
