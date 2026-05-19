use std::sync::LazyLock;
use dashmap::DashMap;
use crate::view::open_view;

mod mc_action;
mod mc_token;
mod mc_types;
mod view;

static GLOBAL_CACHE: LazyLock<DashMap<String, String>> = LazyLock::new(|| DashMap::new());

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _pdir = directories::ProjectDirs::from("com", "Duacodie", "VoxelRuler").unwrap();
    let token_init_attempt = match mc_token::SessionData::load_session() {
        Ok(Some(s)) => {
            if *s.mc_token_expires_at() >= chrono::Utc::now().timestamp() {
                Some(s.minecraft_access_token().clone())
            } else {
                match mc_token::refresh_minecraft_token(s.microsoft_refresh_token()).await {
                    Ok(new_token) => {
                        Some(new_token)
                    }
                    Err(_) => {
                        None
                    }
                }
            }
        }
        _ => {
            None
        }
    };
    if GLOBAL_CACHE.get("mc_ac_key").is_none() {
        if let Some(token) = token_init_attempt {
            GLOBAL_CACHE.insert("mc_ac_key".into(), token);
        }
    }
    open_view().await?;
    Ok(())
}
