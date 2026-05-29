#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
#![allow(unused)]
use crate::view::open_view;
use dashmap::DashMap;
use std::sync::LazyLock;
use tracing::{debug, info};
use url::Url;

mod mc_api;
mod mc_install;
mod mc_instance;
mod mc_parser;
mod mc_paths;
mod mc_token;
mod mc_types;
#[cfg(target_os = "macos")]
mod url_handler;
mod view;

#[derive(Debug)]
pub struct AuthArgs {
    pub code: String,
    pub state: Option<String>,
}

#[derive(Debug)]
pub enum DeepLinkAction {
    MicrosoftAuth(AuthArgs),
    Unknown,
}

impl DeepLinkAction {
    pub fn parse_string(url_str: &str) -> Self {
        let Ok(parsed_url) = Url::parse(url_str) else {
            return Self::Unknown;
        };

        if parsed_url.scheme() != "voxelruler" {
            return Self::Unknown;
        }

        match parsed_url.host_str() {
            Some("auth") => {
                let mut auth_code = None;
                let mut auth_state = None;
                for (key, value) in parsed_url.query_pairs() {
                    match key.as_ref() {
                        "code" => auth_code = Some(value.into_owned()),
                        "state" => auth_state = Some(value.into_owned()),
                        _ => {}
                    }
                }
                if let Some(code) = auth_code {
                    Self::MicrosoftAuth(AuthArgs {
                        code,
                        state: auth_state,
                    })
                } else {
                    Self::Unknown
                }
            }
            _ => {
                debug!("解析到未知的 voxelruler URL，host: {:#?}", parsed_url);
                Self::Unknown
            }
        }
    }
}

static GLOBAL_CACHE: LazyLock<DashMap<String, String>> = LazyLock::new(DashMap::new);
static PROJECT_DIR: LazyLock<Option<directories::ProjectDirs>> =
    LazyLock::new(|| directories::ProjectDirs::from("com", "Duacodie", "VoxelRuler"));

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Deep Link 處理 ────────────────────────────────────────────────────
    //
    // macOS：URL scheme 透過 Apple Events 傳遞，不走 argv。
    //        在 Slint event loop 啟動前向 NSAppleEventManager 註冊 handler，
    //        收到 URL 後透過 channel 傳至此 async task。
    //
    // Windows / Linux：cargo-packager 會將 URL 以 argv[1] 傳入，
    //                  直接從 args 解析即可。
    #[cfg(target_os = "macos")]
    let mut deep_link_rx = url_handler::register();

    #[cfg(target_os = "macos")]
    tokio::spawn(async move {
        while let Some(url) = deep_link_rx.recv().await {
            debug!(url = %url, "deep link channel 收到 URL");
            match DeepLinkAction::parse_string(&url) {
                DeepLinkAction::MicrosoftAuth(auth_data) => {
                    debug!(
                        code = %auth_data.code,
                        state = ?auth_data.state,
                        "收到 Microsoft OAuth deep link（Apple Events）"
                    );
                    // TODO M2：呼叫 token exchange，更新 GLOBAL_CACHE
                }
                DeepLinkAction::Unknown => {
                    debug!("收到未知的 VoxelRuler deep link，略過");
                }
            }
        }
    });

    // Windows / Linux：URL scheme 以 argv[1] 傳入
    #[cfg(not(target_os = "macos"))]
    {
        let args: Vec<String> = std::env::args().collect();
        if args.len() > 1 {
            debug!("收到啟動參數：{:#?}", args);
            match DeepLinkAction::parse_string(&args[1]) {
                DeepLinkAction::MicrosoftAuth(auth_data) => {
                    debug!(
                        code = %auth_data.code,
                        state = ?auth_data.state,
                        "收到 Microsoft OAuth deep link（argv）"
                    );
                    // TODO M2：呼叫 token exchange，更新 GLOBAL_CACHE
                }
                DeepLinkAction::Unknown => {
                    debug!("收到未知的 VoxelRuler 指令");
                }
            }
        }
    }
    // ── Logging 模式 ──────────────────────────────────────────
    //
    //  【預設（不設 RUST_LOG）】
    //    debug build   → voxelruler=debug  只看自己 app 的 debug+
    //    release build → voxelruler=info   只看自己 app 的 info+
    //
    //  【看所有 crate】
    //    RUST_LOG=debug / RUST_LOG=info
    //
    //  【混合模式】
    //    RUST_LOG=voxelruler=debug,reqwest=warn,tokio=info
    //
    //  Log 檔案位置（release）：
    //    macOS   → ~/Library/Logs/VoxelRuler/voxelruler.log
    //    Windows → %APPDATA%\Duacodie\VoxelRuler\logs\voxelruler.log
    //    Linux   → ~/.local/share/VoxelRuler/voxelruler.log
    //    即時查看：tail -f <上述路徑>
    // ──────────────────────────────────────────────────────────
    use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

    // release build：寫入 log 檔案
    #[cfg(not(debug_assertions))]
    let _file_guard = {
        let log_dir = PROJECT_DIR
            .as_ref()
            .map(|d| d.data_dir().to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        std::fs::create_dir_all(&log_dir).ok();
        let file_appender = tracing_appender::rolling::never(&log_dir, "voxelruler.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        let filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("voxelruler=info"));
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().with_writer(non_blocking))
            .init();
        guard
    };

    // debug build：輸出到 terminal + tokio-console
    #[cfg(debug_assertions)]
    {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("voxelruler=debug"));
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer())
            .with(console_subscriber::spawn())
            .init();
    }
    let token_init_attempt = match mc_token::SessionData::load_session() {
        Ok(Some(s)) => {
            if *s.mc_token_expires_at() >= chrono::Utc::now().timestamp() {
                Some(s.minecraft_access_token().clone())
            } else {
                mc_token::refresh_minecraft_token(s.microsoft_refresh_token())
                    .await
                    .ok()
            }
        }
        _ => None,
    };
    if GLOBAL_CACHE.get("mc_ac_key").is_none()
        && let Some(token) = token_init_attempt
    {
        GLOBAL_CACHE.insert("mc_ac_key".into(), token);
    }
    info!(version = env!("CARGO_PKG_VERSION"), "VoxelRuler 啟動中");
    let has_token = GLOBAL_CACHE.get("mc_ac_key").is_some();
    info!(authenticated = has_token, "token 狀態載入完成");
    open_view().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_microsoft_auth() {
        let test_url = "voxelruler://auth?code=M.R3_aaa.123&state=xyz987";
        let action = DeepLinkAction::parse_string(test_url);
        if let DeepLinkAction::MicrosoftAuth(auth_args) = action {
            assert_eq!(auth_args.code, "M.R3_aaa.123");
            assert_eq!(auth_args.state, Some("xyz987".to_string()));
        } else {
            panic!("應該要成功解析為 MicrosoftAuth，但卻失敗了！");
        }
    }

    #[test]
    fn test_parse_with_trailing_slash() {
        let test_url = "voxelruler://auth/?code=secret_code";
        let action = DeepLinkAction::parse_string(test_url);
        if let DeepLinkAction::MicrosoftAuth(auth_args) = action {
            assert_eq!(auth_args.code, "secret_code");
        } else {
            panic!("結尾帶斜線應該也要能正確解析！");
        }
    }

    #[test]
    fn test_invalid_scheme() {
        let test_url = "http://auth?code=123";
        let action = DeepLinkAction::parse_string(test_url);
        assert!(matches!(action, DeepLinkAction::Unknown));
    }
}
