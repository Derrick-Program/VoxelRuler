# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## PM 啟動程序（每次對話必做）

**每次對話開始，必須**：
1. 讀取 `.claude/rule/TODO.md` — 確認目前里程碑進度
2. 若使用者有完成事項，更新 TODO.md 的狀態
3. 將本次對話重點記錄至 `.claude/conversations/YYYY-MM-DD.md`
4. 如 CLAUDE.md 有需更新的資訊，自行維護

---

## 專案概覽

**VoxelRuler** — Minecraft Launcher（Rust + Slint GUI）

| 項目 | 內容 |
|------|------|
| 期限 | 2026-06-30 |
| 目標 | 使用者可點選實例真正啟動 Minecraft 遊玩 |
| 平台 | Windows / macOS / Linux |
| 團隊 | 2 人，無特定分工 |

### 里程碑快覽

| 里程碑 | 期間 | 主要目標 |
|--------|------|---------|
| M1 | 5/19–5/25 | Account 頁面串接 OAuth |
| M2 | 5/26–6/01 | Instances 資料層 |
| M3 | 6/02–6/08 | 啟動 Minecraft（核心） |
| M4 | 6/09–6/15 | 實例 CRUD |
| M5 | 6/16–6/22 | 遊戲下載與進度 |
| M6 | 6/23–6/30 | 多平台打包與 v1.0.0 發布 |

詳細進度：`.claude/rule/TODO.md`

---

## Commands

```bash
# Build
cargo build

# Build (release)
cargo build --release

# Run
cargo run

# Check for compile errors without building
cargo check

# Run tests
cargo test

# Run a single test
cargo test <test_name>

# Lint
cargo clippy
```

---

## Architecture

VoxelRuler is a Minecraft launcher built with Rust + [Slint](https://slint.dev/) for the GUI. The UI is defined entirely in `.slint` files compiled at build time via `build.rs`.

### Entry Point

`src/main.rs` → `open_view()` in `src/view.rs`

**Startup flow**:
1. Load session from disk (`mc_token::SessionData::load_session()`)
2. Refresh token if expired (`mc_token::refresh_minecraft_token`)
3. Store valid token in `GLOBAL_CACHE` (`DashMap<String, String>`)
4. Launch Slint UI via `open_view()`

### Build System

`build.rs` compiles `ui/main.slint` with three registered library aliases:
- `@material` → `ui/libs/material-1.0/material.slint`
- `@theme` → `ui/theme.slint` (global `AppTheme` singleton)
- `@assets` → `ui/assets/index.slint` (global `Assets` singleton)

Translations: `ui/translations/` (supports `zh_TW` and `en_US`). Use `@tr("...")` in `.slint` files.

### UI Layer (`ui/`)

```
ui/
├── main.slint          # Root window (MainApp)
├── theme.slint         # AppTheme global: colors, radius, font sizes
├── global.slint        # Cross-component callbacks
├── assets/index.slint  # All image references
└── components/
    ├── footer.slint
    ├── file-progress.slint   # Download progress dialog
    ├── url-dialog.slint      # OAuth URL display
    ├── sidebar/              # SideBar, SideButton, Avatar
    └── pages/
        ├── home.slint
        ├── instances.slint   # InstanceCard grid, Search, AddInstance
        └── account.slint     # Login/logout UI
```

**Navigation**: `SideTabId` enum (`home`, `instances`, `mods`, `downloads`, `account`, `settings`)

WIP pages: `mods`, `downloads`, `settings`（WIP placeholder）

### Rust Layer (`src/`)

```
src/
├── main.rs       # Entry: token init → open_view()
├── view.rs       # Slint UI init + event binding
├── mc_token.rs   # Microsoft OAuth + Minecraft token management
├── mc_action.rs  # Minecraft API calls
└── mc_types.rs   # Shared type definitions
```

### Theme System

`AppTheme` global in `ui/theme.slint`. Toggle: `AppTheme.is-dark`. Colors: `AppTheme.colors.<token>`. Minecraft constants: `mc-green`, `mc-grass`, `mc-dirt`, `mc-stone`, `mc-diamond`.

### Adding New Icons

1. Add SVG to `ui/assets/icons/`
2. Register in `ui/assets/index.slint` as `out property <image>`
3. Use via `Assets.<name>`

---

## Git Workflow

```
main ← Squash merge from develop + git tag v*.*.*
  └── develop ← integration branch
        ├── feat/<name>  ← new features
        └── fix/<name>   ← bug fixes
```

Commit format: `type(scope): description` (Conventional Commits)

詳細規範：`.claude/docs/git-conventions.md`

---

## 協作文件

| 文件 | 路徑 |
|------|------|
| 進度追蹤 | `.claude/rule/TODO.md` |
| **工作分配表** | `.claude/docs/task-assignment.md` |
| Git 規範 | `.claude/docs/git-conventions.md` |
| 開發規範 | `.claude/docs/dev-standards.md` |
| 功能規格書 | `.claude/docs/feature-specs.md` |
| 對話記錄 | `.claude/conversations/YYYY-MM-DD.md` |
