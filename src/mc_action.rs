use oauth2::reqwest;

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
}