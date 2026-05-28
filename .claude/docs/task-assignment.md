# 工作分配表（Task Assignment）

> VoxelRuler 專案 | 版本：1.0 | 建立：2026-05-19 | 最後更新：2026-05-19  
> 成員：**Derrick**、**mlask** | 期限：2026-06-30

---

## 分工原則

- 每個里程碑內，Derrick 主責 **Rust 後端邏輯**，mlask 主責 **Slint UI 層**
- 雙方可同步進行，但 UI 串接任務需等後端介面（struct / callback）定義完成後才能開始
- 每個任務標記優先等級：🔴 必須完成、🟡 重要、🟢 加分項
- 任務完成後在此文件對應的 `[ ]` 改為 `[x]`，並更新 `TODO.md`

---

## M1：Account 帳號頁面（5/19 – 5/25）

> **目標**：使用者可看到登入狀態，並可登入 / 登出

### Derrick — Rust 後端

| 優先 | 任務 | 相關檔案 | 說明 |
|------|------|----------|------|
| 🔴 | [ ] 定義帳號狀態介面，供 UI 讀取 | `src/view.rs` | 將 `GLOBAL_CACHE` 中的 `mc_ac_key` 轉為 UI 可用的 bool / String |
| 🔴 | [ ] 實作 `logout()` 函式 | `src/mc_token.rs` | 清除 session 檔案 + 移除 `GLOBAL_CACHE` 中的 token |
| 🔴 | [ ] 在 `view.rs` 綁定登入 callback | `src/view.rs` | 接收 UI 發出的登入事件，非同步執行 OAuth 流程 |
| 🔴 | [ ] 在 `view.rs` 綁定登出 callback | `src/view.rs` | 接收 UI 發出的登出事件，呼叫 `logout()` |
| 🟡 | [ ] 取得 Minecraft 帳號名稱 | `src/mc_token.rs` | 從已儲存的 session 或 API 取得 `mc_username` |
| 🟡 | [ ] 取得 Minecraft 頭像 URL | `src/mc_action.rs` | 呼叫 `sessionserver.mojang.com` 取得頭像 |

**產出介面（mlask 依賴此）**：
```rust
// view.rs 中需要設定這些 property / callback
app.set_is_logged_in(bool);
app.set_username(SharedString);
app.on_login_requested(|| { /* 啟動 OAuth */ });
app.on_logout_requested(|| { /* 執行登出 */ });
```

---

### mlask — Slint UI

> 依賴 Derrick 完成介面定義後開始串接，UI 外觀可提前完成

| 優先 | 任務 | 相關檔案 | 說明 |
|------|------|----------|------|
| 🔴 | [ ] 未登入狀態 UI 版面 | `ui/components/pages/account.slint` | 顯示預設頭像、「使用 Microsoft 登入」按鈕 |
| 🔴 | [ ] 已登入狀態 UI 版面 | `ui/components/pages/account.slint` | 顯示頭像、使用者名稱、「登出」按鈕 |
| 🔴 | [ ] 登入狀態指示圓點 | `ui/components/pages/account.slint` | 未登入紅色、已登入綠色 |
| 🔴 | [ ] 登入按鈕觸發 callback | `ui/components/pages/account.slint` | `login-requested()` callback 傳至 `global.slint` |
| 🔴 | [ ] 登出按鈕觸發 callback | `ui/components/pages/account.slint` | `logout-requested()` callback |
| 🟡 | [ ] 登入中 loading 狀態 | `ui/components/pages/account.slint` | 按下登入後顯示 spinner，禁止重複點擊 |
| 🟡 | [ ] 錯誤訊息顯示 | `ui/components/pages/account.slint` | OAuth 失敗時顯示錯誤文字 |
| 🟢 | [ ] 側邊欄頭像同步更新 | `ui/components/sidebar/avatar.slint` | 登入後側邊欄 Avatar 顯示真實頭像 |

**M1 完成標準（兩人共同驗收）**：
- [ ] 冷啟動且有有效 token → 帳號頁直接顯示已登入
- [ ] 點選登入 → OAuth URL 對話框開啟 → 完成授權 → 顯示已登入
- [ ] 點選登出 → 狀態回到未登入

---

## M2：Instances 資料層（5/26 – 6/01）

> **目標**：實例列表可從磁碟讀取並顯示在 UI，搜尋可過濾

### Derrick — Rust 後端

| 優先 | 任務 | 相關檔案 | 說明 |
|------|------|----------|------|
| 🔴 | [ ] 建立 `src/instance/` 模組 | `src/instance/mod.rs` | 宣告模組，在 `main.rs` 引入 |
| 🔴 | [ ] 定義 `InstanceData` struct | `src/instance/data.rs` | 含 id, name, mc_version, loader, last_played 等欄位 |
| 🔴 | [ ] 實作 `InstanceStorage`：讀取 | `src/instance/storage.rs` | 從 `{data_dir}/instances/` 讀取所有 `instance.json` |
| 🔴 | [ ] 實作 `InstanceStorage`：寫入 | `src/instance/storage.rs` | 儲存單一實例至對應目錄 |
| 🔴 | [ ] 在 `view.rs` 載入實例並傳給 UI | `src/view.rs` | 啟動時讀取實例列表，設定為 `ModelRc` |
| 🟡 | [ ] 定義 Slint 可用的 `InstanceViewModel` | `src/view.rs` | 將 `InstanceData` 轉換為 Slint struct |

**產出介面（mlask 依賴此）**：
```rust
// Slint 端可用的結構（在 global.slint 或 instances.slint 宣告）
struct InstanceViewModel {
    id: string,
    name: string,
    mc-version: string,
    loader: string,
    last-played: string,
}
app.set_instances(ModelRc<InstanceViewModel>);
```

---

### mlask — Slint UI

| 優先 | 任務 | 相關檔案 | 說明 |
|------|------|----------|------|
| 🔴 | [ ] `InstanceCard` 元件完善 | `ui/components/pages/instances.slint` | 顯示圖示、名稱、版本、最後遊玩時間 |
| 🔴 | [ ] 卡片列表綁定 `ModelRc` 資料 | `ui/components/pages/instances.slint` | `for instance in instances:` 動態渲染 |
| 🔴 | [ ] 搜尋列即時過濾邏輯 | `ui/components/pages/instances.slint` | 輸入時過濾顯示的卡片 |
| 🟡 | [ ] 空列表佔位畫面 | `ui/components/pages/instances.slint` | 無實例時顯示「點選 + 新增第一個實例」 |
| 🟡 | [ ] 卡片 hover 狀態 | `ui/components/pages/instances.slint` | 滑鼠移入時顯示操作按鈕（啟動、刪除） |

**M2 完成標準**：
- [ ] 有實例資料時，卡片正確渲染並顯示所有欄位
- [ ] 搜尋列輸入文字後，列表即時過濾

---

## M3：啟動 Minecraft（6/02 – 6/08）【核心功能】

> **目標**：點選實例啟動按鈕，Minecraft 真正開啟可遊玩

### Derrick — Rust 後端

| 優先 | 任務 | 相關檔案 | 說明 |
|------|------|----------|------|
| 🔴 | [ ] 從 version manifest 取得版本 JSON | `src/mc_action.rs` | GET `piston-meta.mojang.com/mc/game/version_manifest_v2.json` |
| 🔴 | [ ] 解析版本 JSON 取得 libraries 與 mainClass | `src/mc_action.rs` | 建立 `LaunchArgs` struct |
| 🔴 | [ ] 驗證本機遊戲檔案（SHA1 check） | `src/mc_action.rs` | 比對 libraries 的 SHA1，缺少則標記需下載 |
| 🔴 | [ ] 下載缺失的遊戲檔案 | `src/mc_action.rs` | 並行下載，回傳進度 channel |
| 🔴 | [ ] 組合 JVM 啟動指令 | `src/mc_action.rs` | classpath, JVM args, game args, token 注入 |
| 🔴 | [ ] `std::process::Command` 執行 Minecraft | `src/mc_action.rs` | `.spawn()` 非阻塞，監聽退出狀態 |
| 🔴 | [ ] 在 `view.rs` 綁定啟動 callback | `src/view.rs` | 接收 UI 的 `launch-instance(id)` 事件 |
| 🟡 | [ ] Java 自動偵測 | `src/mc_action.rs` | 依平台搜尋 `java` / `java.exe` 路徑 |
| 🟡 | [ ] 下載進度傳至 UI | `src/view.rs` | 透過 `invoke_from_event_loop` 更新進度 property |

---

### mlask — Slint UI

| 優先 | 任務 | 相關檔案 | 說明 |
|------|------|----------|------|
| 🔴 | [ ] 實例卡片「啟動」按鈕 | `ui/components/pages/instances.slint` | 觸發 `launch-instance(id)` callback |
| 🔴 | [ ] 啟動中狀態顯示 | `ui/components/pages/instances.slint` | 按鈕變為 loading 狀態，禁止重複點擊 |
| 🔴 | [ ] 下載進度對話框串接 | `ui/components/file-progress.slint` | 顯示下載百分比、檔案名稱 |
| 🔴 | [ ] 啟動成功 / 失敗通知 | `ui/components/pages/instances.slint` | 成功：顯示「遊戲進行中」；失敗：顯示錯誤 |
| 🟡 | [ ] 未登入時點啟動的提示 | `ui/components/pages/instances.slint` | 提示「請先前往帳號頁登入」 |
| 🟡 | [ ] 遊戲進行中時的 UI 狀態 | `ui/components/pages/instances.slint` | 卡片顯示「進行中」標籤，可點「結束遊戲」 |

**M3 完成標準（最關鍵里程碑）**：
- [ ] 點選已有遊戲檔案的實例 → Minecraft 啟動，可正常進入遊戲
- [ ] 點選尚未下載的版本 → 顯示下載進度 → 下載完成後啟動
- [ ] Java 不存在時顯示明確錯誤訊息

---

## M4：實例 CRUD 管理（6/09 – 6/15）

> **目標**：使用者可在 UI 中新增、刪除、編輯實例

### Derrick — Rust 後端

| 優先 | 任務 | 相關檔案 | 說明 |
|------|------|----------|------|
| 🔴 | [ ] 實作新增實例邏輯 | `src/instance/storage.rs` | 建立目錄結構，寫入 `instance.json` |
| 🔴 | [ ] 實作刪除實例邏輯 | `src/instance/storage.rs` | 刪除 `{data_dir}/instances/<id>/` 整個目錄 |
| 🔴 | [ ] 實作編輯實例邏輯 | `src/instance/storage.rs` | 更新 `instance.json` 指定欄位 |
| 🔴 | [ ] 在 `view.rs` 綁定 CRUD callbacks | `src/view.rs` | `on_add_instance`, `on_delete_instance`, `on_edit_instance` |
| 🟡 | [ ] 取得 Minecraft 版本清單 | `src/mc_action.rs` | 供新增實例時選擇版本用 |

---

### mlask — Slint UI

| 優先 | 任務 | 相關檔案 | 說明 |
|------|------|----------|------|
| 🔴 | [ ] 新增實例對話框 | `ui/components/pages/instances.slint` | 輸入名稱、選擇版本、選擇 Loader |
| 🔴 | [ ] 版本下拉選單 | `ui/components/pages/instances.slint` | 從後端取得版本清單並顯示 |
| 🔴 | [ ] 刪除確認對話框 | `ui/components/pages/instances.slint` | 「確定要刪除 {name} 嗎？」+ 確認 / 取消按鈕 |
| 🔴 | [ ] 編輯實例設定 UI | `ui/components/pages/instances.slint` | 可修改名稱、記憶體、Java 路徑 |
| 🟡 | [ ] 新增後自動重整列表 | `ui/components/pages/instances.slint` | 新增成功後列表即時反映新實例 |

**M4 完成標準**：
- [ ] 可從「+」按鈕新增實例，填入名稱和版本後儲存並顯示
- [ ] 可刪除實例（有確認步驟）
- [ ] 可編輯實例名稱與基本設定

---

## M5：遊戲下載與進度（6/16 – 6/22）

> **目標**：使用者可瀏覽版本清單並下載，顯示完整進度

### Derrick — Rust 後端

| 優先 | 任務 | 相關檔案 | 說明 |
|------|------|----------|------|
| 🔴 | [ ] 取得完整版本清單 API | `src/mc_action.rs` | Release + Snapshot，含版本號、發布日期 |
| 🔴 | [ ] 並行下載引擎 | `src/mc_action.rs` | `tokio::spawn` 多工，可控制並行數量 |
| 🔴 | [ ] 下載進度 channel | `src/mc_action.rs` | `(已下載 bytes, 總 bytes, 當前檔案名)` |
| 🔴 | [ ] 跨平台資料目錄路徑 | `src/mc_action.rs` | 統一使用 `directories::ProjectDirs` |
| 🟡 | [ ] 下載取消機制 | `src/mc_action.rs` | `tokio::CancellationToken` 實作取消 |
| 🟡 | [ ] 下載失敗重試 | `src/mc_action.rs` | 最多重試 3 次，指數退避 |
| 🟢 | [ ] CI 雛形建立（提前準備 M6） | `.github/workflows/` | 建立 build workflow，先只做單平台 |

---

### mlask — Slint UI

| 優先 | 任務 | 相關檔案 | 說明 |
|------|------|----------|------|
| 🔴 | [ ] 建立 `downloads.slint` 頁面 | `ui/components/pages/downloads.slint` | 版本列表頁面骨架 |
| 🔴 | [ ] 在 `pages/index.slint` 路由 downloads | `ui/components/pages/index.slint` | 將 `SideTabId.downloads` 導向新頁面 |
| 🔴 | [ ] 版本列表（Release / Snapshot 分頁） | `ui/components/pages/downloads.slint` | Tab 切換兩種版本類型 |
| 🔴 | [ ] 每列顯示版本號、發布日期、下載按鈕 | `ui/components/pages/downloads.slint` | 已下載顯示「已安裝」標籤 |
| 🔴 | [ ] 下載進度條整合 | `ui/components/file-progress.slint` | 複用現有元件 |
| 🟡 | [ ] 取消下載按鈕 | `ui/components/pages/downloads.slint` | 顯示「取消」並觸發後端取消 |

**M5 完成標準**：
- [ ] 可在下載頁瀏覽版本清單
- [ ] 點擊下載後顯示進度條，下載完成後標記為「已安裝」

---

## M6：多平台打包與 v1.0.0 發布（6/23 – 6/30）

> **目標**：GitHub Release 頁面有三平台安裝包，完成 v1.0.0 發布

### Derrick — CI / 打包

| 優先 | 任務 | 相關檔案 | 說明 |
|------|------|----------|------|
| 🔴 | [ ] GitHub Actions 三平台編譯 | `.github/workflows/release.yml` | matrix: windows / macos / ubuntu |
| 🔴 | [ ] cargo-packer 設定確認 | `Cargo.toml` / packer config | 確認三平台打包參數正確 |
| 🔴 | [ ] Release Asset 自動上傳 | `.github/workflows/release.yml` | tag push 觸發，上傳各平台產出物 |
| 🟡 | [ ] CHANGELOG 自動產生 | `.github/workflows/release.yml` | 從 git log 產生 CHANGELOG |
| 🟡 | [ ] Release draft 自動建立 | `.github/workflows/release.yml` | 使用 `softprops/action-gh-release` |

---

### mlask — 測試與 QA

| 優先 | 任務 | 說明 |
|------|------|------|
| 🔴 | [ ] Windows 平台完整流程測試 | 登入 → 建立實例 → 下載版本 → 啟動遊戲 |
| 🔴 | [ ] macOS 平台完整流程測試 | 同上 |
| 🔴 | [ ] 已知 Bug 清單整理 | 記錄發現的問題，優先修復 P0 級別 |
| 🟡 | [ ] UI 最終微調 | 字型、間距、顏色對齊 |
| 🟡 | [ ] README 更新 | 安裝說明、截圖、功能清單 |
| 🟢 | [ ] Linux 平台測試 | AppImage 啟動與功能驗證 |

**M6 完成標準（發布條件）**：
- [ ] 三平台安裝包可正常安裝並啟動
- [ ] 完整遊玩流程無崩潰
- [ ] GitHub Release `v1.0.0` 發布成功

---

## 跨里程碑共同責任

| 事項 | 兩人共同負責 |
|------|-------------|
| Code Review | 每個 PR 需對方 Approve 才能 merge |
| 分支管理 | 遵守 Git 規範（詳見 `git-conventions.md`） |
| 進度更新 | 完成任務後在此文件勾選，並通知對方 |
| Bug 回報 | 發現問題立即在 GitHub Issue 記錄 |

---

## 任務依賴關係圖

```
M1：Derrick 定義介面 → mlask 串接 UI
M2：Derrick 建立資料層 → mlask 綁定列表
M3：M1 + M2 完成 → 雙方同步開始啟動功能
M4：M2 完成 → Derrick 後端 CRUD → mlask 對話框 UI
M5：M3 完成 → Derrick 下載引擎 → mlask 下載頁面
M6：M5 完成 → Derrick CI → mlask 測試
```

---

*此文件由 PM（Claude Code）維護，任務完成請勾選並更新最後更新日期*
