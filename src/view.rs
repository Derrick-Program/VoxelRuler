slint::include_modules!();
use slint::{Model, ModelRc, VecModel};
use std::{path::{Path, PathBuf}, rc::Rc, sync::{Arc, atomic::{AtomicBool, Ordering}}};

use crate::{mc_parser::LaunchContext, mc_token, mc_types::McSpecificVersionDetail};
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
    logic.on_launch_instance(|id| {
        println!("Rust: 正在啟動實例 ID: {}", id);
        let data = std::fs::read_to_string("data/1.20.4.json").expect("找不到 data/1.20.4.json");
        let version: McSpecificVersionDetail =
            serde_json::from_str(&data).expect("解析 1.20.4.json 失敗");
        let temp = LaunchContext {
            version,
            java_path: PathBuf::from("/usr/bin/java"),
            game_dir: PathBuf::from("/game"),
            libraries_dir: PathBuf::from("/libs"),
            assets_dir: PathBuf::from("/assets"),
            natives_dir: PathBuf::from("/natives"),
            versions_dir: PathBuf::from("/versions"),
            auth_player_name: "Steve".into(),
            auth_uuid: "uuid-1234".into(),
            auth_access_token: "token-abcd".into(),
            client_id: "".into(),
            xuid: "".into(),
            xmx: "2G".into(),
            xms: "512M".into(),
        };
        let cmd = temp.build_command();
        println!("啟動指令：{:#?}", cmd);
        // 在這裡呼叫 Command::new("java")...
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
