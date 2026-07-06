use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::warn;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortMode {
    Name,
    Version,
    CreatedAt,
    LastPlayed,
}

impl Default for SortMode {
    fn default() -> Self {
        SortMode::CreatedAt
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub java_mode: String,
    pub java_path: String,
    pub sort_mode: SortMode,
    pub sort_ascending: bool,
}

impl AppSettings {
    fn default_settings_path() -> anyhow::Result<PathBuf> {
        crate::PROJECT_DIR
            .as_ref()
            .map(|dir| dir.data_dir().join("settings.toml"))
            .ok_or_else(|| anyhow::anyhow!("Failed to get system app directory"))
    }

    pub fn is_custom_java(&self) -> bool {
        self.java_mode == "custom"
    }

    pub fn load_from_path(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
                warn!(path = %path.display(), error = %e, "Failed to parse settings.toml, using defaults");
                Self::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Self::default()
            }
            Err(e) => {
                warn!(path = %path.display(), error = %e, "IO error while reading settings.toml, using defaults");
                Self::default()
            }
        }
    }

    pub fn load() -> Self {
        Self::default_settings_path()
            .map(|path| Self::load_from_path(&path))
            .unwrap_or_default()
    }

    pub fn save_to_path(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::default_settings_path()?;
        self.save_to_path(&path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_settings_default_is_empty() {
        let s = AppSettings::default();
        assert!(s.java_mode.is_empty());
        assert!(s.java_path.is_empty());
        assert!(!s.is_custom_java());
    }

    #[test]
    fn test_settings_parse_partial_toml() {
        let s: AppSettings = toml::from_str("").unwrap();
        assert!(s.java_mode.is_empty());
        assert!(s.java_path.is_empty());

        let s: AppSettings = toml::from_str(r#"java_mode = "custom""#).unwrap();
        assert_eq!(s.java_mode, "custom");
        assert!(s.java_path.is_empty());
        assert!(s.is_custom_java());
    }

    #[test]
    fn test_settings_roundtrip() {
        let s = AppSettings {
            java_mode: "custom".into(),
            java_path: "/usr/bin/java".into(),
            ..Default::default()
        };
        let toml_str = toml::to_string_pretty(&s).unwrap();
        let back: AppSettings = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.java_mode, "custom");
        assert_eq!(back.java_path, "/usr/bin/java");
    }

    #[test]
    fn test_settings_file_io() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("settings.toml");

        let s = AppSettings {
            java_mode: "custom".into(),
            ..Default::default()
        };
        s.save_to_path(&file_path).unwrap();

        let loaded = AppSettings::load_from_path(&file_path);
        assert_eq!(loaded.java_mode, "custom");
    }

    #[test]
    fn test_sort_mode_defaults_to_created_at() {
        let s = AppSettings::default();
        assert_eq!(s.sort_mode, SortMode::CreatedAt);
        assert!(!s.sort_ascending);
    }

    #[test]
    fn test_sort_mode_toml_roundtrip() {
        let s = AppSettings {
            sort_mode: SortMode::Name,
            sort_ascending: true,
            ..Default::default()
        };
        let toml_str = toml::to_string_pretty(&s).unwrap();
        let back: AppSettings = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.sort_mode, SortMode::Name);
        assert!(back.sort_ascending);
    }

    #[test]
    fn test_sort_mode_missing_from_old_toml_defaults() {
        let s: AppSettings = toml::from_str(r#"java_mode = "custom""#).unwrap();
        assert_eq!(s.sort_mode, SortMode::CreatedAt);
        assert!(!s.sort_ascending);
    }
}
