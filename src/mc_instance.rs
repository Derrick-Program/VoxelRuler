use notify_debouncer_mini::{
    Debouncer, new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;
use tracing::error;

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

pub struct InstanceStore {
    base_dir: PathBuf,
}

impl InstanceStore {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn load(&self) -> anyhow::Result<Vec<InstanceConfig>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }
        let mut instances = Vec::new();
        for entry in std::fs::read_dir(&self.base_dir)? {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let instance_toml = entry.path().join("instance.toml");
            if !instance_toml.is_file() {
                continue;
            }
            let content = match std::fs::read_to_string(&instance_toml) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let cfg: InstanceConfig = match toml::from_str(&content) {
                Ok(c) => c,
                Err(_) => continue,
            };
            instances.push(cfg);
        }
        Ok(instances)
    }

    pub fn save_one(&self, instance: &InstanceConfig) -> anyhow::Result<()> {
        let dir = self.base_dir.join(&instance.id);
        std::fs::create_dir_all(&dir)?;
        let content = toml::to_string_pretty(instance)?;
        std::fs::write(dir.join("instance.toml"), content)?;
        Ok(())
    }

    pub fn delete_one(&self, instance_id: &str) -> anyhow::Result<()> {
        let dir = self.base_dir.join(instance_id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }

    pub fn append(&self, instance: InstanceConfig) -> anyhow::Result<Vec<InstanceConfig>> {
        self.save_one(&instance)?;
        self.load()
    }

    pub fn watch_changes(&self) -> anyhow::Result<(Debouncer<RecommendedWatcher>, Receiver<()>)> {
        let (tx, rx) = channel();
        if !self.base_dir.exists() {
            std::fs::create_dir_all(&self.base_dir)?;
        }
        let mut debouncer = new_debouncer(
            Duration::from_millis(200),
            move |res: notify_debouncer_mini::DebounceEventResult| match res {
                Ok(events) => {
                    // 只有 instance.toml 變動才通知 UI 重建列表。
                    // 遊戲執行期間寫入的 logs / saves / options.txt 等檔案
                    // 不屬於 instance.toml，不會觸發不必要的列表刷新。
                    let relevant = events.iter().any(|e| {
                        e.path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n == "instance.toml")
                            .unwrap_or(false)
                    });
                    if relevant {
                        let _ = tx.send(());
                    }
                }
                Err(errors) => {
                    error!(?errors, "notify watcher 錯誤");
                }
            },
        )?;

        debouncer
            .watcher()
            .watch(&self.base_dir, RecursiveMode::Recursive)?;
        Ok((debouncer, rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_store() -> (InstanceStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path().to_path_buf());
        (store, dir)
    }

    #[test]
    fn test_instance_store_load_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = InstanceStore::new(dir.path().join("no-such-dir"));
        let result = store.load().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_instance_store_append_and_reload() {
        let (store, _dir) = tmp_store();
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
        let (store, _dir) = tmp_store();
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
        let (store, _dir) = tmp_store();
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
        assert!(loaded.iter().any(|c| c.name == "Instance 2"));
    }

    #[test]
    fn test_instance_store_delete_one() {
        let (store, _dir) = tmp_store();
        let cfg = InstanceConfig {
            id: "del-id".into(),
            name: "To Delete".into(),
            version: "1.20.4".into(),
            ..Default::default()
        };
        store.append(cfg).unwrap();
        assert_eq!(store.load().unwrap().len(), 1);

        store.delete_one("del-id").unwrap();
        assert!(store.load().unwrap().is_empty());
    }
}
