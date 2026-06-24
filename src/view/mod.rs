slint::include_modules!();
pub mod appearance;
pub mod create;
pub mod instance_detail;
pub mod launch;
pub mod login;
pub mod skin;

use instance_detail::*;
use launch::*;
use skin::*;

thread_local! {
    static SKIN_TIMER: std::cell::RefCell<Option<slint::Timer>> = std::cell::RefCell::new(None);
}
use crate::{
    mc_install,
    mc_instance::{InstanceConfig, InstanceStore},
    mc_parser::LaunchContext,
    mc_paths::McPaths,
    mc_token::{self, SessionData},
    mc_types::McSpecificVersionDetail,
    settings::AppSettings,
};
use anyhow::Context as _;
use slint::{Model, ModelRc, VecModel};
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    process::Child,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tracing::{debug, error, info, warn};

/// 下拉選單顯示文字（同時也是 Rust 端比對用的哨兵值）
const JAVA_MODE_LABEL_GLOBAL: &str = "Use Global Settings";
const JAVA_MODE_LABEL_MINECRAFT: &str = "Use Minecraft Runtime";
const JAVA_MODE_LABEL_CUSTOM: &str = "Use Custom Java";

/// 設定檔中儲存的 java_mode 值
const JAVA_MODE_GLOBAL: &str = "global";
const JAVA_MODE_MINECRAFT: &str = "minecraft";
const JAVA_MODE_CUSTOM: &str = "custom";

fn java_mode_to_label(mode: &str, is_instance: bool) -> &'static str {
    match mode {
        JAVA_MODE_CUSTOM => JAVA_MODE_LABEL_CUSTOM,
        JAVA_MODE_MINECRAFT => JAVA_MODE_LABEL_MINECRAFT,
        // 空字串 = 預設：instance 跟隨全域、全域跟隨 Minecraft
        _ if is_instance => JAVA_MODE_LABEL_GLOBAL,
        _ => JAVA_MODE_LABEL_MINECRAFT,
    }
}

fn java_label_to_mode(label: &str) -> &'static str {
    match label {
        JAVA_MODE_LABEL_CUSTOM => JAVA_MODE_CUSTOM,
        JAVA_MODE_LABEL_MINECRAFT => JAVA_MODE_MINECRAFT,
        _ => JAVA_MODE_GLOBAL,
    }
}

/// 啟動時實際使用的 Java 來源
#[derive(Debug)]
enum JavaSource {
    /// 使用者自訂的 java 執行檔路徑
    CustomPath(PathBuf),
    /// 跟隨版本 JSON 的 javaVersion.component（Minecraft 提供）
    VersionDefault,
}

/// Java 解析優先序：
/// instance（custom / minecraft / global）→ 全域（custom / minecraft）→ Minecraft 版本預設
fn resolve_java_source(instance: &InstanceConfig, settings: &AppSettings) -> JavaSource {
    match instance.java_mode.as_str() {
        JAVA_MODE_CUSTOM if !instance.java_path.trim().is_empty() => {
            return JavaSource::CustomPath(PathBuf::from(instance.java_path.trim()));
        }
        JAVA_MODE_MINECRAFT => return JavaSource::VersionDefault,
        // "global" / 空字串 / 其他 → 跟隨全域
        _ => {}
    }
    match settings.java_mode.as_str() {
        JAVA_MODE_CUSTOM if !settings.java_path.trim().is_empty() => {
            JavaSource::CustomPath(PathBuf::from(settings.java_path.trim()))
        }
        _ => JavaSource::VersionDefault,
    }
}

fn config_to_ui_data(config: &InstanceConfig) -> InstanceData {
    let play_time = if config.play_time_secs == 0 {
        String::new()
    } else {
        let h = config.play_time_secs / 3600;
        let m = (config.play_time_secs % 3600) / 60;
        format!("{}h {}m", h, m)
    };
    let icon_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/assets/icons/voxelruler.png");
    let image = slint::Image::load_from_path(&icon_path).unwrap_or_default();
    InstanceData {
        id: config.id.as_str().into(),
        name: config.name.as_str().into(),
        version: config.version.as_str().into(),
        mod_loader: config.mod_loader.as_str().into(),
        last_played: config.last_played.as_str().into(),
        play_time: play_time.into(),
        image,
        status: "ready".into(),
    }
}

#[allow(unused)]
pub async fn open_view() -> anyhow::Result<()> {
    let ui = MainApp::new()?;
    let logic = ui.global::<InstanceLogic>();

    let store = Arc::new(Mutex::new(InstanceStore::new(
        McPaths::new()?.instances_base_dir(),
    )));
    let master_configs: Arc<Mutex<Vec<InstanceConfig>>> = {
        let loaded = store.lock().unwrap().load().unwrap_or_default();
        Arc::new(Mutex::new(loaded))
    };
    {
        let configs = master_configs.lock().unwrap();
        let ui_items: Vec<InstanceData> = configs.iter().map(config_to_ui_data).collect();
        logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(ui_items))));
    }

    // running_procs 在 watcher callback 中也需要讀取，因此提前定義。
    // 這樣在重建 instance list 時，可以保留正在執行中的實例狀態，
    // 避免 watcher 刷新列表時把 "running" status overridden to "ready"。
    let running_procs: Arc<Mutex<HashMap<String, Child>>> = Arc::new(Mutex::new(HashMap::new()));
    let launching_procs: Arc<Mutex<std::collections::HashSet<String>>> =
        Arc::new(Mutex::new(std::collections::HashSet::new()));
    let instance_logs: Arc<Mutex<HashMap<String, VecDeque<crate::view::LogLine>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let (_debouncer, rx) = store.lock().unwrap().watch_changes()?;
    let ui_weak_for_watch = ui.as_weak();
    let store_for_watch = Arc::clone(&store);
    let master_for_watch = Arc::clone(&master_configs);
    let running_procs_for_watch = Arc::clone(&running_procs);
    tokio::spawn(async move {
        while rx.recv().is_ok() {
            info!("Detected instance.toml change, syncing to UI list...");
            let latest_configs = match store_for_watch.lock() {
                Ok(s) => s.load().unwrap_or_default(),
                Err(_) => continue,
            };
            if let Ok(mut master) = master_for_watch.lock() {
                *master = latest_configs.clone();
            }
            // 在進入 event loop 前取得目前正在執行的實例 ID 集合，
            // 保留這些實例的 "running" 狀態，不被重建列表覆蓋。
            let running_ids: std::collections::HashSet<String> = running_procs_for_watch
                .lock()
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            let ui_weak = ui_weak_for_watch.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui_handle) = ui_weak.upgrade() {
                    let logic = ui_handle.global::<InstanceLogic>();
                    let mut ui_items: Vec<InstanceData> = latest_configs
                        .iter()
                        .map(|c| {
                            let mut item = config_to_ui_data(c);
                            if running_ids.contains(&c.id) {
                                item.status = "running".into();
                            }
                            item
                        })
                        .collect();
                    if ui_items.is_empty() {
                        ui_items.push(InstanceData {
                            id: "".into(),
                            name: "".into(),
                            version: "".into(),
                            mod_loader: "".into(),
                            last_played: "".into(),
                            play_time: "".into(),
                            image: Default::default(),
                            status: "".into(),
                        });
                    }

                    logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(ui_items))));
                    info!("UI list securely synced with disk");
                }
            });
        }
    });

    let mc_versions_cache = Arc::new(std::sync::Mutex::new(
        Vec::<crate::mc_types::McVersion>::new(),
    ));

    let ui_weak_for_versions = ui.as_weak();
    {
        if let Some(ui) = ui_weak_for_versions.upgrade() {
            ui.global::<InstanceCreateLogic>().set_is_loading(true);
        }
    }

    let cache_for_fetch = Arc::clone(&mc_versions_cache);
    let ui_weak_for_fetch = ui.as_weak();
    tokio::spawn(async move {
        let api = crate::mc_api::McAction::new();
        match api.get_all_mc_versions().await {
            Ok(versions) => {
                *cache_for_fetch.lock().unwrap() = versions;
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_for_fetch.upgrade() {
                        let create = ui.global::<InstanceCreateLogic>();
                        create.set_is_loading(false);
                        create.invoke_filter_versions();
                    }
                })
                .ok();
            }
            Err(e) => {
                eprintln!("Failed to fetch MC versions: {e}");
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_for_fetch.upgrade() {
                        ui.global::<InstanceCreateLogic>().set_is_loading(false);
                    }
                })
                .ok();
            }
        }
    });

    let cache_for_filter = Arc::clone(&mc_versions_cache);
    let ui_weak_for_filter = ui.as_weak();
    ui.global::<InstanceCreateLogic>()
        .on_filter_versions(move || {
            let Some(ui) = ui_weak_for_filter.upgrade() else {
                return;
            };
            let logic = ui.global::<InstanceCreateLogic>();

            let versions = cache_for_filter.lock().unwrap();
            let search_text = logic.get_version_search_text().to_string().to_lowercase();
            let show_release = logic.get_show_release();
            let show_snapshot = logic.get_show_snapshot();
            let show_beta = logic.get_show_beta();
            let show_alpha = logic.get_show_alpha();
            let show_experimental = logic.get_show_experimental();

            let filtered: Vec<slint::SharedString> = versions
                .iter()
                .filter(|v| {
                    if !search_text.is_empty() && !v.id.to_lowercase().contains(&search_text) {
                        return false;
                    }

                    match v.r#type.as_str() {
                        "release" => show_release,
                        "snapshot" => show_snapshot,
                        "old_beta" => show_beta,
                        "old_alpha" => show_alpha,
                        "experimental" | "pending" => show_experimental,
                        _ => show_experimental,
                    }
                })
                .map(|v| v.id.clone().into())
                .collect();

            let current_selected = logic.get_selected_version().to_string();
            let mut found = false;
            for f in &filtered {
                if f.as_str() == current_selected {
                    found = true;
                    break;
                }
            }

            if !found {
                let first = filtered.first().cloned().unwrap_or_default();
                logic.set_selected_version(first);
            }

            logic.set_version_list(ModelRc::from(Rc::new(VecModel::from(filtered))));
            logic.invoke_loader_changed();
        });

    // ── 網路狀態偵測（footer status）────────────────────────────────────
    // 每 15 秒 HEAD 一次 Mojang 端點：對 launcher 而言「連得上 Mojang」才算 online
    let ui_weak_for_net = ui.as_weak();
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("Failed to create network detection client");
        loop {
            let online = client
                .head("https://launchermeta.mojang.com/mc/game/version_manifest_v2.json")
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false);
            let ui_weak = ui_weak_for_net.clone();
            if slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    let app = ui.global::<AppData>();
                    app.set_network_checking(false);
                    app.set_is_online(online);
                }
            })
            .is_err()
            {
                break; // event loop 已結束
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
        }
    });

    // ── Java 模式選單 + 全域設定載入 ─────────────────────────────────────
    {
        let edit_items: Vec<slint::SharedString> = vec![
            JAVA_MODE_LABEL_GLOBAL.into(),
            JAVA_MODE_LABEL_MINECRAFT.into(),
            JAVA_MODE_LABEL_CUSTOM.into(),
        ];
        ui.global::<InstanceEditLogic>()
            .set_java_mode_list(ModelRc::from(Rc::new(VecModel::from(edit_items))));

        let settings_items: Vec<slint::SharedString> = vec![
            JAVA_MODE_LABEL_MINECRAFT.into(),
            JAVA_MODE_LABEL_CUSTOM.into(),
        ];
        let app_settings = AppSettings::load();
        let sl = ui.global::<SettingsLogic>();
        sl.set_java_mode_list(ModelRc::from(Rc::new(VecModel::from(settings_items))));
        sl.set_selected_java_mode(java_mode_to_label(&app_settings.java_mode, false).into());
        sl.set_java_path(app_settings.java_path.as_str().into());
    }

    // ── 掃描系統 Java 安裝（背景執行，完成後填入兩處清單）────────────────
    let ui_weak_for_scan = ui.as_weak();
    tokio::spawn(async move {
        let javas = tokio::task::spawn_blocking(crate::java_scan::scan_system_javas)
            .await
            .unwrap_or_default();
        info!(count = javas.len(), "System Java scan completed");
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = ui_weak_for_scan.upgrade() else {
                return;
            };
            let items: Vec<slint::SharedString> = javas.iter().map(|p| p.as_str().into()).collect();
            ui.global::<InstanceEditLogic>()
                .set_detected_java_list(ModelRc::from(Rc::new(VecModel::from(items.clone()))));
            ui.global::<SettingsLogic>()
                .set_detected_java_list(ModelRc::from(Rc::new(VecModel::from(items))));
        });
    });

    // ── 系統檔案選擇框 ───────────────────────────────────────────────────
    // rfd 使用 xdg-portal 後端（Linux 不連結 GTK，AppImage 友善），
    // 該後端僅提供 async API，因此用 slint::spawn_local 在 UI 執行緒上等待。
    let ui_weak_for_edit_browse = ui.as_weak();
    ui.global::<InstanceEditLogic>().on_browse_java(move || {
        let ui_weak = ui_weak_for_edit_browse.clone();
        let _ = slint::spawn_local(async move {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .set_title("Select Java Executable")
                .pick_file()
                .await
                && let Some(ui) = ui_weak.upgrade()
            {
                ui.global::<InstanceEditLogic>()
                    .set_java_path(file.path().display().to_string().into());
            }
        });
    });

    let ui_weak_for_settings_browse = ui.as_weak();
    ui.global::<SettingsLogic>().on_browse_java(move || {
        let ui_weak = ui_weak_for_settings_browse.clone();
        let _ = slint::spawn_local(async move {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .set_title("Select Java Executable")
                .pick_file()
                .await
                && let Some(ui) = ui_weak.upgrade()
            {
                ui.global::<SettingsLogic>()
                    .set_java_path(file.path().display().to_string().into());
            }
        });
    });

    launch::setup_launch_logic(
        &ui,
        Arc::clone(&store),
        Arc::clone(&master_configs),
        Arc::clone(&running_procs),
        Arc::clone(&launching_procs),
        Arc::clone(&instance_logs),
    );

    instance_detail::setup_instance_detail_logic(
        &ui,
        Arc::clone(&store),
        Arc::clone(&master_configs),
        Arc::clone(&running_procs),
        Arc::clone(&instance_logs),
    );

    create::setup_create_logic(&ui, Arc::clone(&store), Arc::clone(&master_configs));

    let mod_logic = ui.global::<ModLogic>();
    let raw_mods: Vec<ModData> = mod_logic.get_mod_list().iter().collect();
    let mod_logic_weak = ui.as_weak();
    mod_logic.on_search_changed(move |text| {
        let ui = mod_logic_weak.unwrap();
        let logic = ui.global::<ModLogic>();
        let filtered: Vec<ModData> = raw_mods
            .iter()
            .filter(|m| {
                text.is_empty() || m.name.to_lowercase().contains(text.to_lowercase().as_str())
            })
            .cloned()
            .collect();
        logic.set_selected_index(0);
        logic.set_mod_list(ModelRc::from(Rc::new(VecModel::from(filtered))));
    });

    let applogic = ui.global::<AppLogic>();
    applogic.on_sidebar_change(|id| {
        debug!(tab = ?id, "sidebar switch");
    });

    login::setup_login_logic(&ui);

    appearance::setup_appearance_window(&ui);

    // slint::select_bundled_translation("zh_TW").unwrap();
    slint::select_bundled_translation("en_US").unwrap();
    ui.run()?;
    Ok(())
}

fn set_install_state(
    ui_weak: &slint::Weak<MainApp>,
    installing: bool,
    progress: f32,
    status: &str,
    is_error: bool,
) {
    let status = status.to_string();
    let ui_weak = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        let Some(ui) = ui_weak.upgrade() else { return };
        let logic = ui.global::<InstanceLogic>();
        logic.set_is_installing(installing);
        logic.set_install_progress(progress);
        logic.set_install_status(status.into());
        logic.set_install_is_error(is_error);
    });
}

fn set_instance_status(ui_weak: &slint::Weak<MainApp>, instance_id: &str, status: &str) {
    let id = instance_id.to_string();
    let status = status.to_string();
    let ui_weak = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        let Some(ui) = ui_weak.upgrade() else { return };
        // 詳細視窗開著同一實例時，同步 running 狀態指示
        let detail = ui.global::<InstanceDetailLogic>();
        if detail.get_instance_id().as_str() == id {
            detail.set_instance_running(status == "running");
        }
        let list = ui.global::<InstanceLogic>().get_instance_list();
        for i in 0..list.row_count() {
            if i >= list.row_count() {
                break;
            }
            if let Some(mut item) = list.row_data(i)
                && item.id.as_str() == id
            {
                item.status = status.into();
                if i < list.row_count() {
                    list.set_row_data(i, item);
                }
                break;
            }
        }
    });
}
