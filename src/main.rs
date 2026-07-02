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

mod instance_assets;
mod ipc;
mod java_scan;
mod mc_api;
mod mc_compat;
mod mc_install;
mod mc_instance;
mod mc_legacy_fml;
mod mc_modloader;
mod mc_parser;
mod mc_paths;
mod mc_token;
mod mc_types;
mod settings;
mod skin_history;
mod skin_renderer;
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

/// 設定應用程式日誌系統
fn setup_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

    #[cfg(not(debug_assertions))]
    {
        let log_dir = PROJECT_DIR
            .as_ref()
            .map(|d| d.data_dir().to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        std::fs::create_dir_all(&log_dir).ok();
        let file_appender = tracing_appender::rolling::never(&log_dir, "voxelruler.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("voxelruler=info"))
            .add_directive("icu_provider=off".parse().expect("有效的 filter directive"));
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().with_writer(non_blocking))
            .init();
        Some(guard)
    }

    #[cfg(debug_assertions)]
    {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("voxelruler=debug"))
            .add_directive("icu_provider=off".parse().expect("有效的 filter directive"));
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer())
            .with(console_subscriber::spawn())
            .init();
        None
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Deep Link 處理與單一實例 (Single Instance) IPC ────────────────────
    let (deep_link_tx, mut deep_link_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    let is_main = ipc::setup_ipc(deep_link_tx.clone()).await;
    if !is_main {
        // 如果是第二個執行個體，已經把 deeplink 傳遞給第一個了，直接關閉即可
        std::process::exit(0);
    }

    #[cfg(target_os = "macos")]
    {
        // macOS：URL scheme 透過 Apple Events 傳遞。
        let mut mac_rx = url_handler::register();
        let tx_clone = deep_link_tx.clone();
        tokio::spawn(async move {
            while let Some(url) = mac_rx.recv().await {
                let _ = tx_clone.send(url);
            }
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Windows / Linux：cargo-packager 會將 URL 以 argv[1] 傳入
        let args: Vec<String> = std::env::args().collect();
        if args.len() > 1 && args[1].starts_with("voxelruler://") {
            let _ = deep_link_tx.send(args[1].clone());
        }
    }

    // 集中處理所有來源的 Deep Link
    tokio::spawn(async move {
        while let Some(url) = deep_link_rx.recv().await {
            debug!(url = %url, "deep link channel received URL");
            match DeepLinkAction::parse_string(&url) {
                DeepLinkAction::MicrosoftAuth(auth_data) => {
                    debug!(
                        code = %auth_data.code,
                        state = ?auth_data.state,
                        "Received Microsoft OAuth deep link"
                    );
                    // TODO M2：呼叫 token exchange，更新 GLOBAL_CACHE
                }
                DeepLinkAction::Unknown => {
                    debug!("Received unknown VoxelRuler deep link, skipping");
                }
            }
        }
    });
    // 初始化日誌系統
    let _file_guard = setup_logging();
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

    // Clean up the IPC socket so the next launch doesn't hit a stale file.
    #[cfg(unix)]
    ipc::cleanup();

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
