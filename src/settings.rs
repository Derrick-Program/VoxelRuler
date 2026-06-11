use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::warn;

/// 全域應用程式設定，存於 `<data_dir>/settings.toml`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppSettings {
    /// 全域自訂 Java 執行檔路徑（留空 = 未設定）
    #[serde(default)]
    pub java_path: String,
    /// 全域 Mojang Java runtime component（如 `java-runtime-gamma`；留空 = 未設定）
    #[serde(default)]
    pub java_runtime: String,
}

impl AppSettings {
    fn settings_path() -> anyhow::Result<PathBuf> {
        Ok(crate::PROJECT_DIR
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("無法取得系統應用程式目錄"))?
            .data_dir()
            .join("settings.toml"))
    }

    /// 讀取設定；檔案不存在或解析失敗時回傳預設值（不阻擋啟動）
    pub fn load() -> Self {
        let Ok(path) = Self::settings_path() else {
            return Self::default();
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        toml::from_str(&content).unwrap_or_else(|e| {
            warn!(path = %path.display(), error = %e, "settings.toml 解析失敗，使用預設值");
            Self::default()
        })
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::settings_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_default_is_empty() {
        let s = AppSettings::default();
        assert!(s.java_path.is_empty());
        assert!(s.java_runtime.is_empty());
    }

    #[test]
    fn test_settings_parse_partial_toml() {
        // 舊版 settings.toml 缺欄位也要能解析
        let s: AppSettings = toml::from_str("").unwrap();
        assert!(s.java_path.is_empty());

        let s: AppSettings = toml::from_str(r#"java_runtime = "java-runtime-delta""#).unwrap();
        assert_eq!(s.java_runtime, "java-runtime-delta");
        assert!(s.java_path.is_empty());
    }

    #[test]
    fn test_settings_roundtrip() {
        let s = AppSettings {
            java_path: "/usr/bin/java".into(),
            java_runtime: "jre-legacy".into(),
        };
        let toml_str = toml::to_string_pretty(&s).unwrap();
        let back: AppSettings = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.java_path, "/usr/bin/java");
        assert_eq!(back.java_runtime, "jre-legacy");
    }
}
