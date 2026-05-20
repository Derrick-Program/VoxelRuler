#![allow(unused)]
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use std::marker::PhantomData;

use crate::mc_types::{
    McAssetObjects, McJavaAll, McJavaManifest, McLatestVersion, McSpecificVersionDetail,
    McVersion, McVersionInfo,
};
use futures_util::{StreamExt, stream};

const NEW_MC_SERVER: &str = "https://api.minecraftservices.com";
const JAVA_RUNTIME_ALL_URL: &str =
    "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

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
        let json: serde_json::Value = self
            .client
            .get(&url)
            .send()
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
        let json: serde_json::Value = self
            .client
            .get(&url)
            .send()
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
        Ok(self
            .client
            .get(url)
            .send()
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
        let datail: serde_json::Value = self
            .client
            .get(&version.url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let detail: crate::mc_types::McSpecificVersionDetail = serde_json::from_value(datail)?;
        let json_data = serde_json::to_vec_pretty(&detail)?;
        tokio::fs::create_dir_all("data").await?; //TODO: runtime 需要放到系統特定資料夾
        tokio::fs::write(format!("data/{}.json", version_id), json_data).await?;

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
        let runtimes = self.get_java_runtimes().await?;
        let os_arch = crate::mc_parser::get_mojang_os_arch();
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
