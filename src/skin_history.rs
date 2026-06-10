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

#[derive(Deserialize)]
struct PrismSkinIndex {
    skins: Vec<PrismSkin>,
}

#[derive(Deserialize)]
struct PrismSkin {
    model: Option<String>,
    name: Option<String>,
    url: Option<String>,
}

impl SkinHistory {
    pub fn load(path: &PathBuf) -> Self {
        if let Ok(data) = std::fs::read_to_string(path) {
            if let Ok(h) = serde_json::from_str(&data) {
                return h;
            }
        }
        let mut history = Self::default();
        history.import_from_prism();
        history
    }

    pub fn import_from_prism(&mut self) {
        if let Some(base_dirs) = directories::BaseDirs::new() {
            let prism_path = base_dirs.data_dir().join("PrismLauncher").join("skins").join("index.json");
            if let Ok(data) = std::fs::read_to_string(prism_path) {
                if let Ok(index) = serde_json::from_str::<PrismSkinIndex>(&data) {
                    for skin in index.skins {
                        if let Some(url) = skin.url {
                            let variant = skin.model.unwrap_or_else(|| "classic".to_string());
                            let variant = if variant.eq_ignore_ascii_case("slim") {
                                "slim".to_string()
                            } else {
                                "classic".to_string()
                            };
                            let name = skin.name.unwrap_or_else(|| "Prism Skin".to_string());
                            let entry = SkinEntry {
                                cape_id: "".to_string(),
                                name,
                                model: variant,
                                url,
                            };
                            self.add_skin(entry);
                        }
                    }
                }
            }
        }
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
