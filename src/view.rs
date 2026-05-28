slint::include_modules!();
use crate::{
    mc_install,
    mc_instance::{InstanceConfig, InstanceStore},
    mc_parser::LaunchContext,
    mc_paths::McPaths,
    mc_token::{self, SessionData},
    mc_types::McSpecificVersionDetail,
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

async fn fetch_avatar_path(username: &str) -> Option<std::path::PathBuf> {
    let cache_dir = std::env::temp_dir().join("voxelruler_avatars");
    let _ = std::fs::create_dir_all(&cache_dir);
    let avatar_path = cache_dir.join(format!("{}.png", username));

    if avatar_path.exists() {
        return Some(avatar_path);
    }

    let url = format!("https://minotar.net/helm/{}/100.png", username);
    if let Ok(resp) = reqwest::get(url).await
        && let Ok(bytes) = resp.bytes().await
        && std::fs::write(&avatar_path, bytes).is_ok()
    {
        return Some(avatar_path);
    }
    None
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

    let ui_weak_for_versions = ui.as_weak();
    {
        if let Some(ui) = ui_weak_for_versions.upgrade() {
            ui.global::<InstanceCreateLogic>().set_is_loading(true);
        }
    }
    tokio::spawn(async move {
        let api = crate::mc_api::McAction::new();
        match api.get_all_mc_versions().await {
            Ok(versions) => {
                let list: Vec<String> = versions.iter().map(|v| v.id.clone()).collect();
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_for_versions.upgrade() {
                        let create = ui.global::<InstanceCreateLogic>();
                        let shared: Vec<slint::SharedString> =
                            list.iter().map(|s| s.as_str().into()).collect();
                        let first = shared.first().cloned().unwrap_or_default();
                        create.set_version_list(ModelRc::from(Rc::new(VecModel::from(shared))));
                        create.set_selected_version(first);
                        create.set_is_loading(false);
                    }
                })
                .ok();
            }
            Err(e) => {
                eprintln!("Failed to fetch MC versions: {e}");
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_for_versions.upgrade() {
                        ui.global::<InstanceCreateLogic>().set_is_loading(false);
                    }
                })
                .ok();
            }
        }
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
        let (version_id, instance_id, instance_name, xmx, xms) = {
            let configs = master_for_launch.lock().unwrap();
            if let Some(c) = configs.iter().find(|c| c.id == id.as_str()) {
                (
                    c.version.clone(),
                    c.id.clone(),
                    c.name.clone(),
                    c.xmx.clone(),
                    c.xms.clone(),
                )
            } else {
                (
                    id.to_string(),
                    id.to_string(),
                    id.to_string(),
                    "2G".into(),
                    "512M".into(),
                )
            }
        };
        let running_procs = Arc::clone(&running_procs_for_launch);
        let ui_weak = ui_weak_for_launch.clone();
        let logs = Arc::clone(&instance_logs_for_launch);
        if running_procs.lock().unwrap().contains_key(&instance_id) {
            return;
        }
        tokio::spawn(async move {
            match do_launch(
                version_id,
                instance_id.clone(),
                instance_name.clone(),
                xmx,
                xms,
                ui_weak.clone(),
                logs,
            )
            .await
            {
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

    logic.on_open_instance_settings(move |_id| {
        // TODO: 開啟 instance 設定頁面（M4 里程碑實作）
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

                    let avatar_path = fetch_avatar_path(&username).await;

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
                let avatar_path = fetch_avatar_path(&username).await;
                if let Ok(Some(session)) = SessionData::load_session() {
                    let token = session.minecraft_access_token().clone();
                    let api = crate::mc_api::McAction::new().authenticate(&token);
                    let ownership = api.check_game_ownership().await.unwrap_or(false);
                    let status_text = if ownership { "Online" } else { "Offline" };

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
                }
            });
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

async fn do_launch(
    version_id: String,
    instance_id: String,
    instance_name: String,
    xmx: String,
    xms: String,
    ui_weak: slint::Weak<MainApp>,
    instance_logs: Arc<Mutex<HashMap<String, VecDeque<String>>>>,
) -> anyhow::Result<Child> {
    set_install_state(&ui_weak, true, 0.0, "正在取得版本資料...", false);

    let api = crate::mc_api::McAction::new();
    let version = api.get_specific_mc_version_detail(&version_id).await?;
    let java_manifest = api.get_java_runtime_manifest_for_version(&version).await?;

    let paths = McPaths::new()?;
    let java_component = version
        .java_version
        .as_ref()
        .map(|j| j.component.clone())
        .unwrap_or_else(|| "jre-legacy".into());

    info!(java_dir = ?paths.java_dir(&java_component), "開始安裝 Java");
    mc_install::install_java(&java_manifest, &paths.java_dir(&java_component), {
        let ui_weak = ui_weak.clone();
        move |p| {
            let status = format!("下載 Java 執行環境... {:.0}%", p * 100.0);
            set_install_state(&ui_weak, true, 0.1 + p * 0.3, &status, false);
        }
    })
    .await
    .context("安裝 Java 失敗")?;

    info!(versions_dir = ?paths.versions_dir(), "開始安裝 Minecraft 主程式");
    mc_install::install_client(&version, &paths.versions_dir(), {
        let ui_weak = ui_weak.clone();
        move |p| {
            let status = format!("下載 Minecraft 主程式... {:.0}%", p * 100.0);
            set_install_state(&ui_weak, true, 0.4 + p * 0.2, &status, false);
        }
    })
    .await
    .context("安裝 Minecraft 主程式失敗")?;

    info!(libraries_dir = ?paths.libraries_dir(), "開始安裝函式庫");
    mc_install::install_libraries(&version, &paths.libraries_dir(), {
        let ui_weak = ui_weak.clone();
        move |p| {
            let status = format!("下載函式庫... {:.0}%", p * 100.0);
            set_install_state(&ui_weak, true, 0.6 + p * 0.2, &status, false);
        }
    })
    .await
    .context("安裝函式庫失敗")?;

    info!(assets_dir = ?paths.assets_dir(), "開始安裝遊戲資源");
    mc_install::install_assets(&version, &paths.assets_dir(), {
        let ui_weak = ui_weak.clone();
        move |p| {
            let status = format!("下載遊戲資源... {:.0}%", p * 100.0);
            set_install_state(&ui_weak, true, 0.8 + p * 0.2, &status, false);
        }
    })
    .await
    .context("安裝遊戲資源失敗")?;

    set_install_state(&ui_weak, true, 1.0, "啟動遊戲中...", false);

    let token = crate::GLOBAL_CACHE
        .get("mc_ac_key")
        .map(|v| v.clone())
        .unwrap_or_default();

    let (player_name, player_uuid) = if !token.is_empty() {
        match crate::mc_api::McAction::new()
            .authenticate(&token)
            .get_user_profile()
            .await
        {
            Ok(profile) => (profile.name, profile.id),
            Err(_) => (
                "Player".into(),
                "00000000-0000-0000-0000-000000000000".into(),
            ),
        }
    } else {
        (
            "Player".into(),
            "00000000-0000-0000-0000-000000000000".into(),
        )
    };

    let ctx = LaunchContext {
        version,
        java_path: paths.java_bin(&java_component),
        game_dir: paths.instance_dir(&instance_id),
        libraries_dir: paths.libraries_dir(),
        assets_dir: paths.assets_dir(),
        natives_dir: paths.natives_dir(&version_id),
        versions_dir: paths.versions_dir(),
        auth_player_name: player_name,
        auth_uuid: player_uuid,
        auth_access_token: token,
        client_id: String::new(),
        xuid: String::new(),
        xmx,
        xms,
    };
    let mut cmd = ctx.build_command();
    debug!(cmd = ?cmd, java = ?ctx.java_path, game_dir = ?ctx.game_dir, "啟動指令");
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().with_context(|| {
        format!(
            "spawn 失敗，java={:?} game_dir={:?}",
            ctx.java_path, ctx.game_dir
        )
    })?;
    set_install_state(&ui_weak, false, 0.0, "", false);

    instance_logs
        .lock()
        .unwrap()
        .insert(instance_id.clone(), VecDeque::with_capacity(500));

    if let Some(stdout) = child.stdout.take() {
        spawn_log_reader(
            stdout,
            instance_id.clone(),
            Arc::clone(&instance_logs),
            ui_weak.clone(),
        );
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_reader(
            stderr,
            instance_id.clone(),
            Arc::clone(&instance_logs),
            ui_weak.clone(),
        );
    }

    Ok(child)
}

fn spawn_log_reader<R: std::io::Read + Send + 'static>(
    reader: R,
    instance_id: String,
    instance_logs: Arc<Mutex<HashMap<String, VecDeque<String>>>>,
    ui_weak: slint::Weak<MainApp>,
) {
    tokio::task::spawn_blocking(move || {
        use std::io::BufRead;
        let buf = std::io::BufReader::new(reader);
        for line in buf.lines().map_while(Result::ok) {
            debug!(instance = %instance_id, "[Java] {}", line);
            {
                let mut logs = instance_logs.lock().unwrap();
                if let Some(deque) = logs.get_mut(&instance_id) {
                    if deque.len() >= 500 {
                        deque.pop_front();
                    }
                    deque.push_back(line.clone());
                }
            }
            let id = instance_id.clone();
            let line_shared: slint::SharedString = line.into();
            let ui = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui_handle) = ui.upgrade() else {
                    return;
                };
                let logic = ui_handle.global::<InstanceLogic>();
                if logic.get_show_log() && logic.get_log_instance_id().as_str() == id {
                    let current = logic.get_log_lines();
                    let mut lines: Vec<slint::SharedString> = (0..current.row_count())
                        .filter_map(|i| current.row_data(i))
                        .collect();
                    lines.push(line_shared);
                    logic.set_log_lines(ModelRc::from(Rc::new(VecModel::from(lines))));
                }
            });
        }
    });
}
