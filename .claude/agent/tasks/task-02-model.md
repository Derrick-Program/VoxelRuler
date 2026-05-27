# Task 02 — 資料層 (mc_instance.rs)

> **依賴**：Task 01 完成後執行  
> **預計時間**：10–15 分鐘  
> **分支**：feat/mc/new_instance

---

## 目標

建立 `src/mc_instance.rs`，包含 `InstanceConfig` struct、`InstanceStore` 讀寫邏輯，以及 4 個單元測試。

---

## 步驟

### Step 1：建立 src/mc_instance.rs

建立 `/Users/derrick/Documents/Program/rust/Project/VoxelRuler/src/mc_instance.rs`，內容如下：

```rust
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize, Deserialize, Default)]
struct InstancesFile {
    #[serde(default)]
    instances: Vec<InstanceConfig>,
}

pub struct InstanceStore {
    path: PathBuf,
}

impl InstanceStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> anyhow::Result<Vec<InstanceConfig>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&self.path)?;
        let file: InstancesFile = toml::from_str(&content)?;
        Ok(file.instances)
    }

    pub fn save(&self, instances: &[InstanceConfig]) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = InstancesFile {
            instances: instances.to_vec(),
        };
        let content = toml::to_string_pretty(&file)?;
        std::fs::write(&self.path, content)?;
        Ok(())
    }

    pub fn append(&self, instance: InstanceConfig) -> anyhow::Result<Vec<InstanceConfig>> {
        let mut instances = self.load()?;
        instances.push(instance);
        self.save(&instances)?;
        Ok(instances)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_store() -> (InstanceStore, tempfile::NamedTempFile) {
        let f = tempfile::NamedTempFile::new().unwrap();
        let store = InstanceStore::new(f.path().to_path_buf());
        (store, f)
    }

    #[test]
    fn test_instance_store_load_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path().join("no-such.toml"));
        let result = store.load().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_instance_store_append_and_reload() {
        let (store, _f) = tmp_store();
        let cfg = InstanceConfig {
            id: "test-id".into(),
            name: "Test Instance".into(),
            version: "1.20.4".into(),
            ..Default::default()
        };
        let result = store.append(cfg).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Test Instance");

        let reloaded = store.load().unwrap();
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].id, "test-id");
    }

    #[test]
    fn test_instance_store_save_preserves_all_fields() {
        let (store, _f) = tmp_store();
        let cfg = InstanceConfig {
            id: "full-id".into(),
            name: "Full Test".into(),
            version: "1.20.4".into(),
            mod_loader: "Fabric".into(),
            xmx: "4G".into(),
            xms: "1G".into(),
            logs_enabled: false,
            world_path: "/some/world".into(),
            resource_pack: "/some/pack".into(),
            shader_pack: "/some/shader".into(),
            last_played: "2024-01-01T00:00:00Z".into(),
            play_time_secs: 3600,
        };
        store.append(cfg).unwrap();
        let loaded = store.load().unwrap();
        let c = &loaded[0];
        assert_eq!(c.mod_loader, "Fabric");
        assert_eq!(c.xmx, "4G");
        assert_eq!(c.xms, "1G");
        assert!(!c.logs_enabled);
        assert_eq!(c.world_path, "/some/world");
        assert_eq!(c.play_time_secs, 3600);
    }

    #[test]
    fn test_instance_store_append_multiple() {
        let (store, _f) = tmp_store();
        for i in 0..3_usize {
            let cfg = InstanceConfig {
                id: format!("id-{i}"),
                name: format!("Instance {i}"),
                version: "1.20.4".into(),
                ..Default::default()
            };
            store.append(cfg).unwrap();
        }
        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[2].name, "Instance 2");
    }
}
```

### Step 2：執行測試

```bash
cd /Users/derrick/Documents/Program/rust/Project/VoxelRuler
cargo test mc_instance 2>&1
```

預期輸出：
```
test mc_instance::tests::test_instance_store_load_empty ... ok
test mc_instance::tests::test_instance_store_append_and_reload ... ok
test mc_instance::tests::test_instance_store_save_preserves_all_fields ... ok
test mc_instance::tests::test_instance_store_append_multiple ... ok

test result: ok. 4 passed; 0 failed
```

如果有錯誤，根據錯誤訊息修復，然後重新執行。

### Step 3：提交

```bash
cd /Users/derrick/Documents/Program/rust/Project/VoxelRuler
git add src/mc_instance.rs
git commit -m "feat: add InstanceConfig and InstanceStore with TOML persistence"
```

---

## 完成後

更新 `.claude/agent/status.md`，將 Batch 2 改為 `✅ 完成`，並記錄完成時間。
