slint::include_modules!();
mod mc_token;
mod mc_action;
use slint::{ModelRc, VecModel};
use std::{path::Path, rc::Rc};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pdir = directories::ProjectDirs::from("com", "Duacodie", "VoxelRuler").unwrap();
    dbg!(pdir);
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
        // 在這裡呼叫 Command::new("java")...
    });

    logic.on_new_instance(|| {
        println!("Rust: 開啟建立視窗");
    });
    slint::select_bundled_translation("zh_TW").unwrap();
    ui.run()?;
    // mc_token::get_minecraft_token().await?;
    Ok(())
}
