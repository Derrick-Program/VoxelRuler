// slint::include_modules!();
// fn main() -> anyhow::Result<()>{
//     // slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/ui/translations"));
//     let app = MainApp::new()?;
//     slint::select_bundled_translation("zh_TW").unwrap();
//     // let app_weak = app.as_weak();
//     app.run()?;
//     Ok(())
// }

use minecraft_msa_auth::MinecraftAuthorizationFlow;
use oauth2::basic::BasicClient;
use oauth2::reqwest;
use oauth2::{
    AuthType, AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl,
    Scope, TokenResponse, TokenUrl,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use url::Url;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client_id = std::env::args()
        .nth(1)
        .expect("client_id as first argument");
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
            Ok(t) => println!("Success: {:?}", t),
            Err(e) => {
                eprintln!("詳細錯誤: {:?}", e);
            }
        }
        break;
    }

    Ok(())
}
