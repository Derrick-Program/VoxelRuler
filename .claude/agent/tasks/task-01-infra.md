# Task 01 — 基礎設施

> **依賴**：無（可立即執行）  
> **預計時間**：5–10 分鐘  
> **分支**：feat/mc/new_instance

---

## 目標

為 `mc_instance` 模組添加必要的 Cargo 依賴，在 `main.rs` 宣告模組，在 `mc_paths.rs` 新增 `instances_file()` 方法。

---

## 步驟

### Step 1：Cargo.toml — 新增 toml 和 uuid 依賴

在 `Cargo.toml` 的 `[dependencies]` 區塊中，找到 `serde_json = "1.0.149"` 這行，在它之後新增：

```toml
toml = "0.8"
uuid = { version = "1", features = ["v4", "fast-rng"] }
```

### Step 2：main.rs — 宣告 mc_instance 模組

在 `src/main.rs` 中，找到 `mod mc_paths;` 這行，在它之後新增：

```rust
mod mc_instance;
```

### Step 3：mc_paths.rs — 新增 instances_file() 方法

在 `src/mc_paths.rs` 中，找到 `pub fn instance_dir(&self, instance_id: &str) -> PathBuf {` 這個方法，在它之前（即在 `pub fn java_bin` 結束後、`pub fn instance_dir` 開始前）插入：

```rust
pub fn instances_file(&self) -> PathBuf {
    self.base.join("instances.toml")
}
```

### Step 4：驗證編譯

執行：
```bash
cd /Users/derrick/Documents/Program/rust/Project/VoxelRuler
cargo check
```
預期：無錯誤（`mc_instance` 模組不存在會報 error，這是正常的 — 下個任務會建立它）

> **注意**：此時 `cargo check` 會因為 `mod mc_instance;` 找不到 `src/mc_instance.rs` 而報錯。這是預期行為。我們只需確保其他兩個改動（Cargo.toml, mc_paths.rs）沒有語法錯誤。

實際驗證指令：
```bash
cd /Users/derrick/Documents/Program/rust/Project/VoxelRuler
# 暫時把 mod mc_instance 注解掉確認其他部分沒問題
cargo check 2>&1 | grep -v "mc_instance"
```

### Step 5：提交

```bash
cd /Users/derrick/Documents/Program/rust/Project/VoxelRuler
git add Cargo.toml src/main.rs src/mc_paths.rs
git commit -m "feat: add toml/uuid deps and mc_instance module scaffold"
```

---

## 完成後

更新 `.claude/agent/status.md`，將 Batch 1 改為 `✅ 完成`，並記錄完成時間。
