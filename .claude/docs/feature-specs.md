# 功能規格書（Feature Specifications）

> VoxelRuler 專案適用 | 版本：1.0 | 建立：2026-05-19

---

## FS-01：帳號管理（Account）

**優先級**：P0（最高）  
**里程碑**：M1（5/19 – 5/25）  
**相關檔案**：`ui/components/pages/account.slint`、`src/mc_token.rs`、`src/view.rs`

### 功能描述

使用者可透過 Microsoft 帳號登入，登入後顯示 Minecraft 帳號資訊，並可登出。

### 使用者流程

```
未登入狀態
  └─ 點選「登入」
       └─ 開啟 Microsoft OAuth URL（url-dialog.slint）
            └─ 使用者完成授權
                 ├─ 成功 → 儲存 session → 顯示已登入狀態
                 └─ 失敗 → 顯示錯誤訊息

已登入狀態
  └─ 點選「登出」
       └─ 清除 session 檔案 → 回到未登入狀態
```

### UI 需求

| 元件 | 未登入狀態 | 已登入狀態 |
|------|-----------|-----------|
| 頭像 | 顯示預設圖示 | 顯示 Minecraft 頭像 |
| 使用者名稱 | 隱藏 | 顯示 Minecraft ID |
| 主要按鈕 | 「使用 Microsoft 登入」 | 「登出」 |
| 登入狀態指示 | 紅色圓點 | 綠色圓點 |

### Rust API

```rust
// 檢查登入狀態
fn is_logged_in() -> bool

// 取得使用者名稱
fn get_username() -> Option<String>

// 執行登入（非同步）
async fn login() -> Result<()>

// 執行登出
fn logout() -> Result<()>
```

---

## FS-02：實例管理（Instances）

**優先級**：P0（最高）  
**里程碑**：M2（5/26 – 6/01）、M4（6/09 – 6/15）  
**相關檔案**：`ui/components/pages/instances.slint`、`src/instance/`（待建立）

### 功能描述

使用者可建立、瀏覽、搜尋、刪除 Minecraft 遊戲實例，每個實例對應一個獨立的遊戲設定與版本。

### 實例資料結構

```rust
pub struct InstanceData {
    pub id: String,           // UUID
    pub name: String,         // 使用者命名
    pub mc_version: String,   // 如 "1.21.4"
    pub loader: Loader,       // Vanilla / Fabric / Forge / Quilt
    pub loader_version: Option<String>,
    pub icon: Option<PathBuf>,
    pub last_played: Option<DateTime<Utc>>,
    pub total_play_time: Duration,
}

pub enum Loader {
    Vanilla,
    Fabric,
    Forge,
    Quilt,
}
```

### 儲存格式

- 位置：`{data_dir}/instances/<id>/instance.json`
- 格式：JSON（`serde_json`）

### UI 需求

- 以卡片格式（`InstanceCard`）顯示實例列表（Grid 排列）
- 搜尋列可即時過濾實例名稱
- 每張卡片顯示：圖示、名稱、版本、最後遊玩時間
- 右下角「+」按鈕開啟新增實例對話框

---

## FS-03：啟動 Minecraft（Launch）

**優先級**：P0（最高）  
**里程碑**：M3（6/02 – 6/08）  
**相關檔案**：`src/mc_action.rs`、`src/view.rs`、`ui/components/file-progress.slint`

### 功能描述

使用者點選實例後，可啟動對應版本的 Minecraft，進入遊戲正常遊玩。

### 啟動流程

```
點選實例「啟動」按鈕
  ├─ 未登入 → 提示先登入
  └─ 已登入
       ├─ 檢查遊戲檔案完整性
       │    ├─ 不完整 → 顯示下載進度 → 下載後繼續
       │    └─ 完整 → 繼續
       ├─ 從 Minecraft API 取得啟動參數
       ├─ 組合 JVM 啟動指令
       ├─ std::process::Command::new(java_path).args(...).spawn()
       └─ UI 顯示「啟動中」→「遊戲進行中」
```

### 啟動參數來源

- Minecraft version manifest：`https://piston-meta.mojang.com/mc/game/version_manifest_v2.json`
- 版本 JSON：從 manifest 取得對應版本的 JSON URL
- Asset index：從版本 JSON 取得
- Libraries：從版本 JSON 取得並驗證 SHA1

### 錯誤處理

| 錯誤情況 | UI 顯示 |
|---------|---------|
| 未登入 | 「請先登入 Microsoft 帳號」 |
| 遊戲檔案損毀 | 「檔案驗證失敗，嘗試重新下載？」 |
| Java 未找到 | 「找不到 Java，請安裝 Java 17+」 |
| 網路錯誤 | 「網路連線錯誤，請檢查連線」 |

---

## FS-04：遊戲下載（Downloads）

**優先級**：P1  
**里程碑**：M5（6/16 – 6/22）  
**相關檔案**：`ui/components/pages/`（待建立 downloads 頁面）、`src/mc_action.rs`

### 功能描述

使用者可瀏覽並下載 Minecraft 版本（含 Release 與 Snapshot），下載過程顯示進度。

### UI 需求

- 版本列表（Release / Snapshot 分類）
- 下載按鈕（已下載顯示「已安裝」）
- 下載進度條（`file-progress.slint`）
- 可取消下載

---

## FS-05：設定（Settings）

**優先級**：P2  
**里程碑**：視 M1–M4 完成狀況排程  
**相關檔案**：`ui/components/pages/`（待建立 settings 頁面）

### 功能描述

使用者可調整 Launcher 全域設定。

### 基本設定項目

| 設定 | 類型 | 預設值 |
|------|------|--------|
| 語言 | 下拉選單（zh_TW / en_US） | 系統語言 |
| 主題 | 切換（深色 / 淺色） | 深色 |
| Java 路徑 | 文字輸入 / 自動偵測 | 自動偵測 |
| 最大記憶體（MB） | 數字輸入 | 2048 |
| 下載並行數 | 滑桿（1–8） | 4 |

---

## FS-06：多平台打包與發布

**優先級**：P0（發布必要）  
**里程碑**：M6（6/23 – 6/30）

### GitHub Actions 矩陣

```yaml
strategy:
  matrix:
    os: [windows-latest, macos-latest, ubuntu-latest]
```

### 產出物

| 平台 | 格式 |
|------|------|
| Windows | `.exe`（透過 NSIS 或 cargo-packer） |
| macOS | `.dmg` 或 `.app.tar.gz` |
| Linux | `.AppImage` 或 `.tar.gz` |

### 版本發布流程

1. `develop` → PR → `main`（Squash merge）
2. 打 tag：`git tag v1.0.0 && git push origin v1.0.0`
3. GitHub Actions 觸發，編譯三平台並上傳 Release Assets
4. 發布 GitHub Release（含 CHANGELOG）
