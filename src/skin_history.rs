use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    pub fn load(path: &PathBuf) -> Self {
        if let Ok(data) = std::fs::read_to_string(path) {
            if let Ok(h) = serde_json::from_str(&data) {
                return h;
            }
        }
        let history = Self::default();
        let _ = history.save(path);
        history
    }

    pub fn save(&self, path: &PathBuf) -> anyhow::Result<()> {
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
