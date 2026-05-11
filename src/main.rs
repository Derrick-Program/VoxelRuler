slint::include_modules!();
mod mc_token;
use std::{path::Path, rc::Rc};
use slint::{ModelRc, VecModel};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ui = MainApp::new()?;
    let logic = ui.global::<InstanceLogic>();
    let path_buf = Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("ui/assets/icons/voxelruler.png");
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
    ];
    let model = Rc::new(VecModel::from(raw_instances.clone()));
    logic.set_instance_list(ModelRc::from(Rc::clone(&model)));
    let logic_weak = ui.as_weak();
    let raw_data_for_search = raw_instances.clone();
    
    logic.on_search_changed(move |text| {
        let ui = logic_weak.unwrap();
        let logic = ui.global::<InstanceLogic>();
        
        // 過濾邏輯
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
    Ok(())
}
