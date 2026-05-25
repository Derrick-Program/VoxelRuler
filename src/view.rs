slint::include_modules!();
use slint::{Model, ModelRc, VecModel};
use std::{collections::{HashMap, VecDeque}, path::{Path, PathBuf}, process::Child, rc::Rc, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}};

use crate::{mc_install, mc_parser::LaunchContext, mc_paths::McPaths, mc_token, mc_types::McSpecificVersionDetail};
#[allow(unused)]
pub async fn open_view() -> anyhow::Result<()> {
    let ui = MainApp::new()?;
    let logic = ui.global::<InstanceLogic>();
    let path_buf = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/assets/icons/voxelruler.png");
    let raw_instances = vec![
        InstanceData {
            id: "1".into(),
            name: "生存模式 1.20".into(),
            version: "1.20.4".into(),
            mod_loader: "Fabric".into(),
            status: "ready".into(),
            image: slint::Image::load_from_path(&path_buf)?,
            ..Default::default()
        },
        InstanceData {
            id: "2".into(),
            name: "紅石測試".into(),
            version: "1.19.2".into(),
            mod_loader: "Forge".into(),
            status: "ready".into(),
            image: slint::Image::load_from_path(&path_buf)?,
            last_played: "2024-06-01".into(),
            play_time: "2h 15m".into(),
        },
        InstanceData {
            id: "3".into(),
            name: "模組測試".into(),
            version: "1.18.1".into(),
            mod_loader: "Fabric".into(),
            status: "ready".into(),
            image: slint::Image::load_from_path(&path_buf)?,
            last_played: "2024-05-28".into(),
            play_time: "5h 42m".into(),
        },
        InstanceData {
            id: "4".into(),
            name: "實驗性版本".into(),
            version: "1.20.4".into(),
            mod_loader: "None".into(),
            status: "ready".into(),
            image: slint::Image::load_from_path(&path_buf)?,
            ..Default::default()
        },
        InstanceData {
            id: "5".into(),
            name: "冒險模式".into(),
            version: "1.19.2".into(),
            mod_loader: "Forge".into(),
            status: "ready".into(),
            image: slint::Image::load_from_path(&path_buf)?,
            last_played: "2024-06-02".into(),
            play_time: "3h 20m".into(),
        },
        InstanceData {
            id: "6".into(),
            name: "建築專用".into(),
            version: "1.18.1".into(),
            mod_loader: "Fabric".into(),
            status: "ready".into(),
            image: slint::Image::load_from_path(&path_buf)?,
            last_played: "2024-05-30".into(),
            play_time: "10h 5m".into(),
        },
        InstanceData {
            id: "7".into(),
            name: "生存模式 1.20".into(),
            version: "1.20.4".into(),
            mod_loader: "Fabric".into(),
            status: "ready".into(),
            image: slint::Image::load_from_path(&path_buf)?,
            ..Default::default()
        },
        InstanceData {
            id: "8".into(),
            name: "紅石測試".into(),
            version: "1.19.2".into(),
            mod_loader: "Forge".into(),
            status: "ready".into(),
            image: slint::Image::load_from_path(&path_buf)?,
            last_played: "2024-06-01".into(),
            play_time: "2h 15m".into(),
        },
    ];
    let model = Rc::new(VecModel::from(raw_instances.clone()));
    logic.set_instance_list(ModelRc::from(Rc::clone(&model)));
    let logic_weak = ui.as_weak();
    let raw_data_for_search = raw_instances.clone();
    logic.on_search_changed(move |text| {
        let ui = logic_weak.unwrap();
        let logic = ui.global::<InstanceLogic>();
        let filtered: Vec<InstanceData> = raw_data_for_search
            .iter()
            .filter(|inst| {
                text.is_empty() || inst.name.to_lowercase().contains(&text.to_lowercase())
            })
            .cloned()
            .collect();

        // 更新 UI 的 Model
        logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(filtered))));
    });
    // --- 設定回調 ---
    let running_procs: Arc<Mutex<HashMap<String, Child>>> = Arc::new(Mutex::new(HashMap::new()));
    let instance_logs: Arc<Mutex<HashMap<String, VecDeque<String>>>> = Arc::new(Mutex::new(HashMap::new()));

    let raw_instances_for_launch = raw_instances.clone();
    let running_procs_for_launch = Arc::clone(&running_procs);
    let instance_logs_for_launch = Arc::clone(&instance_logs);
    let ui_weak_for_launch = ui.as_weak();
    logic.on_launch_instance(move |id| {
        let version_id = raw_instances_for_launch
            .iter()
            .find(|inst| inst.id == id)
            .map(|inst| inst.version.to_string())
            .unwrap_or_else(|| id.to_string());
        let instance_id = id.to_string();
        let running_procs = Arc::clone(&running_procs_for_launch);
        let ui_weak = ui_weak_for_launch.clone();
        let logs = Arc::clone(&instance_logs_for_launch);
        if running_procs.lock().unwrap().contains_key(&instance_id) {
            return;
        }
        tokio::spawn(async move {
            match do_launch(version_id, instance_id.clone(), ui_weak.clone(), logs).await {
                Ok(child) => {
                    running_procs.lock().unwrap().insert(instance_id.clone(), child);
                    set_instance_status(&ui_weak, &instance_id, "running");
                    // Watch for process exit and reset status
                    let running_procs_watch = Arc::clone(&running_procs);
                    let ui_weak_watch = ui_weak.clone();
                    let id_watch = instance_id.clone();
                    tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            let mut map = running_procs_watch.lock().unwrap();
                            let Some(child) = map.get_mut(&id_watch) else { break };
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
                    eprintln!("啟動失敗：{e:#}");
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

    logic.on_new_instance(|| {
        println!("Rust: 開啟建立視窗");
    });

    let mod_logic = ui.global::<ModLogic>();
    let raw_mods: Vec<ModData> = mod_logic.get_mod_list().iter().collect();
    let mod_logic_weak = ui.as_weak();
    mod_logic.on_search_changed(move |text| {
        let ui = mod_logic_weak.unwrap();
        let logic = ui.global::<ModLogic>();
        let filtered: Vec<ModData> = raw_mods
            .iter()
            .filter(|m| text.is_empty() || m.name.to_lowercase().contains(text.to_lowercase().as_str()))
            .cloned()
            .collect();
        logic.set_selected_index(0);
        logic.set_mod_list(ModelRc::from(Rc::new(VecModel::from(filtered))));
    });

    let applogic = ui.global::<AppLogic>();
    applogic.on_sidebar_change(|id| {
        println!("Sidebar changed to: {:#?}", id);
    });

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
    page_account_logic.on_login_with_microsoft(move ||{
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
                        pal.set_login_url(url.into()); // 網址塞進 Dialog 超連結！
                        pal.set_login_status_text("請在打開的瀏覽器網頁中完成驗證。".into());
                    }
                });
            };
            match mc_token::set_token_in_native_store(on_url_ready).await {
                Ok(new_token) => {
                    if cancel_flag.load(Ordering::SeqCst) {
                        return;
                    }
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            ui.global::<PageAccountLogic>().set_is_logging_in(false);
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
    // slint::select_bundled_translation("zh_TW").unwrap();
    ui.run()?;
    Ok(())
}

fn set_install_state(ui_weak: &slint::Weak<MainApp>, installing: bool, progress: f32, status: &str, is_error: bool) {
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
            if let Some(mut item) = list.row_data(i) {
                if item.id.as_str() == id {
                    item.status = status.into();
                    list.set_row_data(i, item);
                    break;
                }
            }
        }
    });
}

async fn do_launch(
    version_id: String,
    instance_id: String,
    ui_weak: slint::Weak<MainApp>,
    instance_logs: Arc<Mutex<HashMap<String, VecDeque<String>>>>,
) -> anyhow::Result<Child> {
    set_install_state(&ui_weak, true, 0.0, "正在取得版本資料...", false);

    let api = crate::mc_api::McAction::new();
    let version = api.get_specific_mc_version_detail(&version_id).await?;
    let java_manifest = api.get_java_runtime_manifest_for_version(&version).await?;

    let paths = McPaths::new()?;
    let java_component = version.java_version.as_ref()
        .map(|j| j.component.clone())
        .unwrap_or_else(|| "jre-legacy".into());

    mc_install::install_java(&java_manifest, &paths.java_dir(&java_component), {
        let ui_weak = ui_weak.clone();
        move |p| {
            let status = format!("下載 Java 執行環境... {:.0}%", p * 100.0);
            set_install_state(&ui_weak, true, 0.1 + p * 0.3, &status, false);
        }
    }).await?;

    mc_install::install_client(&version, &paths.versions_dir(), {
        let ui_weak = ui_weak.clone();
        move |p| {
            let status = format!("下載 Minecraft 主程式... {:.0}%", p * 100.0);
            set_install_state(&ui_weak, true, 0.4 + p * 0.2, &status, false);
        }
    }).await?;

    mc_install::install_libraries(&version, &paths.libraries_dir(), {
        let ui_weak = ui_weak.clone();
        move |p| {
            let status = format!("下載函式庫... {:.0}%", p * 100.0);
            set_install_state(&ui_weak, true, 0.6 + p * 0.2, &status, false);
        }
    }).await?;

    mc_install::install_assets(&version, &paths.assets_dir(), {
        let ui_weak = ui_weak.clone();
        move |p| {
            let status = format!("下載遊戲資源... {:.0}%", p * 100.0);
            set_install_state(&ui_weak, true, 0.8 + p * 0.2, &status, false);
        }
    }).await?;

    set_install_state(&ui_weak, true, 1.0, "啟動遊戲中...", false);

    let token = crate::GLOBAL_CACHE
        .get("mc_ac_key")
        .map(|v| v.clone())
        .unwrap_or_default();

    let (player_name, player_uuid) = if !token.is_empty() {
        match crate::mc_api::McAction::new().authenticate(&token).get_user_profile().await {
            Ok(profile) => (profile.name, profile.id),
            Err(_) => ("Player".into(), "00000000-0000-0000-0000-000000000000".into()),
        }
    } else {
        ("Player".into(), "00000000-0000-0000-0000-000000000000".into())
    };

    let ctx = LaunchContext {
        version,
        java_path: paths.java_bin(&java_component),
        game_dir: paths.instance_dir(&version_id),
        libraries_dir: paths.libraries_dir(),
        assets_dir: paths.assets_dir(),
        natives_dir: paths.natives_dir(&version_id),
        versions_dir: paths.versions_dir(),
        auth_player_name: player_name,
        auth_uuid: player_uuid,
        auth_access_token: token,
        client_id: String::new(),
        xuid: String::new(),
        xmx: "2G".into(),
        xms: "512M".into(),
    };

    let mut cmd = ctx.build_command();
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn()?;
    set_install_state(&ui_weak, false, 0.0, "", false);

    instance_logs.lock().unwrap().insert(instance_id.clone(), VecDeque::with_capacity(500));

    if let Some(stdout) = child.stdout.take() {
        spawn_log_reader(stdout, instance_id.clone(), Arc::clone(&instance_logs), ui_weak.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_reader(stderr, instance_id.clone(), Arc::clone(&instance_logs), ui_weak.clone());
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
        for line in buf.lines().flatten() {
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
                let Some(ui_handle) = ui.upgrade() else { return };
                let logic = ui_handle.global::<InstanceLogic>();
                if logic.get_show_log() && logic.get_log_instance_id().as_str() == id {
                    let current = logic.get_log_lines();
                    let mut lines: Vec<slint::SharedString> =
                        (0..current.row_count()).filter_map(|i| current.row_data(i)).collect();
                    lines.push(line_shared);
                    logic.set_log_lines(ModelRc::from(Rc::new(VecModel::from(lines))));
                }
            });
        }
    });
}
