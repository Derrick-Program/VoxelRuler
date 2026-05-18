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

pub async fn set_token_in_native_store() -> anyhow::Result<String> {
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

    println!("Open this URL in your browser:\n{}\n", authorize_url);

    let http_client = oauth2::reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let listener = TcpListener::bind("127.0.0.1:8114").await?;

    loop {
        let (stream, _) = listener.accept().await?;
        stream.readable().await?;
        let mut stream = BufReader::new(stream);

        let code;
        let state;
        {
            let mut request_line = String::new();
            stream.read_line(&mut request_line).await?;
            let redirect_url = request_line.split_whitespace().nth(1).unwrap();
            let url = Url::parse(&("http://localhost".to_string() + redirect_url))?;

            let (_, value) = url.query_pairs().find(|(k, _)| k == "code").unwrap();
            code = AuthorizationCode::new(value.into_owned());

            let (_, value) = url.query_pairs().find(|(k, _)| k == "state").unwrap();
            state = CsrfToken::new(value.into_owned());
        }

        let message = "Go back to your terminal :)";
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{}",
            message.len(),
            message
        );
        stream.get_mut().write_all(response.as_bytes()).await?;

        println!("MS code:\n{}\n", code.secret());
        println!(
            "MS state:\n{} (expected `{}`)\n",
            state.secret(),
            csrf_state.secret()
        );

        let token = client
            .exchange_code(code)
            .set_pkce_verifier(pkce_code_verifier)
            .request_async(&http_client)
            .await?;

        println!("microsoft token:\n{:?}\n", token);

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
                return Ok(token_string);
            }
            Err(e) => anyhow::bail!("Failed to exchange Microsoft token for Minecraft token: {:?}", e),
        }
        break;
    }
    Ok(Default::default())
}
