use crate::settings::SortMode;
use notify_debouncer_mini::{
    Debouncer, new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;
use tracing::{error, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceConfig {
    pub id: String,
    pub name: String,
    pub version: String,
    pub mod_loader: String,
    #[serde(default)]
    pub mod_loader_version: String,
    pub xmx: String,
    pub xms: String,
    pub logs_enabled: bool,
    pub world_path: String,
    pub resource_pack: String,
    pub shader_pack: String,
    pub last_played: String,
    pub play_time_secs: u64,
    #[serde(default)]
    pub java_mode: String,
    #[serde(default)]
    pub java_path: String,
    #[serde(default)]
    pub created_at: i64,
}

impl Default for InstanceConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            version: String::new(),
            mod_loader: "None".into(),
            mod_loader_version: String::new(),
            xmx: "2G".into(),
            xms: "512M".into(),
            logs_enabled: true,
            world_path: String::new(),
            resource_pack: String::new(),
            shader_pack: String::new(),
            last_played: String::new(),
            play_time_secs: 0,
            java_mode: String::new(),
            java_path: String::new(),
            created_at: 0,
        }
    }
}

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();
    for i in 0..a_parts.len().max(b_parts.len()) {
        let ord = match (a_parts.get(i), b_parts.get(i)) {
            (Some(x), Some(y)) => match (x.parse::<u64>(), y.parse::<u64>()) {
                (Ok(xi), Ok(yi)) => xi.cmp(&yi),
                _ => x.cmp(y),
            },
            (Some(_), None) => Greater,
            (None, Some(_)) => Less,
            (None, None) => Equal,
        };
        if ord != Equal {
            return ord;
        }
    }
    Equal
}

fn sort_instances(instances: &mut [(InstanceConfig, i64)], mode: SortMode, ascending: bool) {
    use std::cmp::Ordering::*;
    instances.sort_by(|(a, a_key), (b, b_key)| match mode {
        SortMode::Name => {
            let ord = a.name.to_lowercase().cmp(&b.name.to_lowercase());
            if ascending { ord } else { ord.reverse() }
        }
        SortMode::Version => {
            let ord = compare_versions(&a.version, &b.version);
            if ascending { ord } else { ord.reverse() }
        }
        SortMode::CreatedAt => {
            let ord = a_key.cmp(b_key);
            if ascending { ord } else { ord.reverse() }
        }
        // Emptiness placement (never-played sinks last) must NOT flip with
        // ascending/descending, so only the non-empty branch gets reversed.
        SortMode::LastPlayed => match (a.last_played.is_empty(), b.last_played.is_empty()) {
            (true, true) => Equal,
            (true, false) => Greater,
            (false, true) => Less,
            (false, false) => {
                let ord = a.last_played.cmp(&b.last_played);
                if ascending { ord } else { ord.reverse() }
            }
        },
    });
}

pub struct InstanceStore {
    base_dir: PathBuf,
    sort_mode: SortMode,
    sort_ascending: bool,
}

impl InstanceStore {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            sort_mode: SortMode::default(),
            sort_ascending: false,
        }
    }

    pub fn set_sort(&mut self, mode: SortMode, ascending: bool) {
        self.sort_mode = mode;
        self.sort_ascending = ascending;
    }

    pub fn load(&self) -> anyhow::Result<Vec<InstanceConfig>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut instances = Vec::new();
        for entry_res in std::fs::read_dir(&self.base_dir)? {
            let entry = match entry_res {
                Ok(e) => e,
                Err(e) => {
                    warn!("Failed to read directory entry: {}", e);
                    continue;
                }
            };

            let path = entry.path().join("instance.toml");
            if !path.is_file() {
                continue;
            }

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    warn!(path = %path.display(), error = %e, "Failed to read instance config file");
                    continue;
                }
            };

            match toml::from_str::<InstanceConfig>(&content) {
                Ok(config) => {
                    // 不能只靠檔案時間排序：save_one 每次編輯都重寫 TOML，會更新 mtime 導致排序跳動
                    let sort_key = if config.created_at > 0 {
                        config.created_at
                    } else {
                        std::fs::metadata(&path)
                            .and_then(|m| m.created().or_else(|_| m.modified()))
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0)
                    };
                    instances.push((config, sort_key));
                }
                Err(e) => {
                    warn!(path = %path.display(), error = %e, "Failed to parse instance config TOML, instance ignored");
                }
            }
        }

        sort_instances(&mut instances, self.sort_mode, self.sort_ascending);
        Ok(instances.into_iter().map(|(c, _)| c).collect())
    }

    pub fn save_one(&self, instance: &InstanceConfig) -> anyhow::Result<()> {
        let dir = self.base_dir.join(&instance.id);
        std::fs::create_dir_all(&dir)?;
        let content = toml::to_string_pretty(instance)?;
        std::fs::write(dir.join("instance.toml"), content)?;
        Ok(())
    }

    pub fn record_launch_started(&self, master: &mut [InstanceConfig], id: &str) {
        if let Some(c) = master.iter_mut().find(|c| c.id == id) {
            c.last_played = chrono::Utc::now().to_rfc3339();
            let _ = self.save_one(c);
        }
    }

    pub fn record_play_session_end(
        &self,
        master: &mut [InstanceConfig],
        id: &str,
        elapsed_secs: u64,
    ) {
        if let Some(c) = master.iter_mut().find(|c| c.id == id) {
            c.play_time_secs += elapsed_secs;
            let _ = self.save_one(c);
        }
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
                    error!(?errors, "notify watcher error");
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
            mod_loader_version: "0.15.7".into(),
            xmx: "4G".into(),
            xms: "1G".into(),
            logs_enabled: false,
            world_path: "/some/world".into(),
            resource_pack: "/some/pack".into(),
            shader_pack: "/some/shader".into(),
            last_played: "2024-01-01T00:00:00Z".into(),
            play_time_secs: 3600,
            java_mode: "custom".into(),
            java_path: "/custom/java/bin/java".into(),
            created_at: 1_700_000_000,
        };
        store.append(cfg).unwrap();
        let loaded = store.load().unwrap();
        let c = &loaded[0];
        assert_eq!(c.mod_loader, "Fabric");
        assert_eq!(c.mod_loader_version, "0.15.7");
        assert_eq!(c.xmx, "4G");
        assert_eq!(c.xms, "1G");
        assert!(!c.logs_enabled);
        assert_eq!(c.world_path, "/some/world");
        assert_eq!(c.play_time_secs, 3600);
        assert_eq!(c.java_mode, "custom");
        assert_eq!(c.java_path, "/custom/java/bin/java");
        assert_eq!(c.created_at, 1_700_000_000);
    }

    #[test]
    fn test_load_sorts_by_created_at_newest_first() {
        let (store, _dir) = tmp_store();
        let old = InstanceConfig {
            id: "old".into(),
            name: "Old".into(),
            created_at: 100,
            ..Default::default()
        };
        let new = InstanceConfig {
            id: "new".into(),
            name: "New".into(),
            created_at: 200,
            ..Default::default()
        };
        store.save_one(&old).unwrap();
        store.save_one(&new).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "new", "newest instance should come first");
        assert_eq!(loaded[1].id, "old");
    }

    #[test]
    fn test_instance_config_backward_compat_without_java_fields() {
        let toml_str = r#"
            id = "old-id"
            name = "Old Instance"
            version = "1.20.4"
            mod_loader = "None"
            mod_loader_version = ""
            xmx = "2G"
            xms = "512M"
            logs_enabled = true
            world_path = ""
            resource_pack = ""
            shader_pack = ""
            last_played = ""
            play_time_secs = 0
        "#;
        let cfg: InstanceConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.java_mode.is_empty());
        assert!(cfg.java_path.is_empty());
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

    #[test]
    fn test_compare_versions_numeric_segments() {
        assert_eq!(
            compare_versions("1.7.2", "1.21.7"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_versions("1.7.10", "1.7.2"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_versions("1.20.4", "1.20.4"),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn test_compare_versions_non_numeric_fallback() {
        // Neither side of a mismatched segment parses as a number: falls back
        // to plain string comparison for that segment instead of panicking.
        let result = compare_versions("24w14a", "1.20.4");
        assert_eq!(result, "24w14a".cmp("1"));
    }

    #[test]
    fn test_sort_instances_by_name_ascending() {
        let (mut store, _dir) = tmp_store();
        store
            .save_one(&InstanceConfig {
                id: "b".into(),
                name: "Banana".into(),
                ..Default::default()
            })
            .unwrap();
        store
            .save_one(&InstanceConfig {
                id: "a".into(),
                name: "Apple".into(),
                ..Default::default()
            })
            .unwrap();

        store.set_sort(SortMode::Name, true);
        let loaded = store.load().unwrap();
        assert_eq!(loaded[0].name, "Apple");
        assert_eq!(loaded[1].name, "Banana");
    }

    #[test]
    fn test_sort_instances_by_version_descending() {
        let (mut store, _dir) = tmp_store();
        store
            .save_one(&InstanceConfig {
                id: "old".into(),
                version: "1.7.2".into(),
                ..Default::default()
            })
            .unwrap();
        store
            .save_one(&InstanceConfig {
                id: "new".into(),
                version: "1.21.7".into(),
                ..Default::default()
            })
            .unwrap();

        store.set_sort(SortMode::Version, false);
        let loaded = store.load().unwrap();
        assert_eq!(
            loaded[0].id, "new",
            "1.21.7 should sort above 1.7.2 descending"
        );
        assert_eq!(loaded[1].id, "old");
    }

    #[test]
    fn test_sort_instances_last_played_never_played_sinks_to_bottom() {
        let (mut store, _dir) = tmp_store();
        store
            .save_one(&InstanceConfig {
                id: "played".into(),
                last_played: "2024-01-01T00:00:00Z".into(),
                ..Default::default()
            })
            .unwrap();
        store
            .save_one(&InstanceConfig {
                id: "never".into(),
                last_played: "".into(),
                ..Default::default()
            })
            .unwrap();

        store.set_sort(SortMode::LastPlayed, true);
        let ascending = store.load().unwrap();
        assert_eq!(ascending.last().unwrap().id, "never");

        store.set_sort(SortMode::LastPlayed, false);
        let descending = store.load().unwrap();
        assert_eq!(descending.last().unwrap().id, "never");
    }

    #[test]
    fn test_default_sort_matches_previous_created_at_behavior() {
        let (store, _dir) = tmp_store();
        let old = InstanceConfig {
            id: "old".into(),
            created_at: 100,
            ..Default::default()
        };
        let new = InstanceConfig {
            id: "new".into(),
            created_at: 200,
            ..Default::default()
        };
        store.save_one(&old).unwrap();
        store.save_one(&new).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded[0].id, "new");
        assert_eq!(loaded[1].id, "old");
    }

    #[test]
    fn test_record_launch_started_sets_last_played() {
        let (store, _dir) = tmp_store();
        let cfg = InstanceConfig {
            id: "a".into(),
            name: "A".into(),
            ..Default::default()
        };
        store.save_one(&cfg).unwrap();
        let mut master = store.load().unwrap();
        assert!(master[0].last_played.is_empty());

        store.record_launch_started(&mut master, "a");

        assert!(!master[0].last_played.is_empty());
        assert!(chrono::DateTime::parse_from_rfc3339(&master[0].last_played).is_ok());

        let reloaded = store.load().unwrap();
        assert_eq!(reloaded[0].last_played, master[0].last_played);
    }

    #[test]
    fn test_record_launch_started_unknown_id_is_noop() {
        let (store, _dir) = tmp_store();
        let mut master: Vec<InstanceConfig> = vec![];
        store.record_launch_started(&mut master, "missing");
        assert!(master.is_empty());
    }

    #[test]
    fn test_record_play_session_end_accumulates_play_time() {
        let (store, _dir) = tmp_store();
        let cfg = InstanceConfig {
            id: "a".into(),
            name: "A".into(),
            play_time_secs: 100,
            ..Default::default()
        };
        store.save_one(&cfg).unwrap();
        let mut master = store.load().unwrap();

        store.record_play_session_end(&mut master, "a", 42);

        assert_eq!(master[0].play_time_secs, 142);
        let reloaded = store.load().unwrap();
        assert_eq!(reloaded[0].play_time_secs, 142);
    }

    #[test]
    fn test_record_play_session_end_unknown_id_is_noop() {
        let (store, _dir) = tmp_store();
        let mut master: Vec<InstanceConfig> = vec![];
        store.record_play_session_end(&mut master, "missing", 42);
        assert!(master.is_empty());
    }
}
