use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::warn;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkinEntry {
    pub cape_id: String,
    pub model: String,
    pub name: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SkinHistory {
    pub skins: Vec<SkinEntry>,
}

impl SkinHistory {
    pub fn load(path: &Path) -> Self {
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(data) => match serde_json::from_str(&data) {
                    Ok(h) => return h,
                    Err(e) => {
                        warn!(path = %path.display(), error = %e, "Failed to parse skin history JSON, falling back to default")
                    }
                },
                Err(e) => {
                    warn!(path = %path.display(), error = %e, "Failed to read skin history file, falling back to default")
                }
            }
        }

        let history = Self::default();
        if let Err(e) = history.save(path) {
            warn!(path = %path.display(), error = %e, "Failed to save default skin history");
        }
        history
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    pub fn add_skin(&mut self, entry: SkinEntry) {
        if !self.skins.iter().any(|s| s.url == entry.url) {
            self.skins.push(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_entry(url: &str, name: &str) -> SkinEntry {
        SkinEntry {
            cape_id: String::new(),
            model: "classic".into(),
            name: name.into(),
            url: url.into(),
        }
    }

    #[test]
    fn test_load_returns_default_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("index.json");
        let h = SkinHistory::load(&path);
        assert!(h.skins.is_empty());
        assert!(path.exists());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("index.json");

        let mut h = SkinHistory::default();
        h.skins
            .push(make_entry("https://example.com/skin.png", "MySkin"));
        h.save(&path).unwrap();

        let loaded = SkinHistory::load(&path);
        assert_eq!(loaded.skins.len(), 1);
        assert_eq!(loaded.skins[0].name, "MySkin");
        assert_eq!(loaded.skins[0].url, "https://example.com/skin.png");
    }

    #[test]
    fn test_add_skin_deduplicates_by_url() {
        let mut h = SkinHistory::default();
        h.add_skin(make_entry("https://example.com/a.png", "Skin A"));
        h.add_skin(make_entry("https://example.com/a.png", "Skin A duplicate"));
        h.add_skin(make_entry("https://example.com/b.png", "Skin B"));
        assert_eq!(h.skins.len(), 2);
        assert_eq!(h.skins[0].name, "Skin A");
        assert_eq!(h.skins[1].name, "Skin B");
    }

    #[test]
    fn test_add_skin_different_urls_are_kept() {
        let mut h = SkinHistory::default();
        for i in 0..5 {
            h.add_skin(make_entry(
                &format!("https://example.com/{}.png", i),
                &format!("Skin {}", i),
            ));
        }
        assert_eq!(h.skins.len(), 5);
    }

    #[test]
    fn test_load_invalid_json_falls_back_to_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("index.json");
        std::fs::write(&path, b"not valid json").unwrap();
        let h = SkinHistory::load(&path);
        assert!(h.skins.is_empty());
    }
}
