use oauth2::reqwest;

use crate::mc_types::{McLatestVersion, McVersion, McVersionInfo};

const NEW_MC_SERVER: &str = "https://api.minecraftservices.com";
const OLD_MC_SERVER: &str = "https://api.mojang.com";

pub async fn get_player_uuid(username: &str) -> anyhow::Result<String> {
    let url = format!("{}/minecraft/profile/lookup/name/{}", NEW_MC_SERVER, username);
    let response = reqwest::get(&url).await?;
    if response.status().is_success() {
        let json: serde_json::Value = response.json().await?;
        if let Some(uuid) = json.get("id").and_then(|v| v.as_str()) {
            return Ok(uuid.to_string());
        }
    }
    Err(anyhow::anyhow!("Failed to get UUID for username: {}", username))
}

pub async fn get_player_name(uuid: &str) -> anyhow::Result<String> {
    let url = format!("{}/minecraft/profile/lookup/{}", NEW_MC_SERVER, uuid);
    let response = reqwest::get(&url).await?;
    if response.status().is_success() {
        let json: serde_json::Value = response.json().await?;
        if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
            return Ok(name.to_string());
        }
    }
    Err(anyhow::anyhow!("Failed to get username for UUID: {}", uuid))
}

async fn get_mc_manifest() -> anyhow::Result<McVersionInfo> {
    let url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
    let response = reqwest::get(url).await?;
    if response.status().is_success() {
        let mc_version_info: McVersionInfo = response.json().await?;
        return Ok(mc_version_info);
    }
    Err(anyhow::anyhow!("Failed to get Minecraft version manifest"))
}

pub async fn get_all_mc_versions() -> anyhow::Result<Vec<McVersion>> {
    let manifest = get_mc_manifest().await?;
    Ok(manifest.versions)
}

pub async fn get_latest_mc_version() -> anyhow::Result<McLatestVersion> {
    let manifest = get_mc_manifest().await?;
    Ok(manifest.latest)
}

mod test {
    use super::*;
    #[tokio::test]
    async fn test_get_player_uuid() {
        let username = "derrick921213";
        match get_player_uuid(username).await {
            Ok(uuid) => println!("UUID for {}: {}", username, uuid),
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    #[tokio::test]
    async fn test_get_player_name() {
        let uuid = "derrick921213";
        match get_player_name(uuid).await {
            Ok(name) => println!("Name for {}: {}", uuid, name),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_get_latest_mc_version() {
        match get_latest_mc_version().await {
            Ok(latest_version) => println!("Latest Minecraft version: {:?}", latest_version),
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_get_all_mc_versions() {
        match get_all_mc_versions().await {
            Ok(versions) => {
                dbg!(&versions);
                println!("Total Minecraft versions: {}", versions.len())
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_get_specific_mc_version() {
        let target_version = "26.1.2";
        match get_all_mc_versions().await {
            Ok(versions) => {
                if let Some(version) = versions.into_iter().find(|v| v.id == target_version) {
                    println!("Found version {}: {:?}", target_version, version);
                } else {
                    println!("Version {} not found", target_version);
                }
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }
}