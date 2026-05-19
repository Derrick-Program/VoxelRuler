# 開發規範（Development Standards）

> VoxelRuler 專案適用 | 版本：1.0 | 建立：2026-05-19

---

## 技術棧

| 層級 | 技術 |
|------|------|
| 語言 | Rust（Edition 2021） |
| GUI | Slint 1.x |
| 非同步 | Tokio |
| HTTP | reqwest |
| 認證 | Microsoft OAuth 2.0 |
| 打包 | cargo-packer |
| CI/CD | GitHub Actions |

---

## Rust 編碼規範

### 命名慣例

| 項目 | 風格 | 範例 |
|------|------|------|
| 函式、變數 | `snake_case` | `load_session()` |
| 型別、Struct、Enum | `PascalCase` | `SessionData` |
| 常數 | `SCREAMING_SNAKE_CASE` | `GLOBAL_CACHE` |
| 模組 | `snake_case` | `mc_token` |

### 模組結構（`src/`）

```
src/
├── main.rs          # 入口點：初始化 token、開啟 view
├── view.rs          # Slint UI 初始化與事件綁定
├── mc_token.rs      # Microsoft OAuth + Minecraft token 管理
├── mc_action.rs     # Minecraft API 調用（啟動參數、版本清單等）
├── mc_types.rs      # 共用資料型別定義
└── instance/        # （待建立）實例資料管理
    ├── mod.rs
    ├── data.rs      # InstanceData 結構
    └── storage.rs   # 讀寫磁碟
```

### 錯誤處理

- 使用 `anyhow::Result` 作為函式回傳型別
- 對使用者可見的錯誤，透過 Slint callback 傳至 UI 顯示
- 不允許直接 `.unwrap()`（除非確保不會 panic，需加註解說明）

### 非同步規範

- Tokio runtime 由 `#[tokio::main]` 管理
- Slint UI 操作必須在主執行緒（透過 `slint::invoke_from_event_loop` 或 callback）
- 背景任務使用 `tokio::spawn`

### 效能

- `GLOBAL_CACHE`（`DashMap`）用於執行期跨模組共享狀態
- 避免不必要的 `clone()`，優先使用參照

---

## Slint UI 規範

### 檔案組織

```
ui/
├── main.slint           # 根視窗，只負責組合元件
├── theme.slint          # AppTheme global（禁止在此加業務邏輯）
├── global.slint         # 跨元件共用的 callback 宣告
├── components/
│   ├── pages/<page>.slint  # 每個頁面一個檔案
│   └── sidebar/         # 側邊欄元件
└── assets/index.slint   # 圖片資源集中管理
```

### 命名慣例

| 項目 | 風格 | 範例 |
|------|------|------|
| 元件 | `PascalCase` | `InstanceCard` |
| 屬性 | `kebab-case` | `is-logged-in` |
| Callback | `kebab-case` | `on-launch-clicked` |
| Global | `PascalCase` | `AppTheme`, `Assets` |

### 設計規範

- 所有顏色透過 `AppTheme.colors.*` 取用，不直接寫 hex
- 圖示透過 `Assets.*` 取用，不直接寫路徑
- 支援深/淺色模式：使用 `AppTheme.is-dark` 條件切換
- 多語系文字使用 `@tr("...")`

### Rust ↔ Slint 串接原則

- 資料傳入 UI：透過 `app.set_*()` 或 `ModelRc`
- UI 事件傳出：透過 `app.on_*()` callback
- 非同步操作結果回傳 UI：使用 `slint::invoke_from_event_loop`

---

## 測試規範

- 業務邏輯（`mc_action.rs`、`mc_token.rs`）需有單元測試
- UI 邏輯暫不強制測試（Slint 測試工具有限）
- 執行測試：`cargo test`
- 執行 lint：`cargo clippy -- -D warnings`（CI 中設為 Deny）

---

## 跨平台注意事項

| 平台 | 注意點 |
|------|--------|
| Windows | 路徑使用 `\`，透過 `std::path::PathBuf` 處理 |
| macOS | App 需放在 `~/Library/Application Support/` |
| Linux | 遵循 XDG 規範，使用 `directories::ProjectDirs` |

- 所有平台路徑統一使用 `directories::ProjectDirs::from("com", "Duacodie", "VoxelRuler")`
- 不允許寫死路徑字串

---

## Code Review 檢查清單

Pull Request 前確認：
- [ ] `cargo check` 無錯誤
- [ ] `cargo clippy` 無 warning
- [ ] `cargo test` 全部通過
- [ ] Commit message 符合規範
- [ ] 無寫死路徑、magic number
- [ ] 新增功能有對應 UI 反饋（成功/錯誤狀態）
