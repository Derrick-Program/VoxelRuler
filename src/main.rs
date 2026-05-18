mod mc_action;
mod mc_token;
mod mc_types;
mod view;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pdir = directories::ProjectDirs::from("com", "Duacodie", "VoxelRuler").unwrap();
    dbg!(pdir);
    let token: String = loop {
        let session = match mc_token::SessionData::load_session() {
            Ok(s) => s,
            Err(e) => {
                None
            }
        };

        match session {
            Some(s) if *s.mc_token_expires_at() >= chrono::Utc::now().timestamp() => {
                break s.minecraft_access_token().clone(); 
            }
            Some(s) => {
                match mc_token::refresh_minecraft_token(s.microsoft_refresh_token()).await {
                    Ok(new_token) => {
                        break new_token;
                    }
                    Err(_) => {
                        match mc_token::set_token_in_native_store().await {
                            Ok(new_token) => break new_token,
                            Err(_) => {
                                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            }
                        }
                    }
                }
            }
            None => {
                match mc_token::set_token_in_native_store().await {
                    Ok(new_token) => break new_token, 
                    Err(_) => {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
            }
        }
    };

    println!("成功取得有效 Token，準備進入 VoxelRuler!");
    // mc_action::launch_game(&token).await?;

    Ok(())
}
