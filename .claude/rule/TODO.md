# VoxelRuler 專案進度追蹤

> 專案期限：2026-06-30 | 成員：Derrick、mlask | 最後更新：2026-06-11  
> 詳細分工 → `.claude/docs/task-assignment.md`

---

## 專案目標

開發一個可完整使用的 Minecraft Launcher（VoxelRuler），使用者可以：
1. 透過 Microsoft 帳號登入
2. 管理遊戲實例（新增、刪除、設定）
3. 點選實例後真正啟動 Minecraft 進入遊戲

---

## 里程碑時程

| 里程碑 | 期間 | 狀態 |
|--------|------|------|
| M1：Account 頁面串接完成 | 5/19 – 5/25 | ⚠️ 部分完成（差 2 項） |
| M2：Instances 資料層建立 | 5/26 – 6/01 | 🔄 進行中 |
| M3：啟動 Minecraft（核心功能） | 6/02 – 6/08 | ✅ 核心超前完成 |
| M4：實例 CRUD 管理完成 | 6/09 – 6/15 | ⬜ 待開始 |
| M5：遊戲檔案下載與進度顯示 | 6/16 – 6/22 | ⬜ 待開始 |
| M6：多平台打包、測試、v1.0.0 | 6/23 – 6/30 | ⬜ 待開始 |

---

## M1：Account 頁面串接（5/19 – 5/25）

### 目標
- [x] `account.slint` UI 顯示登入狀態（已登入 / 未登入）
- [ ] 已登入時顯示使用者名稱與頭像（登入成功後需從 GLOBAL_CACHE 取 profile）
- [x] 未登入時顯示「登入」按鈕，觸發 Microsoft OAuth 流程
- [ ] 登出功能（清除 session、清除 GLOBAL_CACHE、重設帳號頁）
- [x] Rust 端 `view.rs` 與 `mc_token.rs` 串接 `GLOBAL_CACHE` 中的 token 狀態

### 完成標準
- 冷啟動時，若有有效 token，帳號頁自動顯示已登入狀態
- 點選登入按鈕可完整走完 OAuth 流程並回到已登入狀態
- 登出後清除 session 檔案，回到未登入狀態

---

## M2：Instances 資料層建立（5/26 – 6/01）

### 目標
- [x] 定義 `InstanceData` 資料結構（名稱、版本、遊戲類型、圖示路徑等）
- [x] 實例儲存格式（JSON / TOML）與讀寫邏輯
- [x] `instances.slint` 從 Rust 端接收實例列表並顯示
- [x] 搜尋功能串接

### 完成標準
- 實例列表可從磁碟讀取並顯示在 UI
- 搜尋可過濾實例名稱

---

## M3：啟動 Minecraft（6/02 – 6/08）【核心功能 — 超前完成】

### 目標
- [x] 調用 Minecraft API 取得啟動參數（`mc_api.rs`）
- [x] 下載並驗證 JVM 與遊戲版本（`mc_install.rs`）
- [x] 組合啟動指令並 `std::process::Command` 執行（`mc_parser.rs` + `do_launch()`）
- [x] 啟動時顯示進度（`set_install_state()` 串接 progress bar）
- [x] 啟動後 UI 反饋（錯誤訊息顯示）
- [x] xmx/xms 改為從實例設定讀取（目前 hardcoded "2G"/"512M"）

### 完成標準
- 點選實例中的啟動按鈕，Minecraft 真正開啟且可正常遊玩

---

## M4：實例 CRUD 管理（6/09 – 6/15）

### 目標
- [x] 新增實例（選擇版本、命名）
- [x] 刪除實例（含確認對話框）（2026-06-11，右鍵選單）
- [x] 編輯實例設定（記憶體 xmx/xms、Java runtime/路徑）（2026-06-11）
- [x] 實例右鍵選單（啟動/停止、Log、編輯、開資料夾、複製、重新命名、刪除）（2026-06-11）
- [x] 實例詳細視窗（Prism 式側欄分頁：Log/版本/模組/資源包/光影包/筆記/世界/伺服器/截圖/設定/其他紀錄檔）（2026-06-11）
- [ ] `AddInstance` UI 串接

### 完成標準
- 使用者可在 UI 中完整新增、刪除、修改實例

---

## M5：遊戲檔案下載與進度（6/16 – 6/22）

### 目標
- [ ] Minecraft 版本清單從 API 取得並顯示（`downloads` 頁面）
- [ ] 下載遊戲核心、資源包時顯示進度條
- [ ] 下載錯誤重試機制
- [ ] 跨平台檔案路徑處理（`directories::ProjectDirs`）

### 完成標準
- 使用者可從 UI 下載指定版本並看到進度

---

## M6：多平台打包與發布（6/23 – 6/30）

### 目標
- [ ] GitHub Actions workflow：Windows / macOS / Linux 三平台編譯
- [ ] `cargo-packer` 打包設定確認
- [ ] Release draft 自動產生（CHANGELOG）
- [ ] 打上 git tag `v1.0.0` 並發布

### 完成標準
- GitHub Release 頁面有三個平台的可執行檔可供下載

---

## 已完成事項 ✅

- [x] Slint UI 框架建立（SideBar、Pages、Footer）
- [x] `AppTheme` 主題系統（深/淺色）
- [x] Microsoft OAuth 認證流程（`mc_token.rs`）
- [x] Minecraft API 端點研究完成（`mc_action.rs`）
- [x] `GLOBAL_CACHE` token 快取機制
- [x] 多語系支援（zh_TW / en_US）
- [x] 修復版本相關 lib 缺失（2026-06-11）：舊版 natives classifier 下載＋解壓、macOS jna 升級一致化、啟動前 classpath 缺檔檢查
- [x] 跨版本圖形崩潰診斷（2026-07-02）：`mc_compat::diagnose_graphics_crash` 特徵表（Intel HD 無加速、GL 3.2 Core、Vulkan 26.2+、Wayland/X11、LWJGL2 macOS、natives 架構不符），遊戲異常退出時自動比對並顯示建議；特徵含 OS 限定避免跨平台誤判；Linux 加 `_JAVA_AWT_WM_NONREPARENTING=1`
- [x] macOS x64 LWJGL 自動替換（2026-07-02）：1.13–1.18 在 Rosetta / Intel Mac（x86_64 Java）自動換 LWJGL **3.2.3** natives-macos（`LWJGL3_X64_OVERRIDE`，Maven Central），修內建 GLFW 3.2.x 在新版 macOS 的「service port for display」崩潰；不能用 3.3.x（GLFW 3.4 dev 對 1.13.x 的 `glfwSetWindowIcon` 報 error 65548 → 啟動期直接崩）；`macos_override_for(version, arm64_java)` 依 Java 架構自動選表
- [x] macOS 26 Tahoe LWJGL 修正（2026-07-03）：3.2.3 內建 GLFW（2019-09 3.4.0-dev snapshot）在 Tahoe 於 glfwInit 就發 65544（x64 probe 實測）→ 兩張 LWJGL3 表升 **3.3.1**（Mojang CDN natives，x64/arm64 各自）＋ **mmachina patched glfw bindings**（`nglfwSetWindowIcon` 移除 JNI 呼叫，bytecode 驗證；跨架構），同時解 65544 與 65548；1.16.4 bytecode 證實開機期無條件 setIcon、無 macOS guard。待實測 1.16.4 / 1.17.1 啟動

---

## 風險與注意事項

| 風險 | 影響 | 因應方式 |
|------|------|----------|
| Minecraft 啟動參數複雜 | M3 延期 | 提前研究 Minecraft launcher spec |
| 跨平台 JVM 路徑差異 | M3/M5 | 使用 `directories` + 平台判斷 |
| GitHub Actions 打包時間不足 | M6 | 在 M5 就建立 CI 雛形 |

---

*此檔案由 Claude Code PM 在每次對話開始時自動檢查並更新*
