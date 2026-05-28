# Task 04 — View 層重構 (src/view.rs)

> **依賴**：Task 02 + Task 03 完成後執行  
> **預計時間**：15–20 分鐘  
> **分支**：feat/mc/new_instance

---

## 目標

重構 `src/view.rs`：
1. 移除 hardcoded 假資料，改為從 TOML 讀取
2. 以 `Arc<Mutex<Vec<InstanceConfig>>>` 作為 master store
3. 修正 search（從 master store 篩選）
4. 更新 `on_launch_instance` 從 master store 取得 xmx/xms
5. 實作 `on_new_instance`（開啟 dialog）
6. 實作 `on_cancel_create`、`on_confirm_create`
7. 背景取得 MC 版本列表
8. 更新 `do_launch` signature

---

## 步驟

### Step 1：在 view.rs 新增 mc_instance import

找到第 5 行：
```rust
use crate::{mc_install, mc_parser::LaunchContext, mc_paths::McPaths, mc_token, mc_types::McSpecificVersionDetail};
```

改成：
```rust
use crate::{mc_install, mc_instance::{InstanceConfig, InstanceStore}, mc_parser::LaunchContext, mc_paths::McPaths, mc_token, mc_types::McSpecificVersionDetail};
```

### Step 2：在 open_view() 前新增 config_to_ui_data 輔助函式

在 `#[allow(unused)]` 這行之前，插入：

```rust
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
```

### Step 3：替換 open_view() 開頭的 hardcoded instances 區塊

找到從 `let ui = MainApp::new()?;` 開始到 `logic.on_search_changed(...)` 結束的整個區塊（大約 lines 8–105），用以下內容替換：

```rust
    let ui = MainApp::new()?;
    let logic = ui.global::<InstanceLogic>();

    // ── Load instances from TOML ──────────────────────────────────────
    let store = Arc::new(Mutex::new(InstanceStore::new(
        McPaths::new()?.instances_file(),
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

    // ── Background: fetch MC version list ────────────────────────────
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

    // ── Search: filter from master store ─────────────────────────────
    let master_for_search = Arc::clone(&master_configs);
    let ui_weak_for_search = ui.as_weak();
    logic.on_search_changed(move |text| {
        let Some(ui) = ui_weak_for_search.upgrade() else { return };
        let logic = ui.global::<InstanceLogic>();
        let configs = master_for_search.lock().unwrap();
        let filtered: Vec<InstanceData> = configs
            .iter()
            .filter(|c| text.is_empty() || c.name.to_lowercase().contains(&text.to_lowercase()))
            .map(config_to_ui_data)
            .collect();
        logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(filtered))));
    });
```

### Step 4：替換 on_launch_instance 使用 master_configs

找到 `let running_procs: Arc<Mutex<HashMap<String, Child>>> = Arc::new(Mutex::new(HashMap::new()));` 開始到整個 `logic.on_launch_instance(...)` 結束的區塊，替換為：

```rust
    let running_procs: Arc<Mutex<HashMap<String, Child>>> = Arc::new(Mutex::new(HashMap::new()));
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
                (c.version.clone(), c.id.clone(), c.name.clone(), c.xmx.clone(), c.xms.clone())
            } else {
                (id.to_string(), id.to_string(), id.to_string(), "2G".into(), "512M".into())
            }
        };
        let running_procs = Arc::clone(&running_procs_for_launch);
        let ui_weak = ui_weak_for_launch.clone();
        let logs = Arc::clone(&instance_logs_for_launch);
        if running_procs.lock().unwrap().contains_key(&instance_id) {
            return;
        }
        tokio::spawn(async move {
            match do_launch(version_id, instance_id.clone(), instance_name.clone(), xmx, xms, ui_weak.clone(), logs).await {
                Ok(child) => {
                    running_procs.lock().unwrap().insert(instance_id.clone(), child);
                    set_instance_status(&ui_weak, &instance_id, "running");
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
```

### Step 5：替換 on_new_instance stub 並新增 InstanceCreateLogic callbacks

找到：
```rust
    logic.on_new_instance(|| {
        println!("Rust: 開啟建立視窗");
        //TODO: 需要建立實例資料夾
    });
```

替換為：
```rust
    let ui_weak_for_new = ui.as_weak();
    logic.on_new_instance(move || {
        let Some(ui) = ui_weak_for_new.upgrade() else { return };
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
        let Some(ui) = ui_weak_for_confirm.upgrade() else { return };
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
```

### Step 6：更新 do_launch 函式簽名

找到：
```rust
async fn do_launch(
    version_id: String,
    instance_id: String,
    instance_name: String,
    ui_weak: slint::Weak<MainApp>,
    instance_logs: Arc<Mutex<HashMap<String, VecDeque<String>>>>,
) -> anyhow::Result<Child> {
```

改成：
```rust
async fn do_launch(
    version_id: String,
    instance_id: String,
    instance_name: String,
    xmx: String,
    xms: String,
    ui_weak: slint::Weak<MainApp>,
    instance_logs: Arc<Mutex<HashMap<String, VecDeque<String>>>>,
) -> anyhow::Result<Child> {
```

然後在 `do_launch` 函式內找到 `LaunchContext` 的 `xmx` 和 `xms` 欄位：
```rust
        xmx: "2G".into(),
        xms: "512M".into(),
```

改成：
```rust
        xmx,
        xms,
```

### Step 7：建置驗證

```bash
cd /Users/derrick/Documents/Program/rust/Project/VoxelRuler
cargo build 2>&1
```

預期：成功建置，0 errors。

如果有錯誤：
- 若是 `InstanceCreateLogic` 找不到 method：確認 Task 03 的 global.slint 改動已提交
- 若是 `get_selected_version` 找不到：確認 global.slint 有 `selected-version` 屬性
- 若是 borrow checker 錯誤：仔細檢查 Arc clone 位置

### Step 8：提交

```bash
cd /Users/derrick/Documents/Program/rust/Project/VoxelRuler
git add src/view.rs
git commit -m "feat: wire InstanceCreateLogic callbacks and replace hardcoded instances with TOML store"
```

---

## 完成後

更新 `.claude/agent/status.md`，將 Batch 4 改為 `✅ 完成`，並記錄完成時間。

同時在整個 TODO.md 中更新 M2 的以下項目：
- `[ ] 實例儲存格式（JSON / TOML）與讀寫邏輯` → `[x]`
- `[ ] instances.slint 從 Rust 端接收實例列表並顯示` → `[x]`
- `[ ] on_new_instance` → `[x]`
