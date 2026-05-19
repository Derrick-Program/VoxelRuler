#![allow(unused)]
use getset::{CopyGetters, Getters};
use keyring::use_native_store;
use keyring_core::Entry;
use minecraft_msa_auth::MinecraftAuthorizationFlow;
use oauth2::{
    AuthType, AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl,
    RefreshToken, Scope, TokenResponse, TokenUrl, basic::BasicClient, reqwest,
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use url::Url;

use crate::GLOBAL_CACHE;

const SERVICE_NAME: &str = "VoxelRuler";
const ACCOUNT_KEY: &str = "user_session_data";

#[derive(Debug, Serialize, Deserialize, Getters, Clone, CopyGetters)]
pub struct SessionData {
    #[getset(get = "pub")]
    microsoft_refresh_token: String,
    #[getset(get = "pub")]
    minecraft_access_token: String,
    #[getset(get = "pub")]
    mc_token_expires_at: i64,
}

impl SessionData {
    fn save_session(&self) -> anyhow::Result<()> {
        use_native_store(false)?;
        let keyring = Entry::new(SERVICE_NAME, ACCOUNT_KEY)?;
        let session_json = serde_json::to_string(self)?;
        keyring.set_password(&session_json)?;
        Ok(())
    }

    pub fn load_session() -> anyhow::Result<Option<SessionData>> {
        use_native_store(false)?;
        let keyring = Entry::new(SERVICE_NAME, ACCOUNT_KEY)?;
        match keyring.get_password() {
            Ok(session_json) => {
                let session: SessionData = serde_json::from_str(&session_json)?;
                Ok(Some(session))
            }
            Err(keyring_core::error::Error::NoEntry) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Failed to load session: {:?}", e)),
        }
    }
}

pub async fn refresh_minecraft_token(saved_refresh_token: &str) -> anyhow::Result<String> {
    let client_id = String::from("ebd68e7a-2003-487d-bfa6-14807af049c9");
    let client = BasicClient::new(ClientId::new(client_id.to_string()))
        .set_auth_uri(AuthUrl::new(
            "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize".to_string(),
        )?)
        .set_token_uri(TokenUrl::new(
            "https://login.microsoftonline.com/consumers/oauth2/v2.0/token".to_string(),
        )?)
        .set_auth_type(AuthType::RequestBody);
    let http_client = oauth2::reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let refresh_token = RefreshToken::new(saved_refresh_token.to_string());
    let token_response = client
        .exchange_refresh_token(&refresh_token)
        .request_async(&http_client)
        .await?;
    let new_ms_access_token = token_response.access_token().secret();
    let new_ms_refresh_token = match token_response.refresh_token() {
        Some(rt) => rt.secret().clone(),
        None => saved_refresh_token.to_string(),
    };

    let mc_flow = MinecraftAuthorizationFlow::new(http_client.clone());
    let mc_token = mc_flow
        .exchange_microsoft_token(new_ms_access_token)
        .await?;

    let mc_token_string = mc_token.access_token().as_ref().to_string();
    let mc_token_expires_at = mc_token.expires_in() as i64 + chrono::Utc::now().timestamp();

    let user_session = SessionData {
        microsoft_refresh_token: new_ms_refresh_token.clone(),
        minecraft_access_token: mc_token_string.clone(),
        mc_token_expires_at,
    };
    user_session.save_session()?;
    Ok(mc_token_string)
}

pub async fn set_token_in_native_store<F>(on_url_generated: F) -> anyhow::Result<String>
where
    F: FnOnce(String) + Send + 'static,
{
    let client_id = String::from("ebd68e7a-2003-487d-bfa6-14807af049c9");
    let client = BasicClient::new(ClientId::new(client_id))
        .set_auth_uri(AuthUrl::new(
            "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize".to_string(),
        )?)
        .set_token_uri(TokenUrl::new(
            "https://login.microsoftonline.com/consumers/oauth2/v2.0/token".to_string(),
        )?)
        .set_auth_type(AuthType::RequestBody)
        .set_redirect_uri(RedirectUrl::new(
            "http://127.0.0.1:8114/redirect".to_string(),
        )?);

    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();

    let (authorize_url, csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("XboxLive.signin offline_access".to_string()))
        .set_pkce_challenge(pkce_code_challenge)
        .url();
    let url_string = authorize_url.to_string();
    on_url_generated(url_string.clone());
    let _ = open::that(url_string.as_str());
    let http_client = oauth2::reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let listener = TcpListener::bind("127.0.0.1:8114").await?;
    let (stream, _) = listener.accept().await?;
    stream.readable().await?;
    let mut stream = BufReader::new(stream);
    let code;
    let state;
    {
        let mut request_line = String::new();
        stream.read_line(&mut request_line).await?;
        let redirect_url = request_line
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("Invalid HTTP request line"))?;
        let url = Url::parse(&("http://localhost".to_string() + redirect_url))?;
        let (_, value) = url
            .query_pairs()
            .find(|(k, _)| k == "code")
            .ok_or_else(|| anyhow::anyhow!("Missing code parameter"))?;
        code = AuthorizationCode::new(value.into_owned());
        let (_, value) = url
            .query_pairs()
            .find(|(k, _)| k == "state")
            .ok_or_else(|| anyhow::anyhow!("Missing state parameter"))?;
        state = CsrfToken::new(value.into_owned());
    }
    let icon_base64 = "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";

    let html_message = format!(
        r#"
    <!DOCTYPE html>
    <html>
    <head>
        <meta charset="utf-8">
        <title>Verification successful</title>
        <link rel="icon" type="image/png" href="data:image/png;base64,{}" />
        <style>
            body {{ font-family: sans-serif; text-align: center; padding-top: 50px; background-color: #f4f4f9; color: #333; }}
            h1 {{ color: #2ecc71; }}
            p {{ font-size: 18px; }}
        </style>
        <script>
            window.onload = function() {{
                setTimeout(function() {{
                    window.close();
                }}, 1500);
            }};
        </script>
    </head>
    <body>
        <h1>✓ Verification successful!</h1>
        <p>This webpage is closing automatically for you and returning you to the VoxelRuler launcher...</p>
        <p style="font-size: 14px; color: #888;">(If the webpage does not close automatically, you can close it manually)</p>
    </body>
    </html>
    "#,
        icon_base64
    );
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
        html_message.len(),
        html_message
    );
    stream.get_mut().write_all(response.as_bytes()).await?;
    if state.secret() != csrf_state.secret() {
        anyhow::bail!("CSRF token mismatch! Security check failed.");
    }

    let token = client
        .exchange_code(code)
        .set_pkce_verifier(pkce_code_verifier)
        .request_async(&http_client)
        .await?;

    let mc_flow = MinecraftAuthorizationFlow::new(http_client.clone());
    let mc_token = mc_flow
        .exchange_microsoft_token(token.access_token().secret())
        .await;

    match mc_token {
        Ok(t) => {
            let token_string = t.access_token().as_ref().to_string();
            let expires_at = t.expires_in() as i64 + chrono::Utc::now().timestamp();
            let ms_refresh_token = token
                .refresh_token()
                .map(|rt| rt.secret().clone())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Microsoft did not return a refresh_token, please check if the scope includes offline_access"
                    )
                })?;
            let user_session = SessionData {
                microsoft_refresh_token: ms_refresh_token,
                minecraft_access_token: token_string.clone(),
                mc_token_expires_at: expires_at,
            };
            user_session.save_session()?;
            GLOBAL_CACHE.insert("mc_ac_key".into(), token_string.clone());
            Ok(token_string)
        }
        Err(e) => anyhow::bail!(
            "Failed to exchange Microsoft token for Minecraft token: {:?}",
            e
        ),
    }
}
