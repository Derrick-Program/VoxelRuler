# Git 提交規範（Git Commit Conventions）

> VoxelRuler 專案適用 | 版本：1.0 | 建立：2026-05-19

---

## 分支策略

```
main          ← 正式版本，僅透過 PR + Squash 合併，需打 tag
  └── develop ← 整合分支，日常開發的基底
        ├── feat/xxx   ← 新功能
        └── fix/xxx    ← 修復
```

### 分支命名規則

| 類型 | 格式 | 範例 |
|------|------|------|
| 新功能 | `feat/<簡述>` | `feat/account-oauth-ui` |
| 修復 | `fix/<簡述>` | `fix/token-refresh-crash` |
| 重構 | `refactor/<簡述>` | `refactor/instance-data-model` |
| 文件 | `docs/<簡述>` | `docs/update-readme` |
| CI/CD | `ci/<簡述>` | `ci/github-actions-multiplatform` |

---

## Commit Message 格式

採用 **Conventional Commits** 規範：

```
<type>(<scope>): <描述>

[可選：詳細說明，換行後撰寫]
```

### Type 類型

| Type | 用途 |
|------|------|
| `feat` | 新增功能 |
| `fix` | 修復 Bug |
| `refactor` | 重構（不影響外部行為） |
| `style` | 純格式調整（空白、縮排等） |
| `docs` | 文件更新 |
| `test` | 新增或修改測試 |
| `ci` | CI/CD 設定 |
| `chore` | 其他雜項（依賴更新等） |
| `perf` | 效能改善 |

### Scope 範圍（建議）

| Scope | 對應模組 |
|-------|----------|
| `ui` | `.slint` UI 檔案 |
| `auth` | `mc_token.rs` 認證相關 |
| `instance` | 實例管理邏輯 |
| `launcher` | Minecraft 啟動邏輯 |
| `download` | 檔案下載相關 |
| `ci` | GitHub Actions |
| `theme` | `theme.slint` |

### 範例

```bash
feat(auth): 新增帳號頁面登入狀態顯示
fix(instance): 修正實例列表重複載入問題
refactor(ui): 將 account.slint 登入/登出拆成獨立元件
ci: 新增 Windows 平台 GitHub Actions 編譯步驟
```

---

## PR 流程

### feat/* / fix/* → develop

1. 從 `develop` 建立分支
2. 開發完成後，在 GitHub 發 PR 到 `develop`
3. 另一人 Code Review（至少 1 個 approve）
4. **Merge commit**（保留 commit 歷史）

### develop → main（版本發布）

1. 確認 `develop` 穩定可發布
2. 在 GitHub 發 PR 到 `main`
3. Review 通過後 **Squash and merge**
4. 立即打 tag：

```bash
git tag v1.0.0
git push origin v1.0.0
```

---

## Tag 命名規則

採用 **Semantic Versioning**：`v<MAJOR>.<MINOR>.<PATCH>`

| 版本號 | 含義 | 範例 |
|--------|------|------|
| MAJOR | 破壞性變更或重大里程碑 | v2.0.0 |
| MINOR | 新增功能（向下相容） | v1.1.0 |
| PATCH | 修復 Bug | v1.0.1 |

---

## 禁止事項

- ❌ 直接 push 到 `main`
- ❌ 在 `develop` 上直接開發（要開分支）
- ❌ 使用 `git push --force`（緊急情況需兩人確認）
- ❌ Commit message 使用 `fix bug`、`update`、`修改` 等模糊描述
