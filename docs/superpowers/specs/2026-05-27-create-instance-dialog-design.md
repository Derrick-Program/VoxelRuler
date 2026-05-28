# 設計文件：新增實例對話框（Create Instance Dialog）

**日期：** 2026-05-27  
**里程碑：** M2（Instances 資料層，5/26–6/01）  
**範疇：** `on_new_instance` callback 完整實作，含 UI 對話框、TOML 持久化、版本列表快取

---

## 目標

使用者點擊 Instances 頁面右上角「Create」按鈕後，可在 3-Tab 對話框中填入完整實例資訊並建立，實例資料以 TOML 格式儲存至磁碟，且在 UI 卡片清單即時更新。

---

## 架構總覽

```
App 啟動
  ├── 從磁碟載入 instances.toml → InstanceData VecModel → UI 顯示
  └── 背景 tokio task：fetch MC 版本列表 → 設到 InstanceCreateLogic.version-list

[點擊 Create 按鈕]
  └── InstanceLogic.new_instance() callback
        └── Rust: 重設表單欄位 → set show_create_dialog = true
              └── UI: create-instance-dialog 對話框顯示

[使用者填表，點「建立實例」]
  └── InstanceCreateLogic.confirm_create() callback
        ├── 驗證：name 不可空、version 已選擇
        ├── 生成 UUID v4 作為 id
        ├── 建立 InstanceConfig 並呼叫 InstanceStore::append()
        │     └── 寫入 instances.toml（append + rewrite）
        └── push 至 VecModel → 對話框關閉 → 卡片即時出現
```

---

## 新增/修改檔案

| 檔案 | 操作 | 說明 |
|------|------|------|
| `src/mc_instance.rs` | 新增 | `InstanceConfig` struct + `InstanceStore` 讀寫 |
| `ui/components/pages/create-instance-dialog.slint` | 新增 | 3-Tab 對話框元件 |
| `ui/global.slint` | 修改 | 新增 `InstanceCreateLogic` global |
| `ui/components/pages/instances.slint` | 修改 | 引入並顯示 create-instance-dialog |
| `src/view.rs` | 修改 | 串接所有 callback、啟動時載入 |
| `Cargo.toml` | 修改 | 新增 `toml`、`uuid` 依賴 |

---

## 資料結構

### TOML 儲存格式（`~/.voxelruler/instances.toml` 或 `McPaths::instances_file()`）

```toml
[[instances]]
id             = "a7f3b2c1-4e5d-6f78-9012-abcdef123456"
name           = "生存模式 1.20"
version        = "1.20.4"
mod_loader     = "Fabric"   # "None" | "Fabric" | "Forge"
xmx            = "2G"
xms            = "512M"
logs_enabled   = true
world_path     = ""
resource_pack  = ""
shader_pack    = ""
last_played    = ""         # ISO 8601，由啟動時更新
play_time_secs = 0
```

### `InstanceConfig`（Rust struct）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceConfig {
    pub id: String,
    pub name: String,
    pub version: String,
    pub mod_loader: String,
    pub xmx: String,
    pub xms: String,
    pub logs_enabled: bool,
    pub world_path: String,
    pub resource_pack: String,
    pub shader_pack: String,
    pub last_played: String,
    pub play_time_secs: u64,
}

impl Default for InstanceConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            version: String::new(),
            mod_loader: "None".into(),
            xmx: "2G".into(),
            xms: "512M".into(),
            logs_enabled: true,
            world_path: String::new(),
            resource_pack: String::new(),
            shader_pack: String::new(),
            last_played: String::new(),
            play_time_secs: 0,
        }
    }
}
```

### `InstanceStore`

```rust
pub struct InstanceStore {
    path: PathBuf,
}

impl InstanceStore {
    pub fn new(path: PathBuf) -> Self
    pub fn load(&self) -> anyhow::Result<Vec<InstanceConfig>>
    pub fn save(&self, instances: &[InstanceConfig]) -> anyhow::Result<()>
    pub fn append(&self, instance: InstanceConfig) -> anyhow::Result<Vec<InstanceConfig>>
}
```

### `InstanceCreateLogic`（新增至 `global.slint`）

```slint
export global InstanceCreateLogic {
    // 對話框控制
    in-out property <bool>     show-dialog: false;
    in-out property <int>      active-tab: 0;       // 0=基本 1=進階 2=資源

    // Basic Tab
    in-out property <string>   name: "";
    in-out property <[string]> version-list: [];
    in-out property <int>      selected-version-index: 0;
    in-out property <string>   mod-loader: "None";   // "None"|"Fabric"|"Forge"

    // Advanced Tab
    in-out property <string>   xmx: "2G";
    in-out property <string>   xms: "512M";
    in-out property <bool>     logs-enabled: true;

    // Resources Tab
    in-out property <string>   world-path: "";
    in-out property <string>   resource-pack: "";
    in-out property <string>   shader-pack: "";

    // 狀態
    in-out property <string>   error-msg: "";
    in-out property <bool>     is-loading: false;   // 版本列表載入中

    callback confirm-create();
    callback cancel-create();
}
```

---

## UI 設計（`create-instance-dialog.slint`）

顯示於 `InstanceCreateLogic.show-dialog == true` 時，全螢幕半透明遮罩 + 居中面板：

### Basic Tab（active-tab == 0）

```
╔══════════════════════════════════════════════╗
║  新增 Minecraft 實例                    [✕]  ║
╠══════════════════════════════════════════════╣
║  [基本] [進階] [資源]                        ║
╠══════════════════════════════════════════════╣
║                                              ║
║  實例名稱                                    ║
║  [_____________________________]             ║
║                                              ║
║  Minecraft 版本                              ║
║  [1.20.4                      ▼]  (下拉)     ║
║                                              ║
║  模組載入器                                  ║
║  (●) 無  ( ) Fabric  ( ) Forge               ║
║                                              ║
╠══════════════════════════════════════════════╣
║  error-msg（紅字，不可見時不佔空間）         ║
║               [ 取消 ]  [ 建立實例 ]         ║
╚══════════════════════════════════════════════╝
```

### Advanced Tab（active-tab == 1）

```
║  記憶體上限 (Xmx)  [2G    ]                  ║
║  記憶體起始 (Xms)  [512M  ]                  ║
║  啟用遊戲 Logs     [✓] 開啟                  ║
```

### Resources Tab（active-tab == 2）

```
║  世界/存檔路徑    [________________]          ║
║  材質包路徑       [________________]          ║
║  光影包路徑       [________________]          ║
║  （手動輸入完整路徑）                        ║
```

**樣式規則：**
- 背景：`AppTheme.colors.background` + 暗色遮罩（`#000000.with-alpha(0.5)`）
- 面板：`border-radius: 12px`，`border-width: 1px`，`border-color: #ffffff.with-alpha(0.1)`
- Tab 選中狀態：`#76bc51` 底線（和 sidebar hover 色一致）
- 「建立實例」按鈕：和卡片啟動按鈕同色方案（`#67ab44`）
- 版本下拉：版本列表載入中時顯示 loading 文字

---

## Rust 實作細節（`view.rs`）

### 啟動時（`open_view()`）

```rust
// 1. 載入實例
// InstanceStore 包裝在 Arc<Mutex<>> 以供跨 thread 使用
let store = Arc::new(Mutex::new(InstanceStore::new(McPaths::new()?.instances_file())));
let loaded_instances = store.lock().unwrap().load().unwrap_or_default();
let raw_instances: Vec<InstanceData> = loaded_instances.iter().map(config_to_ui_data).collect();
// VecModel 也用 Arc 包裝，供 confirm_create 閉包 push 新資料
let instances_model = Arc::new(VecModel::from(raw_instances));
// ... 設到 UI

// 2. 背景取得版本列表
let ui_weak_for_versions = ui.as_weak();
tokio::spawn(async move {
    let api = mc_api::McAction::new();
    if let Ok(versions) = api.get_all_mc_versions().await {
        let list: Vec<SharedString> = versions.iter().map(|v| v.id.as_str().into()).collect();
        slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak_for_versions.upgrade() {
                ui.global::<InstanceCreateLogic>()
                  .set_version_list(ModelRc::from(Rc::new(VecModel::from(list))));
            }
        }).ok();
    }
});
```

### `on_new_instance()`

```rust
logic.on_new_instance(move || {
    let ui = ui_weak.upgrade()?;
    let create = ui.global::<InstanceCreateLogic>();
    // 重設所有欄位為預設值
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
```

### `on_confirm_create()`

```rust
create_logic.on_confirm_create(move || {
    let ui = ui_weak.upgrade()?;
    let create = ui.global::<InstanceCreateLogic>();

    let name = create.get_name().to_string();
    let version_list: Vec<_> = create.get_version_list().iter().collect();
    let idx = create.get_selected_version_index() as usize;

    // 驗證
    if name.trim().is_empty() {
        create.set_error_msg("實例名稱不可為空".into());
        return;
    }
    if version_list.is_empty() || idx >= version_list.len() {
        create.set_error_msg("請選擇 Minecraft 版本".into());
        return;
    }

    let config = InstanceConfig {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.trim().into(),
        version: version_list[idx].to_string(),
        mod_loader: create.get_mod_loader().to_string(),
        xmx: create.get_xmx().to_string(),
        xms: create.get_xms().to_string(),
        logs_enabled: create.get_logs_enabled(),
        world_path: create.get_world_path().to_string(),
        resource_pack: create.get_resource_pack().to_string(),
        shader_pack: create.get_shader_pack().to_string(),
        ..Default::default()
    };

    let store_clone = Arc::clone(&store);
    let ui_weak_clone = ui_weak.clone();
    let model_clone = Arc::clone(&instances_model);
    tokio::spawn(async move {
        match store_clone.append(config) {
            Ok(updated) => {
                slint::invoke_from_event_loop(move || {
                    // push 最後一筆（新增的）至 VecModel
                    if let Some(last) = updated.last() {
                        model_clone.push(config_to_ui_data(last));
                    }
                    if let Some(ui) = ui_weak_clone.upgrade() {
                        ui.global::<InstanceCreateLogic>().set_show_dialog(false);
                    }
                }).ok();
            }
            Err(e) => {
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_clone.upgrade() {
                        ui.global::<InstanceCreateLogic>()
                          .set_error_msg(format!("建立失敗：{e}").into());
                    }
                }).ok();
            }
        }
    });
});
```

### `on_cancel_create()`

```rust
create_logic.on_cancel_create(move || {
    if let Some(ui) = ui_weak.upgrade() {
        ui.global::<InstanceCreateLogic>().set_show_dialog(false);
    }
});
```

---

## 依賴新增（`Cargo.toml`）

```toml
toml = "0.8"
uuid = { version = "1", features = ["v4", "fast-rng"] }
```

---

## `McPaths` 補充

新增 `instances_file()` 方法：

```rust
pub fn instances_file(&self) -> PathBuf {
    self.data_dir.join("instances.toml")
}
```

---

## 轉換函式

```rust
fn config_to_ui_data(config: &InstanceConfig) -> InstanceData {
    // play_time：0 秒 → 空字串；其他 → 格式化為 "Xh Ym"
    let play_time = if config.play_time_secs == 0 {
        String::new()
    } else {
        let h = config.play_time_secs / 3600;
        let m = (config.play_time_secs % 3600) / 60;
        format!("{}h {}m", h, m)
    };

    // 預設圖片：使用 CARGO_MANIFEST_DIR 下的 voxelruler.png
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

---

## 測試涵蓋

| 測試 | 位置 |
|------|------|
| `test_instance_store_load_empty` | `mc_instance.rs` |
| `test_instance_store_append_and_reload` | `mc_instance.rs` |
| `test_instance_store_save_preserves_all_fields` | `mc_instance.rs` |
| `test_config_to_ui_data_conversion` | `mc_instance.rs` |

---

## 不在本次範疇

- 刪除實例（M4）
- 編輯實例（M4）
- Native file picker（之後再評估，目前用手動輸入路徑）
- Mod loader 實際安裝（Fabric/Forge installer 流程）
- 版本 snapshot/release 過濾

---

*此文件由 Claude Code 於 2026-05-27 在 brainstorming 流程中產生*
