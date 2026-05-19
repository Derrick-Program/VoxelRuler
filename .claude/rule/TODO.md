# VoxelRuler 專案進度追蹤

> 專案期限：2026-06-30 | 成員：Derrick、mlask | 最後更新：2026-05-19  
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
| M1：Account 頁面串接完成 | 5/19 – 5/25 | 🔄 進行中 |
| M2：Instances 資料層建立 | 5/26 – 6/01 | ⬜ 待開始 |
| M3：啟動 Minecraft（核心功能） | 6/02 – 6/08 | ⬜ 待開始 |
| M4：實例 CRUD 管理完成 | 6/09 – 6/15 | ⬜ 待開始 |
| M5：遊戲檔案下載與進度顯示 | 6/16 – 6/22 | ⬜ 待開始 |
| M6：多平台打包、測試、v1.0.0 | 6/23 – 6/30 | ⬜ 待開始 |

---

## M1：Account 頁面串接（5/19 – 5/25）

### 目標
- [ ] `account.slint` UI 顯示登入狀態（已登入 / 未登入）
- [ ] 已登入時顯示使用者名稱與頭像
- [ ] 未登入時顯示「登入」按鈕，觸發 Microsoft OAuth 流程
- [ ] 登出功能（清除 session）
- [ ] Rust 端 `view.rs` 與 `mc_token.rs` 串接 `GLOBAL_CACHE` 中的 token 狀態

### 完成標準
- 冷啟動時，若有有效 token，帳號頁自動顯示已登入狀態
- 點選登入按鈕可完整走完 OAuth 流程並回到已登入狀態
- 登出後清除 session 檔案，回到未登入狀態

---

## M2：Instances 資料層建立（5/26 – 6/01）

### 目標
- [ ] 定義 `InstanceData` 資料結構（名稱、版本、遊戲類型、圖示路徑等）
- [ ] 實例儲存格式（JSON / TOML）與讀寫邏輯
- [ ] `instances.slint` 從 Rust 端接收實例列表並顯示
- [ ] 搜尋功能串接

### 完成標準
- 實例列表可從磁碟讀取並顯示在 UI
- 搜尋可過濾實例名稱

---

## M3：啟動 Minecraft（6/02 – 6/08）【核心功能】

### 目標
- [ ] 調用 Minecraft API 取得啟動參數
- [ ] 下載並驗證 JVM 與遊戲版本（若未存在）
- [ ] 組合啟動指令並 `std::process::Command` 執行
- [ ] 啟動時顯示進度（`file-progress.slint`）
- [ ] 啟動後 UI 反饋（成功 / 錯誤訊息）

### 完成標準
- 點選實例中的啟動按鈕，Minecraft 真正開啟且可正常遊玩

---

## M4：實例 CRUD 管理（6/09 – 6/15）

### 目標
- [ ] 新增實例（選擇版本、命名）
- [ ] 刪除實例（含確認對話框）
- [ ] 編輯實例設定（記憶體、Java 路徑等）
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

---

## 風險與注意事項

| 風險 | 影響 | 因應方式 |
|------|------|----------|
| Minecraft 啟動參數複雜 | M3 延期 | 提前研究 Minecraft launcher spec |
| 跨平台 JVM 路徑差異 | M3/M5 | 使用 `directories` + 平台判斷 |
| GitHub Actions 打包時間不足 | M6 | 在 M5 就建立 CI 雛形 |

---

*此檔案由 Claude Code PM 在每次對話開始時自動檢查並更新*
