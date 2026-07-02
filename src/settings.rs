use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::warn;

/// 全域應用程式設定，存於 `<data_dir>/settings.toml`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)] // 確保反序列化時，缺失的欄位都會補上預設值
pub struct AppSettings {
    /// Java 來源模式：`"minecraft"` (Follow Minecraft provided, default) / `"custom"`（自訂路徑）
    /// 空字串視同 `"minecraft"`（向下相容）
    pub java_mode: String,
    /// 全域自訂 Java 執行檔路徑（僅 java_mode = "custom" 時生效）
    pub java_path: String,
}

impl AppSettings {
    /// 取得系統預設的設定檔路徑
    fn default_settings_path() -> anyhow::Result<PathBuf> {
        crate::PROJECT_DIR
            .as_ref()
            .map(|dir| dir.data_dir().join("settings.toml"))
            .ok_or_else(|| anyhow::anyhow!("Failed to get system app directory"))
    }

    /// 判斷目前是否啟用自訂 Java 路徑
    pub fn is_custom_java(&self) -> bool {
        self.java_mode == "custom"
    }

    /// 從指定路徑載入設定 (支援依賴注入與可測試性)
    pub fn load_from_path(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
                warn!(path = %path.display(), error = %e, "Failed to parse settings.toml, using defaults");
                Self::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // 檔案不存在是預期內的正常情況
                Self::default()
            }
            Err(e) => {
                // 其他非預期的 IO 錯誤 (如權限不足、磁碟損壞) 應留下紀錄
                warn!(path = %path.display(), error = %e, "IO error while reading settings.toml, using defaults");
                Self::default()
            }
        }
    }

    /// 讀取全域設定；檔案不存在或解析失敗時回傳預設值（不阻擋啟動）
    pub fn load() -> Self {
        Self::default_settings_path()
            .map(|path| Self::load_from_path(&path))
            .unwrap_or_default()
    }

    /// 儲存設定至指定路徑
    pub fn save_to_path(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// 儲存全域設定
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
        };
        let toml_str = toml::to_string_pretty(&s).unwrap();
        let back: AppSettings = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.java_mode, "custom");
        assert_eq!(back.java_path, "/usr/bin/java");
    }

    #[test]
    fn test_settings_file_io() {
        // 現在我們能夠真正對 I/O 進行單元測試 (Testability ++ )
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("settings.toml");

        // 測試寫入
        let s = AppSettings {
            java_mode: "custom".into(),
            ..Default::default()
        };
        s.save_to_path(&file_path).unwrap();

        // 測試讀取
        let loaded = AppSettings::load_from_path(&file_path);
        assert_eq!(loaded.java_mode, "custom");
    }
}
