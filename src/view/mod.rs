slint::include_modules!();
pub mod skin;
pub mod launch;
pub mod instance_detail;

use skin::*;
use launch::*;
use instance_detail::*;

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
    // 避免 watcher 刷新列表時把 "running" 狀態覆蓋成 "ready"。
    let running_procs: Arc<Mutex<HashMap<String, Child>>> = Arc::new(Mutex::new(HashMap::new()));

    let (_debouncer, rx) = store.lock().unwrap().watch_changes()?;
    let ui_weak_for_watch = ui.as_weak();
    let store_for_watch = Arc::clone(&store);
    let master_for_watch = Arc::clone(&master_configs);
    let running_procs_for_watch = Arc::clone(&running_procs);
    tokio::spawn(async move {
        while rx.recv().is_ok() {
            info!("偵測到 instance.toml 變動，正在同步至 UI 列表...");
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
                    info!("UI 列表已與硬碟安全同步");
                }
            });
        }
    });

    let mc_versions_cache = Arc::new(std::sync::Mutex::new(Vec::<crate::mc_types::McVersion>::new()));

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
                }).ok();
            }
            Err(e) => {
                eprintln!("Failed to fetch MC versions: {e}");
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_for_fetch.upgrade() {
                        ui.global::<InstanceCreateLogic>().set_is_loading(false);
                    }
                }).ok();
            }
        }
    });

    let cache_for_filter = Arc::clone(&mc_versions_cache);
    let ui_weak_for_filter = ui.as_weak();
    ui.global::<InstanceCreateLogic>().on_filter_versions(move || {
        let Some(ui) = ui_weak_for_filter.upgrade() else { return; };
        let logic = ui.global::<InstanceCreateLogic>();
        
        let versions = cache_for_filter.lock().unwrap();
        let search_text = logic.get_version_search_text().to_string().to_lowercase();
        let show_release = logic.get_show_release();
        let show_snapshot = logic.get_show_snapshot();
        let show_beta = logic.get_show_beta();
        let show_alpha = logic.get_show_alpha();
        let show_experimental = logic.get_show_experimental();

        let filtered: Vec<slint::SharedString> = versions.iter()
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
            .expect("建立網路偵測 client 失敗");
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
        info!(count = javas.len(), "系統 Java 掃描完成");
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
                .set_title("選擇 Java 執行檔")
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
                .set_title("選擇 Java 執行檔")
                .pick_file()
                .await
                && let Some(ui) = ui_weak.upgrade()
            {
                ui.global::<SettingsLogic>()
                    .set_java_path(file.path().display().to_string().into());
            }
        });
    });

    let master_for_search = Arc::clone(&master_configs);
    let ui_weak_for_search = ui.as_weak();
    logic.on_search_changed(move |text| {
        let Some(ui) = ui_weak_for_search.upgrade() else {
            return;
        };
        let logic = ui.global::<InstanceLogic>();
        let configs = master_for_search.lock().unwrap();
        let filtered: Vec<InstanceData> = configs
            .iter()
            .filter(|c| text.is_empty() || c.name.to_lowercase().contains(&text.to_lowercase()))
            .map(config_to_ui_data)
            .collect();
        logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(filtered))));
    });

    let instance_logs: Arc<Mutex<HashMap<String, VecDeque<String>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let master_for_launch = Arc::clone(&master_configs);
    let running_procs_for_launch = Arc::clone(&running_procs);
    let instance_logs_for_launch = Arc::clone(&instance_logs);
    let ui_weak_for_launch = ui.as_weak();
    logic.on_launch_instance(move |id| {
        let config = {
            let configs = master_for_launch.lock().unwrap();
            configs
                .iter()
                .find(|c| c.id == id.as_str())
                .cloned()
                .unwrap_or_else(|| InstanceConfig {
                    id: id.to_string(),
                    name: id.to_string(),
                    version: id.to_string(),
                    ..Default::default()
                })
        };
        let instance_id = config.id.clone();
        let running_procs = Arc::clone(&running_procs_for_launch);
        let ui_weak = ui_weak_for_launch.clone();
        let logs = Arc::clone(&instance_logs_for_launch);
        if running_procs.lock().unwrap().contains_key(&instance_id) {
            return;
        }
        tokio::spawn(async move {
            match do_launch(config, ui_weak.clone(), logs).await {
                Ok(child) => {
                    running_procs
                        .lock()
                        .unwrap()
                        .insert(instance_id.clone(), child);
                    set_instance_status(&ui_weak, &instance_id, "running");
                    let running_procs_watch = Arc::clone(&running_procs);
                    let ui_weak_watch = ui_weak.clone();
                    let id_watch = instance_id.clone();
                    tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            let mut map = running_procs_watch.lock().unwrap();
                            let Some(child) = map.get_mut(&id_watch) else {
                                break;
                            };
                            match child.try_wait() {
                                Ok(Some(_)) | Err(_) => {
                                    map.remove(&id_watch);
                                    drop(map);
                                    set_instance_status(&ui_weak_watch, &id_watch, "ready");
                                    break;
                                }
                                Ok(None) => {}
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("啟動失敗: {e:#}");
                    set_install_state(&ui_weak, true, 0.0, &format!("啟動失敗：{e:#}"), true);
                }
            }
        });
    });

    let running_procs_for_kill = Arc::clone(&running_procs);
    let ui_weak_for_kill = ui.as_weak();
    logic.on_kill_instance(move |id| {
        let mut map = running_procs_for_kill.lock().unwrap();
        if let Some(mut child) = map.remove(id.as_str()) {
            let _ = child.kill();
            drop(map);
            set_instance_status(&ui_weak_for_kill, id.as_str(), "ready");
        }
    });

    // 「編輯實例」→ 開啟詳細視窗的「設定」分頁（tab 9）
    let master_for_edit_open = Arc::clone(&master_configs);
    let running_for_edit_open = Arc::clone(&running_procs);
    let logs_for_edit_open = Arc::clone(&instance_logs);
    let ui_weak_for_edit_open = ui.as_weak();
    logic.on_open_instance_settings(move |id| {
        let Some(ui) = ui_weak_for_edit_open.upgrade() else {
            return;
        };
        open_instance_detail(
            &ui,
            &master_for_edit_open,
            &running_for_edit_open,
            &logs_for_edit_open,
            id.as_str(),
            9,
        );
    });

    let edit_logic = ui.global::<InstanceEditLogic>();

    let ui_weak_for_edit_cancel = ui.as_weak();
    edit_logic.on_cancel_edit(move || {
        if let Some(ui) = ui_weak_for_edit_cancel.upgrade() {
            ui.global::<InstanceEditLogic>().set_show_dialog(false);
        }
    });

    let store_for_edit = Arc::clone(&store);
    let master_for_edit = Arc::clone(&master_configs);
    let ui_weak_for_edit_confirm = ui.as_weak();
    edit_logic.on_confirm_edit(move || {
        let Some(ui) = ui_weak_for_edit_confirm.upgrade() else {
            return;
        };
        let edit = ui.global::<InstanceEditLogic>();
        let id = edit.get_instance_id().to_string();
        let xmx = edit.get_xmx().trim().to_string();
        let xms = edit.get_xms().trim().to_string();
        let java_path = edit.get_java_path().trim().to_string();
        let java_mode = java_label_to_mode(edit.get_selected_java_mode().as_str());

        if java_mode == JAVA_MODE_CUSTOM {
            if java_path.is_empty() {
                edit.set_error_msg("選擇自訂 Java 時必須填寫路徑".into());
                return;
            }
            if !Path::new(&java_path).is_file() {
                edit.set_error_msg("自訂 Java 路徑不存在或不是檔案".into());
                return;
            }
        }

        let updated_config = {
            let mut master = master_for_edit.lock().unwrap();
            let Some(c) = master.iter_mut().find(|c| c.id == id) else {
                edit.set_error_msg("找不到實例".into());
                return;
            };
            c.xmx = if xmx.is_empty() { "2G".into() } else { xmx };
            c.xms = if xms.is_empty() { "512M".into() } else { xms };
            c.java_mode = java_mode.to_string();
            // 路徑文字保留，切換模式時不清掉使用者輸入
            c.java_path = java_path;
            c.clone()
        };

        // 寫入 instance.toml；watcher 會自動同步 UI 列表
        match store_for_edit.lock().unwrap().save_one(&updated_config) {
            Ok(()) => {
                edit.set_error_msg("".into());
                ui.global::<InstanceDetailLogic>()
                    .set_status_msg("✓ 已儲存".into());
            }
            Err(e) => edit.set_error_msg(format!("儲存失敗：{e}").into()),
        }
    });

    let ui_weak_for_settings = ui.as_weak();
    ui.global::<SettingsLogic>().on_save_settings(move || {
        let Some(ui) = ui_weak_for_settings.upgrade() else {
            return;
        };
        let sl = ui.global::<SettingsLogic>();
        let java_path = sl.get_java_path().trim().to_string();
        let java_mode = java_label_to_mode(sl.get_selected_java_mode().as_str());

        if java_mode == JAVA_MODE_CUSTOM {
            if java_path.is_empty() {
                sl.set_status_msg("⚠ 選擇自訂 Java 時必須填寫路徑".into());
                return;
            }
            if !Path::new(&java_path).is_file() {
                sl.set_status_msg("⚠ Java 路徑不存在或不是檔案".into());
                return;
            }
        }

        let new_settings = AppSettings {
            java_mode: java_mode.to_string(),
            java_path,
        };
        match new_settings.save() {
            Ok(()) => sl.set_status_msg("✓ 已儲存".into()),
            Err(e) => sl.set_status_msg(format!("儲存失敗：{e}").into()),
        }
    });

    let ui_weak_for_dismiss = ui.as_weak();
    logic.on_dismiss_install_dialog(move || {
        if let Some(ui) = ui_weak_for_dismiss.upgrade() {
            let logic = ui.global::<InstanceLogic>();
            logic.set_is_installing(false);
            logic.set_install_is_error(false);
            logic.set_install_status("".into());
        }
    });

    let instance_logs_for_open = Arc::clone(&instance_logs);
    let ui_weak_for_open = ui.as_weak();
    logic.on_open_log(move |id| {
        let id = id.to_string();
        let lines: Vec<slint::SharedString> = {
            let logs = instance_logs_for_open.lock().unwrap();
            logs.get(&id)
                .map(|deque| deque.iter().map(|s| s.as_str().into()).collect())
                .unwrap_or_default()
        };
        if let Some(ui) = ui_weak_for_open.upgrade() {
            let logic = ui.global::<InstanceLogic>();
            logic.set_log_instance_id(id.into());
            logic.set_log_lines(ModelRc::from(Rc::new(VecModel::from(lines))));
            logic.set_show_log(true);
        }
    });

    let ui_weak_for_close_log = ui.as_weak();
    logic.on_close_log(move || {
        if let Some(ui) = ui_weak_for_close_log.upgrade() {
            ui.global::<InstanceLogic>().set_show_log(false);
        }
    });

    // ── 右鍵選單：開啟資料夾 / 複製 / 重新命名 / 刪除 ─────────────────────
    logic.on_open_instance_folder(move |id| {
        if let Ok(paths) = McPaths::new() {
            let _ = open::that(paths.instance_dir(id.as_str()));
        }
    });

    let store_for_dup = Arc::clone(&store);
    let master_for_dup = Arc::clone(&master_configs);
    let ui_weak_for_dup = ui.as_weak();
    logic.on_duplicate_instance(move |id| {
        let config = {
            let configs = master_for_dup.lock().unwrap();
            configs.iter().find(|c| c.id == id.as_str()).cloned()
        };
        let Some(config) = config else { return };
        let store = Arc::clone(&store_for_dup);
        let ui_weak = ui_weak_for_dup.clone();
        // 實例資料夾可能很大（worlds / mods），放 blocking thread 複製
        tokio::task::spawn_blocking(move || {
            let result = (|| -> anyhow::Result<()> {
                let paths = McPaths::new()?;
                let mut new_config = config.clone();
                new_config.id = uuid::Uuid::new_v4().to_string();
                new_config.name = format!("{} (copy)", config.name);
                new_config.last_played = String::new();
                new_config.play_time_secs = 0;
                let src = paths.instance_dir(&config.id);
                let dst = paths.instance_dir(&new_config.id);
                crate::instance_assets::copy_dir_recursive(&src, &dst)?;
                // 覆寫複製來的 instance.toml（換 id / 名稱）；watcher 會同步列表
                store.lock().unwrap().save_one(&new_config)?;
                Ok(())
            })();
            if let Err(e) = result {
                error!("複製實例失敗: {e:#}");
                set_install_state(&ui_weak, true, 0.0, &format!("複製實例失敗：{e:#}"), true);
            }
        });
    });

    let store_for_rename = Arc::clone(&store);
    let master_for_rename = Arc::clone(&master_configs);
    let ui_weak_for_rename = ui.as_weak();
    logic.on_confirm_rename(move || {
        let Some(ui) = ui_weak_for_rename.upgrade() else {
            return;
        };
        let logic = ui.global::<InstanceLogic>();
        let id = logic.get_rename_id().to_string();
        let new_name = logic.get_rename_text().trim().to_string();
        if new_name.is_empty() {
            logic.set_rename_error("名稱不可為空".into());
            return;
        }
        let updated = {
            let mut master = master_for_rename.lock().unwrap();
            let Some(c) = master.iter_mut().find(|c| c.id == id) else {
                logic.set_rename_error("找不到實例".into());
                return;
            };
            c.name = new_name;
            c.clone()
        };
        match store_for_rename.lock().unwrap().save_one(&updated) {
            Ok(()) => logic.set_show_rename(false),
            Err(e) => logic.set_rename_error(format!("儲存失敗：{e}").into()),
        }
    });

    let store_for_del = Arc::clone(&store);
    let running_for_del = Arc::clone(&running_procs);
    let ui_weak_for_del = ui.as_weak();
    logic.on_confirm_delete(move || {
        let Some(ui) = ui_weak_for_del.upgrade() else {
            return;
        };
        let logic = ui.global::<InstanceLogic>();
        let id = logic.get_delete_id().to_string();
        // 執行中先停止
        if let Some(mut child) = running_for_del.lock().unwrap().remove(&id) {
            let _ = child.kill();
        }
        if let Err(e) = store_for_del.lock().unwrap().delete_one(&id) {
            error!("刪除實例失敗: {e:#}");
        }
        logic.set_show_delete_confirm(false);
        // 詳細視窗若開著同一實例，順手關閉
        let detail = ui.global::<InstanceDetailLogic>();
        if detail.get_instance_id().as_str() == id {
            detail.set_show_dialog(false);
        }
    });

    // ── 實例詳細視窗（側欄分頁）──────────────────────────────────────────
    let detail_logic = ui.global::<InstanceDetailLogic>();

    let master_for_detail_open = Arc::clone(&master_configs);
    let running_for_detail_open = Arc::clone(&running_procs);
    let logs_for_detail_open = Arc::clone(&instance_logs);
    let ui_weak_for_detail_open = ui.as_weak();
    detail_logic.on_open_detail(move |id, tab| {
        let Some(ui) = ui_weak_for_detail_open.upgrade() else {
            return;
        };
        open_instance_detail(
            &ui,
            &master_for_detail_open,
            &running_for_detail_open,
            &logs_for_detail_open,
            id.as_str(),
            tab,
        );
    });

    let ui_weak_for_detail_close = ui.as_weak();
    detail_logic.on_close_detail(move || {
        if let Some(ui) = ui_weak_for_detail_close.upgrade() {
            ui.global::<InstanceDetailLogic>().set_show_dialog(false);
        }
    });

    let logs_for_tab = Arc::clone(&instance_logs);
    let ui_weak_for_tab = ui.as_weak();
    detail_logic.on_tab_changed(move |tab| {
        let Some(ui) = ui_weak_for_tab.upgrade() else {
            return;
        };
        let id = ui
            .global::<InstanceDetailLogic>()
            .get_instance_id()
            .to_string();
        load_detail_tab(&ui, &id, tab, &logs_for_tab);
    });

    let ui_weak_for_subfolder = ui.as_weak();
    detail_logic.on_open_subfolder(move |category| {
        let Some(ui) = ui_weak_for_subfolder.upgrade() else {
            return;
        };
        let id = ui
            .global::<InstanceDetailLogic>()
            .get_instance_id()
            .to_string();
        let Ok(paths) = McPaths::new() else { return };
        let dir = detail_category_dir(&paths, &id, category.as_str());
        let _ = std::fs::create_dir_all(&dir);
        let _ = open::that(dir);
    });

    let logs_for_toggle = Arc::clone(&instance_logs);
    let ui_weak_for_toggle = ui.as_weak();
    detail_logic.on_toggle_entry(move |category, file_name| {
        let Some(ui) = ui_weak_for_toggle.upgrade() else {
            return;
        };
        let detail = ui.global::<InstanceDetailLogic>();
        let id = detail.get_instance_id().to_string();
        let Ok(paths) = McPaths::new() else { return };
        let dir = detail_category_dir(&paths, &id, category.as_str());
        match crate::instance_assets::toggle_disabled(&dir, file_name.as_str()) {
            Ok(_) => detail.set_status_msg("".into()),
            Err(e) => detail.set_status_msg(format!("{e}").into()),
        }
        load_detail_tab(
            &ui,
            &id,
            detail_category_tab(category.as_str()),
            &logs_for_toggle,
        );
    });

    let logs_for_delete_entry = Arc::clone(&instance_logs);
    let ui_weak_for_delete_entry = ui.as_weak();
    detail_logic.on_delete_entry(move |category, file_name| {
        let Some(ui) = ui_weak_for_delete_entry.upgrade() else {
            return;
        };
        let detail = ui.global::<InstanceDetailLogic>();
        let id = detail.get_instance_id().to_string();
        let Ok(paths) = McPaths::new() else { return };
        let dir = detail_category_dir(&paths, &id, category.as_str());
        match crate::instance_assets::delete_entry(&dir, file_name.as_str()) {
            Ok(()) => detail.set_status_msg("".into()),
            Err(e) => detail.set_status_msg(format!("{e}").into()),
        }
        load_detail_tab(
            &ui,
            &id,
            detail_category_tab(category.as_str()),
            &logs_for_delete_entry,
        );
    });

    let logs_for_add = Arc::clone(&instance_logs);
    let ui_weak_for_add = ui.as_weak();
    detail_logic.on_add_entry(move |category| {
        let ui_weak = ui_weak_for_add.clone();
        let logs = Arc::clone(&logs_for_add);
        let category = category.to_string();
        let _ = slint::spawn_local(async move {
            let mut dialog = rfd::AsyncFileDialog::new().set_title("選擇要加入的檔案");
            dialog = match category.as_str() {
                "mods" => dialog.add_filter("Minecraft Mod", &["jar"]),
                "resourcepacks" | "shaderpacks" => dialog.add_filter("Pack", &["zip"]),
                _ => dialog,
            };
            let Some(files) = dialog.pick_files().await else {
                return;
            };
            let Some(ui) = ui_weak.upgrade() else { return };
            let detail = ui.global::<InstanceDetailLogic>();
            let id = detail.get_instance_id().to_string();
            let Ok(paths) = McPaths::new() else { return };
            let dir = detail_category_dir(&paths, &id, &category);
            for f in files {
                if let Err(e) = crate::instance_assets::add_file(&dir, f.path()) {
                    detail.set_status_msg(format!("{e}").into());
                }
            }
            load_detail_tab(&ui, &id, detail_category_tab(&category), &logs);
        });
    });

    let ui_weak_for_notes = ui.as_weak();
    detail_logic.on_save_notes(move || {
        let Some(ui) = ui_weak_for_notes.upgrade() else {
            return;
        };
        let detail = ui.global::<InstanceDetailLogic>();
        let id = detail.get_instance_id().to_string();
        let Ok(paths) = McPaths::new() else { return };
        match crate::instance_assets::save_notes(
            &paths.instance_dir(&id),
            detail.get_notes().as_str(),
        ) {
            Ok(()) => detail.set_notes_status("✓ 已儲存".into()),
            Err(e) => detail.set_notes_status(format!("{e}").into()),
        }
    });

    let ui_weak_for_view_log = ui.as_weak();
    detail_logic.on_view_log_file(move |name| {
        let Some(ui) = ui_weak_for_view_log.upgrade() else {
            return;
        };
        let detail = ui.global::<InstanceDetailLogic>();
        let id = detail.get_instance_id().to_string();
        let Ok(paths) = McPaths::new() else { return };
        let logs_dir = paths.instance_dir(&id).join("logs");
        detail.set_viewing_log_name(name.clone());
        match crate::instance_assets::read_log_lines(&logs_dir, name.as_str(), 2000) {
            Ok(lines) => {
                let shared: Vec<slint::SharedString> =
                    lines.iter().map(|s| s.as_str().into()).collect();
                detail.set_other_log_content(ModelRc::from(Rc::new(VecModel::from(shared))));
            }
            Err(e) => {
                let msg: Vec<slint::SharedString> = vec![format!("讀取失敗：{e}").into()];
                detail.set_other_log_content(ModelRc::from(Rc::new(VecModel::from(msg))));
            }
        }
    });

    let store_for_version = Arc::clone(&store);
    let master_for_version = Arc::clone(&master_configs);
    let ui_weak_for_version_save = ui.as_weak();
    detail_logic.on_save_version(move || {
        let Some(ui) = ui_weak_for_version_save.upgrade() else {
            return;
        };
        let detail = ui.global::<InstanceDetailLogic>();
        let id = detail.get_instance_id().to_string();
        let new_version = detail.get_selected_version().to_string();
        if new_version.is_empty() {
            return;
        }
        let updated = {
            let mut master = master_for_version.lock().unwrap();
            let Some(c) = master.iter_mut().find(|c| c.id == id) else {
                return;
            };
            c.version = new_version.clone();
            c.clone()
        };
        match store_for_version.lock().unwrap().save_one(&updated) {
            Ok(()) => {
                detail.set_version(new_version.as_str().into());
                detail.set_status_msg("✓ 已儲存".into());
            }
            Err(e) => detail.set_status_msg(format!("儲存失敗：{e}").into()),
        }
    });

    let ui_weak_for_new = ui.as_weak();
    logic.on_new_instance(move || {
        let Some(ui) = ui_weak_for_new.upgrade() else {
            return;
        };
        let create = ui.global::<InstanceCreateLogic>();
        create.set_name("".into());
        create.set_mod_loader("None".into());
        create.set_xmx("2G".into());
        create.set_xms("512M".into());
        create.set_logs_enabled(true);
        create.set_world_path("".into());
        create.set_resource_pack("".into());
        create.set_shader_pack("".into());
        create.set_error_msg("".into());
        create.set_active_tab(0);
        create.set_show_dialog(true);
    });

    let create_logic = ui.global::<InstanceCreateLogic>();

    let ui_weak_for_cancel = ui.as_weak();
    create_logic.on_cancel_create(move || {
        if let Some(ui) = ui_weak_for_cancel.upgrade() {
            ui.global::<InstanceCreateLogic>().set_show_dialog(false);
        }
    });

    let ui_weak_for_loader = ui.as_weak();
    create_logic.on_loader_changed(move || {
        let Some(ui) = ui_weak_for_loader.upgrade() else {
            return;
        };
        let logic = ui.global::<InstanceCreateLogic>();
        
        let mc_version = logic.get_selected_version().to_string();
        let mod_loader_str = logic.get_mod_loader().to_string();
        
        if mod_loader_str == "None" || mc_version.is_empty() {
            logic.set_mod_loader_versions(ModelRc::from(Rc::new(VecModel::from(vec![]))));
            logic.set_selected_mod_loader_version("".into());
            return;
        }
        
        let loader_type = match mod_loader_str.as_str() {
            "Fabric" => crate::mc_modloader::ModLoaderType::Fabric,
            "Forge" => crate::mc_modloader::ModLoaderType::Forge,
            "NeoForge" => crate::mc_modloader::ModLoaderType::NeoForge,
            _ => return,
        };
        
        logic.set_is_loading(true);
        let ui_handle_async = ui.as_weak();
        
        tokio::spawn(async move {
            match crate::mc_modloader::ModLoaderApi::get_loader_versions(loader_type, &mc_version).await {
                Ok(versions) => {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_handle_async.upgrade() {
                            let logic = ui.global::<InstanceCreateLogic>();
                            let slint_versions: Vec<slint::SharedString> = 
                                versions.into_iter().map(slint::SharedString::from).collect();
                            let model = ModelRc::from(Rc::new(VecModel::from(slint_versions.clone())));
                            logic.set_mod_loader_versions(model);
                            
                            if let Some(first) = slint_versions.first() {
                                logic.set_selected_mod_loader_version(first.clone());
                            } else {
                                logic.set_selected_mod_loader_version("沒有可用版本".into());
                            }
                            logic.set_is_loading(false);
                        }
                    });
                }
                Err(e) => {
                    println!("抓取 Mod Loader 版本失敗: {}", e);
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_handle_async.upgrade() {
                            let logic = ui.global::<InstanceCreateLogic>();
                            logic.set_mod_loader_versions(ModelRc::from(Rc::new(VecModel::from(vec![]))));
                            logic.set_selected_mod_loader_version("讀取失敗".into());
                            logic.set_is_loading(false);
                        }
                    });
                }
            }
        });
    });

    let store_for_create = Arc::clone(&store);
    let master_for_create = Arc::clone(&master_configs);
    let ui_weak_for_confirm = ui.as_weak();
    create_logic.on_confirm_create(move || {
        let Some(ui) = ui_weak_for_confirm.upgrade() else {
            return;
        };
        let create = ui.global::<InstanceCreateLogic>();

        let name = create.get_name().to_string();
        let version = create.get_selected_version().to_string();

        if name.trim().is_empty() {
            create.set_error_msg("實例名稱不可為空".into());
            return;
        }
        if version.is_empty() {
            create.set_error_msg("請選擇 Minecraft 版本".into());
            return;
        }

        let config = InstanceConfig {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.trim().to_string(),
            version,
            mod_loader: create.get_mod_loader().to_string(),
            mod_loader_version: create.get_selected_mod_loader_version().to_string(),
            xmx: create.get_xmx().to_string(),
            xms: create.get_xms().to_string(),
            logs_enabled: create.get_logs_enabled(),
            world_path: create.get_world_path().to_string(),
            resource_pack: create.get_resource_pack().to_string(),
            shader_pack: create.get_shader_pack().to_string(),
            ..Default::default()
        };

        match store_for_create.lock().unwrap().append(config) {
            Ok(updated) => {
                *master_for_create.lock().unwrap() = updated.clone();
                let logic = ui.global::<InstanceLogic>();
                let new_items: Vec<InstanceData> = updated.iter().map(config_to_ui_data).collect();
                logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(new_items))));
                create.set_show_dialog(false);
            }
            Err(e) => {
                create.set_error_msg(format!("建立失敗：{e}").into());
            }
        }
    });

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
        debug!(tab = ?id, "sidebar 切換");
    });

    if let Ok(Some(session)) = SessionData::load_session()
        && !session.mc_username().is_empty()
    {
        let is_expired = *session.mc_token_expires_at() < chrono::Utc::now().timestamp();
        let username = session.mc_username().clone();
        let token = session.minecraft_access_token().clone();
        let ui_weak_for_init = ui.as_weak();

        tokio::spawn(async move {
            let avatar_path = fetch_avatar_path(&username).await;
            let (authenticator_text, status_text) = if is_expired {
                ("Microsoft".to_string(), "Offline".to_string())
            } else {
                let api = crate::mc_api::McAction::new().authenticate(&token);
                match api.check_game_ownership().await {
                    Ok(true) => ("Microsoft (Premium)".to_string(), "Online".to_string()),
                    Ok(false) => ("Microsoft (Unpaid)".to_string(), "Online".to_string()),
                    Err(_) => ("Microsoft".to_string(), "Offline".to_string()),
                }
            };

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_for_init.upgrade() {
                    let avatar_img = avatar_path
                        .and_then(|p| slint::Image::load_from_path(&p).ok())
                        .unwrap_or_default();
                    let pal = ui.global::<PageAccountLogic>();
                    let row = AccountRow {
                        checked: true,
                        authenticator: authenticator_text.into(),
                        username: username.into(),
                        status: status_text.into(),
                        avatar: avatar_img,
                    };
                    pal.set_active_account(row.clone());
                    pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(vec![row]))));
                }
            });
        });
    }

    let page_account_logic_clone = ui.global::<PageAccountLogic>();
    page_account_logic_clone.on_open_browser_url(|url| {
        let _ = open::that(url.as_str());
    });
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_flag_clone = Arc::clone(&cancel_flag);
    let page_account_logic_clone = ui.global::<PageAccountLogic>();
    page_account_logic_clone.on_cancel_login(move || {
        cancel_flag_clone.store(true, Ordering::SeqCst);
    });
    let ui_weak = ui.as_weak();
    let page_account_logic = ui.global::<PageAccountLogic>();
    page_account_logic.on_login_with_microsoft(move || {
        let ui_weak = ui_weak.clone();
        let cancel_flag = Arc::clone(&cancel_flag);
        cancel_flag.store(false, Ordering::SeqCst);
        if let Some(ui) = ui_weak.upgrade() {
            let pal = ui.global::<PageAccountLogic>();
            pal.set_login_url("".into());
            pal.set_login_status_text("正在產生安全登入連結...".into());
            pal.set_is_error(false);
            pal.set_is_logging_in(true);
        }
        tokio::spawn(async move {
            let ui_weak_for_url = ui_weak.clone();
            let on_url_ready = move |url: String| {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_for_url.upgrade() {
                        let pal = ui.global::<PageAccountLogic>();
                        pal.set_login_url(url.into());
                        pal.set_login_status_text("請在打開的瀏覽器網頁中完成驗證。".into());
                    }
                });
            };
            match mc_token::set_token_in_native_store(on_url_ready).await {
                Ok(_new_token) => {
                    if cancel_flag.load(Ordering::SeqCst) {
                        return;
                    }
                    let api = crate::mc_api::McAction::new().authenticate(&_new_token);
                    let is_premium = api.check_game_ownership().await.unwrap_or(false);
                    let authenticator_text = if is_premium {
                        "Microsoft (Premium)".to_string()
                    } else {
                        "Microsoft (Unpaid)".to_string()
                    };
                    let username = SessionData::load_session()
                        .ok()
                        .flatten()
                        .map(|s| s.mc_username().clone())
                        .unwrap_or_default();

                    let avatar_path = fetch_avatar_from_mojang(&_new_token, &username, true)
                        .await
                        .map(|(p, _)| p);

                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let avatar_img = avatar_path
                                .and_then(|p| slint::Image::load_from_path(&p).ok())
                                .unwrap_or_default();
                            let pal = ui.global::<PageAccountLogic>();
                            pal.set_is_logging_in(false);
                            if !username.is_empty() {
                                let row = AccountRow {
                                    checked: true,
                                    authenticator: authenticator_text.into(),
                                    username: username.into(),
                                    status: "Online".into(),
                                    avatar: avatar_img,
                                };
                                pal.set_active_account(row.clone());
                                pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(vec![row]))));
                            }
                        }
                    });
                }
                Err(e) => {
                    let error_msg = format!("登入失敗：{}", e);
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let pal = ui.global::<PageAccountLogic>();
                            pal.set_is_logging_in(false);
                            pal.set_is_error(true);
                            pal.set_login_url("".into());
                            pal.set_login_status_text(error_msg.into());
                        }
                    });
                }
            }
        });
    });
    let ui_weak_remove = ui.as_weak();
    ui.global::<PageAccountLogic>().on_remove_account(move || {
        let Some(ui) = ui_weak_remove.upgrade() else {
            return;
        };
        let pal = ui.global::<PageAccountLogic>();
        let idx = pal.get_selected_index();
        if idx < 0 {
            return;
        }

        let accounts: Vec<AccountRow> = pal.get_accounts().iter().collect();
        let new_accounts: Vec<AccountRow> = accounts
            .into_iter()
            .enumerate()
            .filter(|(i, _)| *i != idx as usize)
            .map(|(_, r)| r)
            .collect();

        pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(new_accounts.clone()))));
        pal.set_selected_index(-1);

        if let Some(default_acc) = new_accounts.iter().find(|r| r.checked) {
            pal.set_active_account(default_acc.clone());
        } else {
            let mut guest = pal.get_active_account();
            guest.username = "Guest".into();
            guest.authenticator = "No Account".into();
            guest.status = "Offline".into();
            guest.checked = false;
            pal.set_active_account(guest);
        }

        let _ = mc_token::SessionData::delete_session();
    });

    let ui_weak_set_default = ui.as_weak();
    ui.global::<PageAccountLogic>()
        .on_set_default_account(move || {
            let Some(ui) = ui_weak_set_default.upgrade() else {
                return;
            };
            let pal = ui.global::<PageAccountLogic>();
            let idx = pal.get_selected_index();
            if idx < 0 {
                return;
            }

            let accounts: Vec<AccountRow> = pal.get_accounts().iter().collect();
            let new_accounts: Vec<AccountRow> = accounts
                .into_iter()
                .enumerate()
                .map(|(i, mut r)| {
                    r.checked = i == idx as usize;
                    r
                })
                .collect();

            if let Some(default_acc) = new_accounts.iter().find(|r| r.checked) {
                pal.set_active_account(default_acc.clone());
            }

            pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(new_accounts))));
        });

    let ui_weak_unset_default = ui.as_weak();
    ui.global::<PageAccountLogic>()
        .on_unset_default_account(move || {
            let Some(ui) = ui_weak_unset_default.upgrade() else {
                return;
            };
            let pal = ui.global::<PageAccountLogic>();
            let accounts: Vec<AccountRow> = pal.get_accounts().iter().collect();
            let new_accounts: Vec<AccountRow> = accounts
                .into_iter()
                .map(|mut r| {
                    r.checked = false;
                    r
                })
                .collect();

            let mut guest = pal.get_active_account();
            guest.username = "Guest".into();
            guest.authenticator = "No Account".into();
            guest.status = "Offline".into();
            guest.checked = false;
            pal.set_active_account(guest);

            pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(new_accounts))));
        });

    let ui_weak_add_offline = ui.as_weak();
    ui.global::<PageAccountLogic>()
        .on_confirm_add_offline_account(move |username| {
            let Some(ui) = ui_weak_add_offline.upgrade() else {
                return;
            };
            let pal = ui.global::<PageAccountLogic>();
            let username = username.to_string();

            let mut accounts: Vec<AccountRow> = pal.get_accounts().iter().collect();
            let new_row = AccountRow {
                checked: accounts.is_empty(),
                authenticator: "Offline".into(),
                username: username.clone().into(),
                status: "Ready".into(),
                avatar: slint::Image::default(),
            };

            if new_row.checked {
                pal.set_active_account(new_row.clone());
            }

            accounts.push(new_row);
            pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(accounts))));
        });

    let ui_weak_refresh = ui.as_weak();
    ui.global::<PageAccountLogic>().on_refresh_account(move || {
        let Some(ui) = ui_weak_refresh.upgrade() else {
            return;
        };
        let pal = ui.global::<PageAccountLogic>();
        let idx = pal.get_selected_index();
        if idx < 0 {
            return;
        }

        let mut accounts: Vec<AccountRow> = pal.get_accounts().iter().collect();
        let mut row = accounts[idx as usize].clone();

        if row.authenticator == "Offline" {
            row.status = "Ready".into();
            accounts[idx as usize] = row.clone();
            pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(accounts))));
            if row.checked {
                pal.set_active_account(row);
            }
        } else {
            let ui_weak_async = ui_weak_refresh.clone();
            let username = row.username.to_string();
            tokio::spawn(async move {
                let Ok(Some(session)) = SessionData::load_session() else {
                    return;
                };
                let token = session.minecraft_access_token().clone();
                let api = crate::mc_api::McAction::new().authenticate(&token);
                let ownership = api.check_game_ownership().await.unwrap_or(false);
                let status_text = if ownership { "Online" } else { "Offline" };
                let avatar_path = fetch_avatar_from_mojang(&token, &username, false)
                    .await
                    .map(|(p, _)| p);

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_async.upgrade() {
                        let pal = ui.global::<PageAccountLogic>();
                        let mut accounts: Vec<AccountRow> = pal.get_accounts().iter().collect();
                        if (idx as usize) < accounts.len() {
                            let mut row = accounts[idx as usize].clone();
                            row.status = status_text.into();
                            if let Some(p) =
                                avatar_path.and_then(|p| slint::Image::load_from_path(&p).ok())
                            {
                                row.avatar = p;
                            }
                            accounts[idx as usize] = row.clone();
                            pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(accounts))));
                            if row.checked {
                                pal.set_active_account(row);
                            }
                        }
                    }
                });
            });
        }
    });

    // ── Appearance / Skin Management ─────────────────────────────────────────
    let appearance_win_rc: std::rc::Rc<std::cell::RefCell<Option<AppearanceWindow>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    let ap_rc_manage = appearance_win_rc.clone();
    let active_renderer: std::sync::Arc<
        std::sync::Mutex<Option<crate::skin_renderer::SkinRenderer>>,
    > = std::sync::Arc::new(std::sync::Mutex::new(None));
    let active_renderer_manage = active_renderer.clone();

    let main_ui_weak_for_appearance = ui.as_weak();
    ui.global::<PageAccountLogic>()
        .on_manage_appearance(move || {
            let mut ap_ref = ap_rc_manage.borrow_mut();
            if ap_ref.is_none() {
                if let Ok(ap) = AppearanceWindow::new() {
                    let ap_weak = ap.as_weak();
                    ap.window().on_close_requested(move || {
                        if let Some(ap) = ap_weak.upgrade() {
                            let _ = ap.hide();
                        }
                        slint::CloseRequestResponse::KeepWindowShown
                    });

                    let ap_weak_drag = ap.as_weak();
                    ap.global::<AppearanceLogic>().on_preview_drag_started(move || {
                        if let Some(ap) = ap_weak_drag.upgrade() {
                            let apl = ap.global::<AppearanceLogic>();
                            apl.set_base_yaw(apl.get_preview_yaw());
                            apl.set_base_pitch(apl.get_preview_pitch());
                            
                            #[cfg(target_os = "macos")]
                            {
                                if crate::GLOBAL_CACHE.get("mac_natural_scroll").is_none() {
                                    let mut is_natural = "1";
                                    if let Ok(output) = std::process::Command::new("defaults")
                                        .args(&["read", "-g", "com.apple.swipescrolldirection"])
                                        .output() {
                                        if let Ok(s) = String::from_utf8(output.stdout) {
                                            if s.trim() == "0" {
                                                is_natural = "0";
                                            }
                                        }
                                    }
                                    crate::GLOBAL_CACHE.insert("mac_natural_scroll".to_string(), is_natural.to_string());
                                }
                            }
                        }
                    });

                    let ap_weak_drag2 = ap.as_weak();
                    ap.global::<AppearanceLogic>().on_preview_dragged(move |dx, dy| {
                        if let Some(ap) = ap_weak_drag2.upgrade() {
                            let apl = ap.global::<AppearanceLogic>();
                            let mut multiplier = 1.0;
                            
                            #[cfg(target_os = "macos")]
                            {
                                if let Some(val) = crate::GLOBAL_CACHE.get("mac_natural_scroll") {
                                    if val.value() == "1" {
                                        multiplier = -1.0;
                                    }
                                } else {
                                    multiplier = -1.0;
                                }
                            }
                            
                            let new_yaw = apl.get_base_yaw() - dx * multiplier;
                            let new_pitch = (apl.get_base_pitch() - dy * multiplier).clamp(-90.0, 90.0);
                            apl.set_preview_yaw(new_yaw);
                            apl.set_preview_pitch(new_pitch);
                        }
                    });

                    let ap_timer = ap.as_weak();
                    let active_renderer_timer = active_renderer_manage.clone();
                    let timer = slint::Timer::default();
                    timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(33), move || {
                        if let Some(ap) = ap_timer.upgrade() {
                            let apl = ap.global::<AppearanceLogic>();
                            let time = apl.get_preview_time() + 0.033;
                            apl.set_preview_time(time);
                            
                            if let Ok(guard) = active_renderer_timer.try_lock() {
                                if let Some(renderer) = &*guard {
                                    let yaw = apl.get_preview_yaw();
                                    let pitch = apl.get_preview_pitch();
                                    let slim = apl.get_skin_variant() == "slim";
                                    let buffer = renderer.render(240, 360, yaw.to_radians(), pitch.to_radians(), slim, time);
                                    apl.set_preview_image(slint::Image::from_rgba8(buffer));
                                    apl.set_has_preview(true);
                                }
                            }
                        }
                    });
                    SKIN_TIMER.with(|t| *t.borrow_mut() = Some(timer));

                    let ap_weak_close = ap.as_weak();
                    ap.global::<AppearanceLogic>().on_close_appearance(move || {
                        if let Some(ap) = ap_weak_close.upgrade() {
                            let _ = ap.hide();
                        }
                    });
                    let ap_weak_apply = ap.as_weak();
                    let main_ui_weak_apply = main_ui_weak_for_appearance.clone();
                    ap.global::<AppearanceLogic>().on_apply_appearance(move || {
                        let ap_weak = ap_weak_apply.clone();
                        let token = crate::GLOBAL_CACHE.get("mc_ac_key").map(|r| r.value().clone()).unwrap_or_default();
                        if token.is_empty() { return; }
                        
                        let mut skin_url_to_apply = String::new();
                        let mut variant_to_apply = String::new();
                        let mut cape_id_to_apply = String::new();
                        
                        let mut skin_changed = false;
                        let mut cape_changed = false;
                        
                        if let Some(ap) = ap_weak.upgrade() {
                            let apl = ap.global::<AppearanceLogic>();
                            let selected_skin_url = apl.get_selected_library_skin_url().to_string();
                            let active_skin_url = apl.get_active_skin_url().to_string();
                            let selected_cape_id = apl.get_selected_cape_id().to_string();
                            let active_cape_id = apl.get_active_cape_id().to_string();
                            
                            skin_url_to_apply = selected_skin_url.clone();
                            variant_to_apply = apl.get_skin_variant().to_string();
                            cape_id_to_apply = selected_cape_id.clone();
                            
                            skin_changed = !selected_skin_url.is_empty() && selected_skin_url != active_skin_url;
                            cape_changed = selected_cape_id != active_cape_id;
                            
                            if !skin_changed && !cape_changed { return; }
                            
                            apl.set_is_uploading(true);
                            apl.set_upload_status("套用變更中...".into());
                        }
                        
                        let main_ui_weak = main_ui_weak_apply.clone();
                        let username = crate::mc_token::SessionData::load_session()
                            .ok()
                            .flatten()
                            .map(|s| s.mc_username().clone())
                            .unwrap_or_default();
                            
                        tokio::spawn(async move {
                            let api = crate::mc_api::McAction::new().authenticate(&token);
                            
                            let mut has_error = false;
                            let mut err_msg = String::new();
                            let mut skin_success = false;
                            let mut new_profile = None;
                            
                            if skin_changed {
                                let mut auto_variant = variant_to_apply;
                                let mut is_file = false;
                                let mut file_path = std::path::PathBuf::new();
                                
                                if skin_url_to_apply.starts_with("file://") {
                                    is_file = true;
                                    if let Ok(parsed_url) = url::Url::parse(&skin_url_to_apply) {
                                        if let Ok(path) = parsed_url.to_file_path() {
                                            file_path = path.clone();
                                            if let Ok(bytes) = std::fs::read(&path) {
                                                if let Ok(img) = image::load_from_memory(&bytes) {
                                                    auto_variant = if detect_is_slim(&img) { "slim".to_string() } else { "classic".to_string() };
                                                }
                                            }
                                        }
                                    }
                                } else if let Ok(resp) = reqwest::get(&skin_url_to_apply).await {
                                    if let Ok(bytes) = resp.bytes().await {
                                        if let Ok(img) = image::load_from_memory(&bytes) {
                                            auto_variant = if detect_is_slim(&img) { "slim".to_string() } else { "classic".to_string() };
                                        }
                                    }
                                }
                                
                                let result = if is_file {
                                    api.upload_skin_from_file(&file_path, &auto_variant).await
                                } else {
                                    api.upload_skin_from_url(&skin_url_to_apply, &auto_variant).await
                                };
                                
                                match result {
                                    Ok(()) => skin_success = true,
                                    Err(e) => {
                                        has_error = true;
                                        err_msg.push_str(&format!("皮膚套用失敗：{}\n", e));
                                    }
                                }
                            }
                            
                            if cape_changed {
                                let res = if cape_id_to_apply.is_empty() {
                                    api.hide_cape().await.map(|_| None)
                                } else {
                                    api.set_active_cape(&cape_id_to_apply).await.map(|p| Some(p))
                                };
                                
                                match res {
                                    Ok(Some(p)) => new_profile = Some(p),
                                    Ok(None) => {},
                                    Err(e) => {
                                        has_error = true;
                                        err_msg.push_str(&format!("披風套用失敗：{}\n", e));
                                    }
                                }
                            }
                            
                            let fetch_result = if skin_success && !username.is_empty() {
                                fetch_avatar_from_mojang(&token, &username, false).await
                            } else {
                                None
                            };
                            let avatar_path_opt = fetch_result.as_ref().map(|(p, _)| p.clone());
                            let new_active_url = fetch_result.map(|(_, u)| u);
                            
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ap) = ap_weak.upgrade() {
                                    let apl = ap.global::<AppearanceLogic>();
                                    apl.set_is_uploading(false);
                                    
                                    if has_error {
                                        apl.set_upload_is_error(true);
                                        apl.set_upload_status(err_msg.trim().into());
                                    } else {
                                        apl.set_upload_is_error(false);
                                        apl.set_upload_status("外觀已成功套用！\n（遊戲內可能需要重新登入才會生效）".into());
                                        if skin_changed {
                                            apl.set_active_skin_url(skin_url_to_apply.clone().into());
                                        }
                                        if cape_changed {
                                            apl.set_active_cape_id(cape_id_to_apply.clone().into());
                                        }
                                    }
                                    apl.set_show_result_dialog(true);
                                    
                                    if cape_changed && !has_error {
                                        if let Ok(paths) = crate::mc_paths::McPaths::new() {
                                            let f = paths.capes_dir().join("profile_cache.json");
                                            if let Some(profile) = &new_profile {
                                                let _ = std::fs::write(&f, serde_json::to_string_pretty(profile).unwrap_or_default());
                                            } else {
                                                if let Ok(s) = std::fs::read_to_string(&f) {
                                                    if let Ok(mut p) = serde_json::from_str::<crate::mc_types::McProfile>(&s) {
                                                        for c in &mut p.capes {
                                                            c.state = if c.id == cape_id_to_apply { crate::mc_types::McState::Active } else { crate::mc_types::McState::Inactive };
                                                        }
                                                        let _ = std::fs::write(&f, serde_json::to_string_pretty(&p).unwrap_or_default());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                if let Some(avatar_path) = avatar_path_opt {
                                    if let Some(main_ui) = main_ui_weak.upgrade() {
                                        if let Ok(img) = slint::Image::load_from_path(&avatar_path) {
                                            let pal = main_ui.global::<PageAccountLogic>();
                                            let mut active = pal.get_active_account();
                                            active.avatar = img.clone();
                                            pal.set_active_account(active.clone());
                                            
                                            let mut accounts: Vec<_> = pal.get_accounts().iter().collect();
                                            if let Some(row) = accounts.iter_mut().find(|r| r.username == username) {
                                                row.avatar = img;
                                            }
                                            pal.set_accounts(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(accounts))));
                                        }
                                    }
                                }
                            });
                        });
                    });

                    let ap_weak_select_cape = ap.as_weak();
                    let active_renderer_select = active_renderer_manage.clone();
                    ap.global::<AppearanceLogic>().on_select_cape(move |cape_id| {
                        let cape_id = cape_id.to_string();
                        let ap_weak = ap_weak_select_cape.clone();
                        let renderer_lock = active_renderer_select.clone();
                        
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ap) = ap_weak.upgrade() {
                                let apl = ap.global::<AppearanceLogic>();
                                apl.set_selected_cape_id(cape_id.clone().into());
                                
                                if cape_id.is_empty() {
                                    if let Ok(mut guard) = renderer_lock.lock() {
                                        if let Some(r) = guard.as_mut() {
                                            r.set_cape(None);
                                        }
                                    }
                                    apl.set_selected_cape_name("無披風".into());
                                    apl.set_selected_cape_preview(Default::default());
                                } else {
                                    // Find cape URL and name
                                    let capes = apl.get_capes();
                                    let mut url = String::new();
                                    let mut cape_name = cape_id.clone();
                                    for i in 0..capes.row_count() {
                                        if let Some(c) = capes.row_data(i) {
                                            if c.id == cape_id {
                                                url = c.url.to_string();
                                                cape_name = if !c.alias.to_string().is_empty() {
                                                    c.alias.to_string()
                                                } else {
                                                    c.id.to_string()
                                                };
                                                break;
                                            }
                                        }
                                    }
                                    apl.set_selected_cape_name(cape_name.into());
                                    
                                    if !url.is_empty() {
                                        let renderer_lock2 = renderer_lock.clone();
                                        let cape_id_clone = cape_id.clone();
                                        let ap_weak2 = ap.as_weak();
                                        tokio::spawn(async move {
                                            let mut cape_bytes = None;
                                            if let Ok(paths) = crate::mc_paths::McPaths::new() {
                                                let cache_file = paths.capes_dir().join(format!("{}.png", cape_id_clone));
                                                if cache_file.exists() {
                                                    cape_bytes = std::fs::read(&cache_file).ok();
                                                } else {
                                                    if let Ok(resp) = reqwest::get(&url).await {
                                                        if let Ok(bytes) = resp.bytes().await {
                                                            let _ = std::fs::write(&cache_file, &bytes);
                                                            cape_bytes = Some(bytes.to_vec());
                                                        }
                                                    }
                                                }
                                            }
                                            if let Some(bytes) = cape_bytes {
                                                if let Ok(img) = image::load_from_memory(&bytes) {
                                                    let (raw_pixels, w, h) = create_cape_preview_raw(&img);
                                                    let _ = slint::invoke_from_event_loop(move || {
                                                        if let Ok(mut guard) = renderer_lock2.lock() {
                                                            if let Some(r) = guard.as_mut() {
                                                                r.set_cape(Some(img));
                                                            }
                                                        }
                                                        if let Some(ap) = ap_weak2.upgrade() {
                                                            let slint_img = slint::Image::from_rgba8(
                                                                slint::SharedPixelBuffer::clone_from_slice(
                                                                    &raw_pixels,
                                                                    w,
                                                                    h,
                                                                )
                                                            );
                                                            ap.global::<AppearanceLogic>()
                                                                .set_selected_cape_preview(slint_img);
                                                        }
                                                    });
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                        });
                    });


                    let ap_weak_delete = ap.as_weak();
                    ap.global::<AppearanceLogic>().on_delete_skin(move |id| {
                        if let Ok(paths) = crate::mc_paths::McPaths::new() {
                            let history_file = paths.skins_history_file();
                            let mut history = crate::skin_history::SkinHistory::load(&history_file);
                            history.skins.retain(|s| s.name != id.as_str());
                            let _ = history.save(&history_file);
                            
                            if let Some(ap) = ap_weak_delete.upgrade() {
                                let ui_skins = get_ui_skins(&paths, &history);
                                ap.global::<AppearanceLogic>().set_skin_history(
                                    slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(ui_skins)))
                                );
                            }
                        }
                    });

                    let ap_weak_browse = ap.as_weak();
                    let active_renderer_browse = active_renderer_manage.clone();
                    ap.global::<AppearanceLogic>().on_browse_skin_file(move || {
                        let result = rfd::FileDialog::new()
                            .add_filter("PNG Image", &["png"])
                            .set_title("Select Skin File")
                            .pick_file();
                        if let Some(path) = result {
                            let path_str = path.to_string_lossy().to_string();
                            if let Some(ap) = ap_weak_browse.upgrade() {
                                let apl = ap.global::<AppearanceLogic>();
                                match image::open(&path) {
                                    Ok(img) => {
                                        use image::GenericImageView;
                                        let (w, h) = img.dimensions();
                                        if w == 64 && (h == 64 || h == 32) {
                                            apl.set_selected_skin_path(path_str.into());
                                            let is_slim = detect_is_slim(&img);
                                            if let Ok(mut guard) = active_renderer_browse.lock() {
                                                let old_cape = guard.as_ref().and_then(|r| r.get_cape());
                                                let mut new_renderer = crate::skin_renderer::SkinRenderer::new(img);
                                                new_renderer.set_cape_rgba(old_cape);
                                                *guard = Some(new_renderer);
                                            }
                                            apl.set_skin_variant(if is_slim { "slim".into() } else { "classic".into() });
                                            apl.set_has_preview(true);
                                        } else {
                                            apl.set_upload_status(format!("無效的皮膚尺寸 ({}x{})，必須是 64x64 或 64x32", w, h).into());
                                            apl.set_upload_is_error(true);
                                            apl.set_show_result_dialog(true);
                                            apl.set_has_preview(false);
                                            apl.set_selected_skin_path("".into());
                                        }
                                    }
                                    Err(_) => {
                                        apl.set_upload_status("無法讀取圖片檔案".into());
                                        apl.set_upload_is_error(true);
                                        apl.set_show_result_dialog(true);
                                        apl.set_has_preview(false);
                                        apl.set_selected_skin_path("".into());
                                    }
                                }
                            }
                        }
                    });

                    let ap_weak_save = ap.as_weak();
                    ap.global::<AppearanceLogic>().on_save_to_library(move || {
                        let Some(ap) = ap_weak_save.upgrade() else { return };
                        let apl = ap.global::<AppearanceLogic>();
                        let name = apl.get_new_skin_name().to_string();
                        let path_str = apl.get_selected_skin_path().to_string();
                        let url_str = apl.get_skin_url().to_string();
                        let variant = apl.get_skin_variant().to_string();
                        
                        let name = if name.is_empty() { "未命名外觀".to_string() } else { name };
                        
                        apl.set_upload_status("正在加入皮膚庫...".into());
                        apl.set_upload_is_error(false);
                        apl.set_is_uploading(true);

                        let ap_weak_async = ap.as_weak();
                        tokio::spawn(async move {
                            let mut skin_bytes = Vec::new();
                            let mut is_error = false;
                            let mut status = "加入成功！".to_string();

                            if !path_str.is_empty() {
                                if let Ok(bytes) = std::fs::read(&path_str) {
                                    skin_bytes = bytes;
                                } else {
                                    is_error = true;
                                    status = "無法讀取本地檔案".to_string();
                                }
                            } else if !url_str.is_empty() {
                                if let Ok(resp) = reqwest::get(&url_str).await {
                                    if let Ok(bytes) = resp.bytes().await {
                                        skin_bytes = bytes.to_vec();
                                    } else {
                                        is_error = true;
                                        status = "下載皮膚失敗".to_string();
                                    }
                                } else {
                                    is_error = true;
                                    status = "無法連線至網址".to_string();
                                }
                            } else {
                                is_error = true;
                                status = "沒有選擇檔案或網址".to_string();
                            }

                            if !is_error && !skin_bytes.is_empty() {
                                if let Ok(paths) = crate::mc_paths::McPaths::new() {
                                    let history_file = paths.skins_history_file();
                                    let mut history = crate::skin_history::SkinHistory::load(&history_file);
                                    
                                    use sha1::Digest;
                                    let mut hash = String::new();
                                    if let Ok(img) = image::load_from_memory(&skin_bytes) {
                                        let mut hasher = sha1::Sha1::new();
                                        hasher.update(img.to_rgba8().into_raw());
                                        hash = hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();
                                    } else {
                                        let mut hasher = sha1::Sha1::new();
                                        hasher.update(&skin_bytes);
                                        hash = hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();
                                    }
                                    
                                    let target_path = paths.skins_dir().join(format!("{}.png", hash));
                                    let render_path = paths.skins_dir().join(format!("{}_render.png", hash));
                                    
                                    let _ = std::fs::write(&target_path, &skin_bytes);
                                    if let Ok(img) = image::load_from_memory(&skin_bytes) {
                                        let is_slim = variant == "slim";
                                        let render_img = crate::view::generate_2d_front(&img, is_slim);
                                        let _ = render_img.save(&render_path);
                                    }
                                    
                                    let final_url = format!("file://{}", target_path.display());
                                    
                                    history.skins.retain(|s| {
                                        let s_hash = s.url.split('/').last().unwrap_or("").trim_end_matches(".png");
                                        s_hash != hash
                                    });
                                    
                                    history.add_skin(crate::skin_history::SkinEntry {
                                        cape_id: "".to_string(),
                                        model: variant.clone(),
                                        name: name.clone(),
                                        url: if url_str.is_empty() { final_url } else { url_str.clone() },
                                    });
                                    let _ = history.save(&history_file);
                                }
                            }

                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ap) = ap_weak_async.upgrade() {
                                    let apl = ap.global::<AppearanceLogic>();
                                    apl.set_upload_status(status.into());
                                    apl.set_upload_is_error(is_error);
                                    apl.set_is_uploading(false);
                                    
                                    if !is_error {
                                        if let Ok(paths) = crate::mc_paths::McPaths::new() {
                                            let history_file = paths.skins_history_file();
                                            let history = crate::skin_history::SkinHistory::load(&history_file);
                                            let ui_skins = get_ui_skins(&paths, &history);
                                            apl.set_skin_history(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(ui_skins))));
                                        }
                                    } else {
                                        apl.set_show_result_dialog(true);
                                    }
                                }
                            });
                        });
                    });

                    let active_renderer_preview = active_renderer_manage.clone();
                    let ap_weak_preview = ap.as_weak();
                    ap.global::<AppearanceLogic>().on_preview_library_skin(move |url| {
                        let url_str = url.to_string();
                        let active_renderer = active_renderer_preview.clone();
                        let ap_weak = ap_weak_preview.clone();
                        tokio::spawn(async move {
                            let mut skin_bytes = None;
                            if url_str.starts_with("file://") {
                                if let Ok(parsed_url) = url::Url::parse(&url_str) {
                                    if let Ok(path) = parsed_url.to_file_path() {
                                        skin_bytes = std::fs::read(&path).ok();
                                    }
                                }
                            } else if !url_str.is_empty() {
                                if let Ok(resp) = reqwest::get(&url_str).await {
                                    skin_bytes = resp.bytes().await.map(|b| b.to_vec()).ok();
                                }
                            }

                            if let Some(bytes) = skin_bytes {
                                if let Ok(img) = image::load_from_memory(&bytes) {
                                    if let Ok(mut guard) = active_renderer.lock() {
                                        let old_cape = guard.as_ref().and_then(|r| r.get_cape());
                                        let mut new_renderer = crate::skin_renderer::SkinRenderer::new(img);
                                        new_renderer.set_cape_rgba(old_cape);
                                        *guard = Some(new_renderer);
                                    }
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(ap) = ap_weak.upgrade() {
                                            ap.global::<AppearanceLogic>().set_has_preview(true);
                                        }
                                    });
                                }
                            }
                        });
                    });

                    let main_ui_weak_inner = main_ui_weak_for_appearance.clone();
                    let ap_weak_upload = ap.as_weak();
                    let main_ui_weak_upload = main_ui_weak_inner.clone();
                    ap.global::<AppearanceLogic>().on_upload_from_file(move || {
                        let Some(ap) = ap_weak_upload.upgrade() else { return };
                        let apl = ap.global::<AppearanceLogic>();
                        let path_str = apl.get_selected_skin_path().to_string();
                        let variant = apl.get_skin_variant().to_string();
                        let token = crate::GLOBAL_CACHE.get("mc_ac_key").map(|v| v.clone()).unwrap_or_default();

                        if path_str.is_empty() {
                            apl.set_upload_status("請先選擇皮膚檔案".into());
                            apl.set_upload_is_error(true);
                            return;
                        }
                        if token.is_empty() {
                            apl.set_upload_status("請先登入 Microsoft 帳號".into());
                            apl.set_upload_is_error(true);
                            return;
                        }

                        apl.set_upload_status("正在上傳...".into());
                        apl.set_upload_is_error(false);
                        apl.set_is_uploading(true);

                        let ap_weak_async = ap.as_weak();
                        let main_ui_weak = main_ui_weak_upload.clone();
                        let username = crate::mc_token::SessionData::load_session()
                            .ok()
                            .flatten()
                            .map(|s| s.mc_username().clone())
                            .unwrap_or_default();
                            
                        tokio::spawn(async move {
                            let api = crate::mc_api::McAction::new().authenticate(&token);
                            let result = api.upload_skin_from_file(std::path::Path::new(&path_str), &variant).await;
                            let (status, is_error) = match result {
                                Ok(()) => ("上傳成功！".to_string(), false),
                                Err(e) => (format!("上傳失敗：{e}"), true),
                            };
                            
                            // Re-fetch avatar directly from Mojang (no CDN delay)
                            let fetch_result: Option<(std::path::PathBuf, String)> = if !is_error && !username.is_empty() {
                                fetch_avatar_from_mojang(&token, &username, false).await
                            } else {
                                None
                            };
                            let avatar_path_opt = fetch_result.as_ref().map(|(p, _)| p.clone());
                            let new_active_url = fetch_result.map(|(_, u)| u);

                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ap) = ap_weak_async.upgrade() {
                                    let apl = ap.global::<AppearanceLogic>();
                                    apl.set_upload_status(status.into());
                                    apl.set_upload_is_error(is_error);
                                    apl.set_is_uploading(false);

                                    if let Some(url) = new_active_url.clone() {
                                        apl.set_active_skin_url(url.into());
                                    }
                                    if !is_error {
                                        if let Ok(paths) = crate::mc_paths::McPaths::new() {
                                            let history_file = paths.skins_history_file();
                                            let history = crate::skin_history::SkinHistory::load(&history_file);
                                            let ui_skins = get_ui_skins(&paths, &history);
                                            apl.set_skin_history(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(ui_skins))));
                                        }
                                    }
                                }
                                
                                if let Some(avatar_path) = avatar_path_opt {
                                    if let Some(main_ui) = main_ui_weak.upgrade() {
                                        if let Ok(img) = slint::Image::load_from_path(&avatar_path) {
                                            let pal = main_ui.global::<PageAccountLogic>();
                                            let mut active = pal.get_active_account();
                                            active.avatar = img.clone();
                                            pal.set_active_account(active.clone());
                                            
                                            let mut accounts: Vec<_> = pal.get_accounts().iter().collect();
                                            if let Some(row) = accounts.iter_mut().find(|r| r.username == username) {
                                                row.avatar = img;
                                            }
                                            pal.set_accounts(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(accounts))));
                                        }
                                    }
                                }
                            });
                        });
                    });

                    let ap_weak_url = ap.as_weak();
                    let main_ui_weak_url = main_ui_weak_inner.clone();
                    ap.global::<AppearanceLogic>().on_upload_from_url(move |url| {
                        let Some(ap) = ap_weak_url.upgrade() else { return };
                        let apl = ap.global::<AppearanceLogic>();
                        let url_str = url.to_string();
                        let variant = apl.get_skin_variant().to_string();
                        let token = crate::GLOBAL_CACHE.get("mc_ac_key").map(|v| v.clone()).unwrap_or_default();

                        if url_str.is_empty() {
                            apl.set_upload_status("請輸入皮膚 URL".into());
                            apl.set_upload_is_error(true);
                            return;
                        }
                        if token.is_empty() {
                            apl.set_upload_status("請先登入 Microsoft 帳號".into());
                            apl.set_upload_is_error(true);
                            return;
                        }

                        apl.set_upload_status("正在套用...".into());
                        apl.set_upload_is_error(false);
                        apl.set_is_uploading(true);

                        let ap_weak_async = ap.as_weak();
                        let main_ui_weak = main_ui_weak_url.clone();
                        let username = crate::mc_token::SessionData::load_session()
                            .ok()
                            .flatten()
                            .map(|s| s.mc_username().clone())
                            .unwrap_or_default();
                            
                        tokio::spawn(async move {
                            let mut auto_variant = variant;
                            let mut is_file = false;
                            let mut file_path = std::path::PathBuf::new();
                            
                            if url_str.starts_with("file://") {
                                is_file = true;
                                if let Ok(parsed_url) = url::Url::parse(&url_str) {
                                    if let Ok(path) = parsed_url.to_file_path() {
                                        file_path = path.clone();
                                        if let Ok(bytes) = std::fs::read(&path) {
                                            if let Ok(img) = image::load_from_memory(&bytes) {
                                                auto_variant = if detect_is_slim(&img) { "slim".to_string() } else { "classic".to_string() };
                                            }
                                        }
                                    }
                                }
                            } else if let Ok(resp) = reqwest::get(&url_str).await {
                                if let Ok(bytes) = resp.bytes().await {
                                    if let Ok(img) = image::load_from_memory(&bytes) {
                                        auto_variant = if detect_is_slim(&img) { "slim".to_string() } else { "classic".to_string() };
                                    }
                                }
                            }
                            
                            let api = crate::mc_api::McAction::new().authenticate(&token);
                            let result = if is_file {
                                api.upload_skin_from_file(&file_path, &auto_variant).await
                            } else {
                                api.upload_skin_from_url(&url_str, &auto_variant).await
                            };
                            let (status, is_error) = match result {
                                Ok(()) => ("套用成功！".to_string(), false),
                                Err(e) => (format!("套用失敗：{e}"), true),
                            };
                            
                            // Re-fetch avatar directly from Mojang (no CDN delay)
                            let fetch_result: Option<(std::path::PathBuf, String)> = if !is_error && !username.is_empty() {
                                fetch_avatar_from_mojang(&token, &username, false).await
                            } else {
                                None
                            };
                            let avatar_path_opt = fetch_result.as_ref().map(|(p, _)| p.clone());
                            let new_active_url = fetch_result.map(|(_, u)| u);

                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ap) = ap_weak_async.upgrade() {
                                    let apl = ap.global::<AppearanceLogic>();
                                    apl.set_skin_variant(auto_variant.into());
                                    apl.set_upload_status(status.into());
                                    apl.set_upload_is_error(is_error);
                                    apl.set_is_uploading(false);
                                    apl.set_show_result_dialog(true);

                                    if let Some(url) = new_active_url.clone() {
                                        apl.set_active_skin_url(url.into());
                                    }
                                    if !is_error {
                                        if let Ok(paths) = crate::mc_paths::McPaths::new() {
                                            let history_file = paths.skins_history_file();
                                            let history = crate::skin_history::SkinHistory::load(&history_file);
                                            let ui_skins = get_ui_skins(&paths, &history);
                                            apl.set_skin_history(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(ui_skins))));
                                        }
                                    }
                                }
                                
                                if let Some(avatar_path) = avatar_path_opt {
                                    if let Some(main_ui) = main_ui_weak.upgrade() {
                                        if let Ok(img) = slint::Image::load_from_path(&avatar_path) {
                                            let pal = main_ui.global::<PageAccountLogic>();
                                            let mut active = pal.get_active_account();
                                            active.avatar = img.clone();
                                            pal.set_active_account(active.clone());
                                            
                                            let mut accounts: Vec<_> = pal.get_accounts().iter().collect();
                                            if let Some(row) = accounts.iter_mut().find(|r| r.username == username) {
                                                row.avatar = img;
                                            }
                                            pal.set_accounts(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(accounts))));
                                        }
                                    }
                                }
                            });
                        });
                    });

                    *ap_ref = Some(ap);
                }
            }

            if let Some(ap) = ap_ref.as_ref() {
                let apl = ap.global::<AppearanceLogic>();
                apl.set_upload_status("".into());
                apl.set_upload_is_error(false);
                apl.set_skin_url("".into());
                apl.set_selected_skin_path("".into());
                apl.set_skin_variant("classic".into());
                apl.set_is_uploading(false);
                apl.set_has_preview(false);

                if let Ok(paths) = crate::mc_paths::McPaths::new() {
                    let history_file = paths.skins_history_file();
                    let history = crate::skin_history::SkinHistory::load(&history_file);
                    let ui_skins = get_ui_skins(&paths, &history);
                    apl.set_skin_history(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(ui_skins))));
                }

                let token = crate::GLOBAL_CACHE.get("mc_ac_key").map(|v| v.clone()).unwrap_or_default();
                if !token.is_empty() {
                    let active_renderer_spawn = active_renderer_manage.clone();
                    let ap_weak = ap.as_weak();
                    tokio::spawn(async move {
                        let api = crate::mc_api::McAction::new().authenticate(&token);
                        
                        let mut profile_cache_file = None;
                        let mut cached_profile = None;
                        if let Ok(paths) = crate::mc_paths::McPaths::new() {
                            let f = paths.capes_dir().join("profile_cache.json");
                            if let Ok(s) = std::fs::read_to_string(&f) {
                                cached_profile = serde_json::from_str::<crate::mc_types::McProfile>(&s).ok();
                            }
                            profile_cache_file = Some(f);
                        }
                        
                        let mut profile_opt = api.get_user_profile().await.ok();
                        
                        if let Some(p) = &profile_opt {
                            if let Some(f) = &profile_cache_file {
                                let _ = std::fs::write(f, serde_json::to_string_pretty(p).unwrap_or_default());
                            }
                        } else if let Some(p) = cached_profile {
                            profile_opt = Some(p); // use cache if API fails
                        }
                        
                        if let Some(profile) = profile_opt {
                            let mut active_skin_url = String::new();
                            for skin in &profile.skins {
                                if skin.state == crate::mc_types::McState::Active {
                                    active_skin_url = skin.url.clone();
                                }
                            }
                            struct TempCape {
                                id: String,
                                alias: String,
                                state: crate::mc_types::McState,
                                url: String,
                                buffer: Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>>,
                            }
                            let mut capes_temp = Vec::new();
                            for cape in &profile.capes {
                                let mut buffer_opt = None;
                                if let Ok(paths) = crate::mc_paths::McPaths::new() {
                                    let cache_file = paths.capes_dir().join(format!("{}.png", cape.id));
                                    let mut cape_bytes = None;
                                    if cache_file.exists() {
                                        cape_bytes = std::fs::read(&cache_file).ok();
                                    } else {
                                        if let Ok(resp) = reqwest::get(&cape.url).await {
                                            if let Ok(bytes) = resp.bytes().await {
                                                let _ = std::fs::write(&cache_file, &bytes);
                                                cape_bytes = Some(bytes.to_vec());
                                            }
                                        }
                                    }
                                    if let Some(bytes) = cape_bytes {
                                        if let Ok(img) = image::load_from_memory(&bytes) {
                                            let (raw, w, h) = create_cape_preview_raw(&img);
                                            buffer_opt = Some(slint::SharedPixelBuffer::clone_from_slice(&raw, w, h));
                                        }
                                    }
                                }
                                capes_temp.push(TempCape {
                                    id: cape.id.clone(),
                                    alias: cape.alias.clone(),
                                    state: cape.state.clone(),
                                    url: cape.url.clone(),
                                    buffer: buffer_opt,
                                });
                            }
                            let has_capes = !capes_temp.is_empty();
                            let mut active_cape_id = String::new();
                            for cape in &profile.capes {
                                if cape.state == crate::mc_types::McState::Active {
                                    active_cape_id = cape.id.clone();
                                }
                            }


                            let active_url_clone = active_skin_url.clone();
                            let ap_weak_1 = ap_weak.clone();
                            let active_cape_id_clone = active_cape_id.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ap) = ap_weak_1.upgrade() {
                                    let apl = ap.global::<AppearanceLogic>();
                                    apl.set_active_skin_url(active_url_clone.into());
                                    
                                    let mut capes_ui = Vec::new();
                                    let mut active_cape_name = String::from("無披風");
                                    let mut active_cape_preview = slint::Image::default();
                                    
                                    for t in capes_temp {
                                        let preview = match t.buffer {
                                            Some(ref b) => slint::Image::from_rgba8(b.clone()),
                                            None => slint::Image::default(),
                                        };
                                        if t.id == active_cape_id_clone {
                                            active_cape_name = if !t.alias.is_empty() { t.alias.clone() } else { t.id.clone() };
                                            active_cape_preview = preview.clone();
                                        }
                                        capes_ui.push(CapeData {
                                            id: t.id.into(),
                                            alias: t.alias.into(),
                                            state: if t.state == crate::mc_types::McState::Active { "ACTIVE".into() } else { "INACTIVE".into() },
                                            url: t.url.into(),
                                            preview,
                                        });
                                    }
                                    
                                    apl.set_capes(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(capes_ui))));
                                    apl.set_has_capes(has_capes);
                                    apl.set_active_cape_id(active_cape_id_clone.clone().into());
                                    apl.set_selected_cape_id(active_cape_id_clone.into());
                                    apl.set_selected_cape_name(active_cape_name.into());
                                    apl.set_selected_cape_preview(active_cape_preview);
                                }
                            });
                            
                                if let Ok(resp) = reqwest::get(&active_skin_url).await {
                                    if let Ok(bytes) = resp.bytes().await {
                                        if let Ok(img) = image::load_from_memory(&bytes) {
                                            let is_slim = detect_is_slim(&img);
                                            
                                            // Fetch initial cape image
                                            let mut cape_img = None;
                                            if !active_cape_id.is_empty() {
                                                for cape in &profile.capes {
                                                    if cape.id == active_cape_id {
                                                        if let Ok(paths) = crate::mc_paths::McPaths::new() {
                                                            let cache_file = paths.capes_dir().join(format!("{}.png", cape.id));
                                                            let mut cape_bytes = None;
                                                            if cache_file.exists() {
                                                                cape_bytes = std::fs::read(&cache_file).ok();
                                                            } else {
                                                                if let Ok(resp) = reqwest::get(&cape.url).await {
                                                                    if let Ok(bytes) = resp.bytes().await {
                                                                        let _ = std::fs::write(&cache_file, &bytes);
                                                                        cape_bytes = Some(bytes.to_vec());
                                                                    }
                                                                }
                                                            }
                                                            if let Some(bytes) = cape_bytes {
                                                                if let Ok(cimg) = image::load_from_memory(&bytes) {
                                                                    cape_img = Some(cimg);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            
                                            let _ = slint::invoke_from_event_loop(move || {
                                                if let Ok(mut guard) = active_renderer_spawn.lock() {
                                                    let mut renderer = crate::skin_renderer::SkinRenderer::new(img);
                                                    renderer.set_cape(cape_img);
                                                    *guard = Some(renderer);
                                                }
                                                if let Some(ap) = ap_weak.upgrade() {
                                                    let apl = ap.global::<AppearanceLogic>();
                                                    apl.set_skin_variant(if is_slim { "slim".into() } else { "classic".into() });
                                                    apl.set_has_preview(true);
                                                }
                                            });
                                        }
                                    }
                                }
                        }
                    });
                }

                let _ = ap.show();
            }
        });

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
