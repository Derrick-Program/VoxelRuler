# Agent 協調中心 — Create Instance Dialog

> 最後更新：2026-05-27  
> 功能分支：`feat/mc/new_instance`  
> 完整計劃：`docs/superpowers/plans/2026-05-27-create-instance-dialog.md`

---

## 任務概覽

| 批次 | 任務檔案 | 涵蓋檔案 | 依賴 |
|------|----------|---------|------|
| Batch 1 | `tasks/task-01-infra.md` | Cargo.toml, main.rs, mc_paths.rs | 無 |
| Batch 2 | `tasks/task-02-model.md` | src/mc_instance.rs | Batch 1 |
| Batch 3 | `tasks/task-03-ui.md` | global.slint, create-instance-dialog.slint, index.slint, main.slint | Batch 1 |
| Batch 4 | `tasks/task-04-view.md` | src/view.rs | Batch 2 + Batch 3 |

> Batch 2 和 Batch 3 可以平行執行（不相互依賴），但都需要 Batch 1 先完成。

---

## 執行狀態

詳見 `status.md`

---

## 架構要點

- `InstanceStore` → TOML 讀寫（`~/.local/share/.../instances.toml`）
- `Arc<Mutex<Vec<InstanceConfig>>>` → master in-memory store（跨 callback 共享）
- `Rc<VecModel<InstanceData>>` → Slint UI model（UI thread only）
- 搜尋：從 master store 篩選，重建 VecModel
- 新增：append to TOML → 更新 master → 重建 UI list
- MC 版本列表：app 啟動時背景 fetch，via `slint::invoke_from_event_loop` 設回 UI

---

## 關鍵注意事項

1. `InstanceCreateLogic` 使用 `selected-version: string`（不是 index），綁定 ComboBox 的 `current-value`
2. `do_launch()` 新增 `xmx: String, xms: String` 參數，從 InstanceConfig 讀取
3. dialog 放在 main.slint 最後，with `x: 0; y: 0; width: root.width; height: root.height`（全覆蓋 overlay）
4. `InstanceCreateLogic` 從 `@global` 別名（`ui/global.slint`）import
5. `confirm-create` callback 在 UI thread 執行同步 TOML write（檔案小，可接受）
