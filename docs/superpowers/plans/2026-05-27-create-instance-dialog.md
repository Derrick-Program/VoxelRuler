# Create Instance Dialog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `on_new_instance` with a 3-tab Create Instance dialog, TOML persistence, and real-time UI updates.

**Architecture:** `InstanceStore` (TOML on disk) ↔ `Arc<Mutex<Vec<InstanceConfig>>>` (master in-memory) ↔ `Rc<VecModel<InstanceData>>` (Slint UI). Search and confirm rebuild the UI model from master. Background tokio task fetches version list from MC API and sets it into `InstanceCreateLogic.version-list`.

**Tech Stack:** Rust + Slint 1.16.1, `toml = "0.8"`, `uuid = "1"` (v4+fast-rng), tokio (already present), serde (already present), tempfile (already present for tests).

---

### Task 1: Infrastructure (Cargo.toml + main.rs + mc_paths.rs)

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs`
- Modify: `src/mc_paths.rs`

- [ ] **Step 1: Add toml and uuid to Cargo.toml**

In `[dependencies]` section, after `serde_json = "1.0.149"` line, add:
```toml
toml = "0.8"
uuid = { version = "1", features = ["v4", "fast-rng"] }
```

- [ ] **Step 2: Add mod mc_instance to main.rs**

After `mod mc_paths;` line, add:
```rust
mod mc_instance;
```

- [ ] **Step 3: Add instances_file() to McPaths**

In `src/mc_paths.rs`, after the `instance_dir()` method and before `natives_dir()`, insert:
```rust
pub fn instances_file(&self) -> PathBuf {
    self.base.join("instances.toml")
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/main.rs src/mc_paths.rs
git commit -m "feat: add toml/uuid deps and mc_instance module scaffold"
```

---

### Task 2: Data Layer — src/mc_instance.rs (TDD)

**Files:**
- Create: `src/mc_instance.rs`

- [ ] **Step 1: Create mc_instance.rs with full implementation and tests**

Create `src/mc_instance.rs`:
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

- [ ] **Step 2: Run tests**

Run: `cargo test mc_instance`
Expected: 4 tests pass

- [ ] **Step 3: Commit**

```bash
git add src/mc_instance.rs
git commit -m "feat: add InstanceConfig and InstanceStore with TOML persistence"
```

---

### Task 3: Slint UI Layer

**Files:**
- Modify: `ui/global.slint`
- Create: `ui/components/pages/create-instance-dialog.slint`
- Modify: `ui/components/index.slint`
- Modify: `ui/main.slint`

- [ ] **Step 1: Add InstanceCreateLogic to ui/global.slint**

Append at the end of `ui/global.slint` (after the `PageAccountLogic` global):
```slint
export global InstanceCreateLogic {
    // Dialog control
    in-out property <bool>     show-dialog: false;
    in-out property <int>      active-tab: 0;

    // Basic Tab
    in-out property <string>   name: "";
    in-out property <[string]> version-list: [];
    in-out property <string>   selected-version: "";
    in-out property <string>   mod-loader: "None";

    // Advanced Tab
    in-out property <string>   xmx: "2G";
    in-out property <string>   xms: "512M";
    in-out property <bool>     logs-enabled: true;

    // Resources Tab
    in-out property <string>   world-path: "";
    in-out property <string>   resource-pack: "";
    in-out property <string>   shader-pack: "";

    // State
    in-out property <string>   error-msg: "";
    in-out property <bool>     is-loading: false;

    callback confirm-create();
    callback cancel-create();
}
```

- [ ] **Step 2: Create create-instance-dialog.slint**

Create `ui/components/pages/create-instance-dialog.slint`:
```slint
import { AppTheme } from "@theme";
import { InstanceCreateLogic } from "@global";
import { ComboBox } from "std-widgets.slint";

export component CreateInstanceDialog inherits Rectangle {
    background: #000000.with-alpha(0.6);

    Rectangle {
        width: 460px;
        height: 520px;
        x: (parent.width - self.width) / 2;
        y: (parent.height - self.height) / 2;
        background: #1e2028;
        border-radius: 12px;
        border-width: 1px;
        border-color: #ffffff.with-alpha(0.1);

        VerticalLayout {
            // ── Header ──────────────────────────────────────────────
            Rectangle {
                height: 52px;
                HorizontalLayout {
                    padding-left: 20px;
                    padding-right: 12px;
                    alignment: space-between;
                    Text {
                        text: @tr("New Minecraft Instance");
                        font-size: 15px;
                        font-weight: 600;
                        color: #e0e0e0;
                        vertical-alignment: center;
                    }
                    Rectangle {
                        width: 28px;
                        height: 28px;
                        border-radius: 6px;
                        background: x-ta.has-hover ? #ffffff.with-alpha(0.1) : transparent;
                        x-ta := TouchArea { clicked => { InstanceCreateLogic.cancel-create(); } }
                        Text {
                            text: "✕";
                            color: #888;
                            font-size: 14px;
                            horizontal-alignment: center;
                            vertical-alignment: center;
                        }
                    }
                }
            }

            Rectangle { height: 1px; background: #ffffff.with-alpha(0.08); }

            // ── Tab bar ─────────────────────────────────────────────
            Rectangle {
                height: 44px;
                HorizontalLayout {
                    padding-left: 12px;
                    spacing: 4px;
                    alignment: start;

                    Rectangle {
                        width: 80px; height: 44px;
                        tab0-ta := TouchArea { clicked => { InstanceCreateLogic.active-tab = 0; } }
                        Text {
                            text: @tr("Basic");
                            horizontal-alignment: center; vertical-alignment: center;
                            font-size: 13px;
                            color: InstanceCreateLogic.active-tab == 0 ? #76bc51 : #888;
                            font-weight: InstanceCreateLogic.active-tab == 0 ? 700 : 400;
                        }
                        Rectangle {
                            y: parent.height - 2px; x: 8px;
                            width: parent.width - 16px; height: 2px;
                            background: InstanceCreateLogic.active-tab == 0 ? #76bc51 : transparent;
                        }
                    }
                    Rectangle {
                        width: 80px; height: 44px;
                        tab1-ta := TouchArea { clicked => { InstanceCreateLogic.active-tab = 1; } }
                        Text {
                            text: @tr("Advanced");
                            horizontal-alignment: center; vertical-alignment: center;
                            font-size: 13px;
                            color: InstanceCreateLogic.active-tab == 1 ? #76bc51 : #888;
                            font-weight: InstanceCreateLogic.active-tab == 1 ? 700 : 400;
                        }
                        Rectangle {
                            y: parent.height - 2px; x: 8px;
                            width: parent.width - 16px; height: 2px;
                            background: InstanceCreateLogic.active-tab == 1 ? #76bc51 : transparent;
                        }
                    }
                    Rectangle {
                        width: 80px; height: 44px;
                        tab2-ta := TouchArea { clicked => { InstanceCreateLogic.active-tab = 2; } }
                        Text {
                            text: @tr("Resources");
                            horizontal-alignment: center; vertical-alignment: center;
                            font-size: 13px;
                            color: InstanceCreateLogic.active-tab == 2 ? #76bc51 : #888;
                            font-weight: InstanceCreateLogic.active-tab == 2 ? 700 : 400;
                        }
                        Rectangle {
                            y: parent.height - 2px; x: 8px;
                            width: parent.width - 16px; height: 2px;
                            background: InstanceCreateLogic.active-tab == 2 ? #76bc51 : transparent;
                        }
                    }
                }
            }

            Rectangle { height: 1px; background: #ffffff.with-alpha(0.08); }

            // ── Tab content ─────────────────────────────────────────
            Rectangle {
                vertical-stretch: 1;

                // Basic
                if InstanceCreateLogic.active-tab == 0: VerticalLayout {
                    padding: 20px; spacing: 16px;

                    VerticalLayout {
                        spacing: 6px;
                        Text { text: @tr("Instance Name"); font-size: 12px; color: #9e9e9e; }
                        Rectangle {
                            height: 36px; border-radius: 6px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: #242424;
                            TextInput {
                                x: 12px; width: parent.width - 24px; height: parent.height;
                                text <=> InstanceCreateLogic.name;
                                placeholder-text: @tr("e.g. Survival 1.20");
                                color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                            }
                        }
                    }

                    VerticalLayout {
                        spacing: 6px;
                        Text { text: @tr("Minecraft Version"); font-size: 12px; color: #9e9e9e; }
                        if InstanceCreateLogic.is-loading: Rectangle {
                            height: 36px; border-radius: 6px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: #242424;
                            Text {
                                x: 12px; height: parent.height;
                                text: @tr("Loading versions...");
                                color: #888; font-size: 13px; vertical-alignment: center;
                            }
                        }
                        if !InstanceCreateLogic.is-loading: ComboBox {
                            height: 36px;
                            model: InstanceCreateLogic.version-list;
                            current-value <=> InstanceCreateLogic.selected-version;
                        }
                    }

                    VerticalLayout {
                        spacing: 8px;
                        Text { text: @tr("Mod Loader"); font-size: 12px; color: #9e9e9e; }
                        HorizontalLayout {
                            spacing: 20px; alignment: start;

                            HorizontalLayout {
                                spacing: 6px;
                                Rectangle {
                                    width: 16px; height: 16px; border-radius: 8px;
                                    border-width: 2px;
                                    border-color: InstanceCreateLogic.mod-loader == "None" ? #76bc51 : #666;
                                    background: transparent;
                                    if InstanceCreateLogic.mod-loader == "None": Rectangle {
                                        width: 8px; height: 8px; x: 2px; y: 2px;
                                        border-radius: 4px; background: #76bc51;
                                    }
                                    r0-ta := TouchArea { clicked => { InstanceCreateLogic.mod-loader = "None"; } }
                                }
                                Text { text: "None"; color: #e0e0e0; font-size: 13px; vertical-alignment: center; }
                            }

                            HorizontalLayout {
                                spacing: 6px;
                                Rectangle {
                                    width: 16px; height: 16px; border-radius: 8px;
                                    border-width: 2px;
                                    border-color: InstanceCreateLogic.mod-loader == "Fabric" ? #76bc51 : #666;
                                    background: transparent;
                                    if InstanceCreateLogic.mod-loader == "Fabric": Rectangle {
                                        width: 8px; height: 8px; x: 2px; y: 2px;
                                        border-radius: 4px; background: #76bc51;
                                    }
                                    r1-ta := TouchArea { clicked => { InstanceCreateLogic.mod-loader = "Fabric"; } }
                                }
                                Text { text: "Fabric"; color: #e0e0e0; font-size: 13px; vertical-alignment: center; }
                            }

                            HorizontalLayout {
                                spacing: 6px;
                                Rectangle {
                                    width: 16px; height: 16px; border-radius: 8px;
                                    border-width: 2px;
                                    border-color: InstanceCreateLogic.mod-loader == "Forge" ? #76bc51 : #666;
                                    background: transparent;
                                    if InstanceCreateLogic.mod-loader == "Forge": Rectangle {
                                        width: 8px; height: 8px; x: 2px; y: 2px;
                                        border-radius: 4px; background: #76bc51;
                                    }
                                    r2-ta := TouchArea { clicked => { InstanceCreateLogic.mod-loader = "Forge"; } }
                                }
                                Text { text: "Forge"; color: #e0e0e0; font-size: 13px; vertical-alignment: center; }
                            }
                        }
                    }
                }

                // Advanced
                if InstanceCreateLogic.active-tab == 1: VerticalLayout {
                    padding: 20px; spacing: 16px;

                    HorizontalLayout {
                        spacing: 12px;
                        Text {
                            text: @tr("Max Memory (Xmx)"); width: 150px;
                            font-size: 13px; color: #e0e0e0; vertical-alignment: center;
                        }
                        Rectangle {
                            horizontal-stretch: 1; height: 36px; border-radius: 6px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: #242424;
                            TextInput {
                                x: 12px; width: parent.width - 24px; height: parent.height;
                                text <=> InstanceCreateLogic.xmx;
                                placeholder-text: "2G";
                                color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                            }
                        }
                    }

                    HorizontalLayout {
                        spacing: 12px;
                        Text {
                            text: @tr("Init Memory (Xms)"); width: 150px;
                            font-size: 13px; color: #e0e0e0; vertical-alignment: center;
                        }
                        Rectangle {
                            horizontal-stretch: 1; height: 36px; border-radius: 6px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: #242424;
                            TextInput {
                                x: 12px; width: parent.width - 24px; height: parent.height;
                                text <=> InstanceCreateLogic.xms;
                                placeholder-text: "512M";
                                color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                            }
                        }
                    }

                    HorizontalLayout {
                        spacing: 12px;
                        Text {
                            text: @tr("Enable Game Logs"); width: 150px;
                            font-size: 13px; color: #e0e0e0; vertical-alignment: center;
                        }
                        Rectangle {
                            width: 42px; height: 24px; border-radius: 12px;
                            background: InstanceCreateLogic.logs-enabled ? #76bc51 : #555;
                            tog-ta := TouchArea {
                                clicked => { InstanceCreateLogic.logs-enabled = !InstanceCreateLogic.logs-enabled; }
                            }
                            Rectangle {
                                width: 20px; height: 20px; border-radius: 10px; background: white;
                                x: InstanceCreateLogic.logs-enabled ? parent.width - self.width - 2px : 2px;
                                y: 2px;
                                animate x { duration: 150ms; easing: ease-in-out; }
                            }
                        }
                    }
                }

                // Resources
                if InstanceCreateLogic.active-tab == 2: VerticalLayout {
                    padding: 20px; spacing: 16px;

                    VerticalLayout {
                        spacing: 6px;
                        Text { text: @tr("World/Save Path"); font-size: 12px; color: #9e9e9e; }
                        Rectangle {
                            height: 36px; border-radius: 6px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: #242424;
                            TextInput {
                                x: 12px; width: parent.width - 24px; height: parent.height;
                                text <=> InstanceCreateLogic.world-path;
                                placeholder-text: @tr("Full path to world (optional)");
                                color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                            }
                        }
                    }

                    VerticalLayout {
                        spacing: 6px;
                        Text { text: @tr("Resource Pack Path"); font-size: 12px; color: #9e9e9e; }
                        Rectangle {
                            height: 36px; border-radius: 6px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: #242424;
                            TextInput {
                                x: 12px; width: parent.width - 24px; height: parent.height;
                                text <=> InstanceCreateLogic.resource-pack;
                                placeholder-text: @tr("Full path to resource pack (optional)");
                                color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                            }
                        }
                    }

                    VerticalLayout {
                        spacing: 6px;
                        Text { text: @tr("Shader Pack Path"); font-size: 12px; color: #9e9e9e; }
                        Rectangle {
                            height: 36px; border-radius: 6px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: #242424;
                            TextInput {
                                x: 12px; width: parent.width - 24px; height: parent.height;
                                text <=> InstanceCreateLogic.shader-pack;
                                placeholder-text: @tr("Full path to shader pack (optional)");
                                color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                            }
                        }
                    }
                }
            }

            Rectangle { height: 1px; background: #ffffff.with-alpha(0.08); }

            // ── Footer ──────────────────────────────────────────────
            Rectangle {
                height: 64px;
                VerticalLayout {
                    padding-top: 4px;
                    padding-left: 20px;
                    padding-right: 20px;
                    spacing: 4px;
                    alignment: center;

                    if InstanceCreateLogic.error-msg != "": Text {
                        text: InstanceCreateLogic.error-msg;
                        color: #e74c3c; font-size: 12px;
                        horizontal-alignment: right; wrap: word-wrap;
                    }

                    HorizontalLayout {
                        alignment: end; spacing: 8px;

                        Rectangle {
                            width: 80px; height: 34px; border-radius: 8px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: cancel-ta.has-hover ? #ffffff.with-alpha(0.1) : transparent;
                            cancel-ta := TouchArea { clicked => { InstanceCreateLogic.cancel-create(); } }
                            Text {
                                text: @tr("Cancel"); color: #888; font-size: 13px;
                                horizontal-alignment: center; vertical-alignment: center;
                            }
                        }

                        Rectangle {
                            width: 110px; height: 34px; border-radius: 8px;
                            background: confirm-ta.has-hover ? #67ab44 : #5a9a3a;
                            confirm-ta := TouchArea { clicked => { InstanceCreateLogic.confirm-create(); } }
                            Text {
                                text: @tr("Create Instance"); color: white;
                                font-size: 13px; font-weight: 600;
                                horizontal-alignment: center; vertical-alignment: center;
                            }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 3: Export CreateInstanceDialog from index.slint**

Add to `ui/components/index.slint`:
```slint
export { CreateInstanceDialog } from "./pages/create-instance-dialog.slint";
```

- [ ] **Step 4: Update ui/main.slint**

In `ui/main.slint`:
1. Change the import from `"components/index.slint"` to include `CreateInstanceDialog`:
```slint
import { SideBar, Footer, Pages, DownloadDialog, LogDialog, CreateInstanceDialog } from "components/index.slint";
```
2. Add `InstanceCreateLogic` to the global import:
```slint
import { AppData, InstanceLogic, AppLogic, PageAccountLogic, ModLogic, ModData, InstanceCreateLogic } from "@global";
```
3. Add `InstanceCreateLogic` to the export line:
```slint
export { AppData, InstanceLogic, AppLogic, PageAccountLogic, ModLogic, ModData, InstanceCreateLogic }
```
4. Add the dialog at the end of `MainApp`, after the last `if` block:
```slint
if InstanceCreateLogic.show-dialog: CreateInstanceDialog {
    x: 0;
    y: 0;
    width: root.width;
    height: root.height;
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check`
Expected: no errors

- [ ] **Step 6: Commit**

```bash
git add ui/global.slint ui/components/pages/create-instance-dialog.slint ui/components/index.slint ui/main.slint
git commit -m "feat: add InstanceCreateLogic global and 3-tab CreateInstanceDialog UI"
```

---

### Task 4: View Layer — src/view.rs Refactor

**Files:**
- Modify: `src/view.rs`

This task rewrites `open_view()` to use TOML-backed instances and wires all InstanceCreateLogic callbacks.

- [ ] **Step 1: Add mc_instance imports to view.rs**

In `src/view.rs`, change line 5:
```rust
// Before:
use crate::{mc_install, mc_parser::LaunchContext, mc_paths::McPaths, mc_token, mc_types::McSpecificVersionDetail};

// After:
use crate::{mc_install, mc_instance::{InstanceConfig, InstanceStore}, mc_parser::LaunchContext, mc_paths::McPaths, mc_token, mc_types::McSpecificVersionDetail};
```

- [ ] **Step 2: Add config_to_ui_data helper before open_view()**

Insert before `pub async fn open_view()`:
```rust
fn config_to_ui_data(config: &InstanceConfig) -> InstanceData {
    let play_time = if config.play_time_secs == 0 {
        String::new()
    } else {
        let h = config.play_time_secs / 3600;
        let m = (config.play_time_secs % 3600) / 60;
        format!("{}h {}m", h, m)
    };
    let icon_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/assets/icons/voxelruler.png");
    let image = slint::Image::load_from_path(&icon_path).unwrap_or_default();
    InstanceData {
        id: config.id.as_str().into(),
        name: config.name.as_str().into(),
        version: config.version.as_str().into(),
        mod_loader: config.mod_loader.as_str().into(),
        last_played: config.last_played.as_str().into(),
        play_time: play_time.into(),
        image,
        status: "ready".into(),
    }
}
```

- [ ] **Step 3: Replace hardcoded instances with TOML-backed store**

Replace lines 8–105 in `open_view()` (from `let ui = MainApp::new()?;` through `logic.on_search_changed(...)`) with:
```rust
let ui = MainApp::new()?;
let logic = ui.global::<InstanceLogic>();

// ── Load instances from TOML ──────────────────────────────────────
let store = Arc::new(Mutex::new(InstanceStore::new(
    McPaths::new()?.instances_file(),
)));
let master_configs: Arc<Mutex<Vec<InstanceConfig>>> = {
    let loaded = store.lock().unwrap().load().unwrap_or_default();
    Arc::new(Mutex::new(loaded))
};
{
    let configs = master_configs.lock().unwrap();
    let ui_items: Vec<InstanceData> = configs.iter().map(config_to_ui_data).collect();
    logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(ui_items))));
}

// ── Background: fetch MC version list ────────────────────────────
let ui_weak_for_versions = ui.as_weak();
{
    if let Some(ui) = ui_weak_for_versions.upgrade() {
        ui.global::<InstanceCreateLogic>().set_is_loading(true);
    }
}
tokio::spawn(async move {
    let api = crate::mc_api::McAction::new();
    match api.get_all_mc_versions().await {
        Ok(versions) => {
            let list: Vec<String> = versions.iter().map(|v| v.id.clone()).collect();
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_for_versions.upgrade() {
                    let create = ui.global::<InstanceCreateLogic>();
                    let shared: Vec<slint::SharedString> =
                        list.iter().map(|s| s.as_str().into()).collect();
                    let first = shared.first().cloned().unwrap_or_default();
                    create.set_version_list(ModelRc::from(Rc::new(VecModel::from(shared))));
                    create.set_selected_version(first);
                    create.set_is_loading(false);
                }
            })
            .ok();
        }
        Err(e) => {
            eprintln!("Failed to fetch MC versions: {e}");
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_for_versions.upgrade() {
                    ui.global::<InstanceCreateLogic>().set_is_loading(false);
                }
            })
            .ok();
        }
    }
});

// ── Search: filter from master store ─────────────────────────────
let master_for_search = Arc::clone(&master_configs);
let ui_weak_for_search = ui.as_weak();
logic.on_search_changed(move |text| {
    let Some(ui) = ui_weak_for_search.upgrade() else { return };
    let logic = ui.global::<InstanceLogic>();
    let configs = master_for_search.lock().unwrap();
    let filtered: Vec<InstanceData> = configs
        .iter()
        .filter(|c| text.is_empty() || c.name.to_lowercase().contains(&text.to_lowercase()))
        .map(config_to_ui_data)
        .collect();
    logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(filtered))));
});
```

- [ ] **Step 4: Update on_launch_instance to use master_configs**

Replace lines 106–162 (running_procs setup + on_launch_instance) with:
```rust
let running_procs: Arc<Mutex<HashMap<String, Child>>> = Arc::new(Mutex::new(HashMap::new()));
let instance_logs: Arc<Mutex<HashMap<String, VecDeque<String>>>> =
    Arc::new(Mutex::new(HashMap::new()));

let master_for_launch = Arc::clone(&master_configs);
let running_procs_for_launch = Arc::clone(&running_procs);
let instance_logs_for_launch = Arc::clone(&instance_logs);
let ui_weak_for_launch = ui.as_weak();
logic.on_launch_instance(move |id| {
    let (version_id, instance_id, instance_name, xmx, xms) = {
        let configs = master_for_launch.lock().unwrap();
        if let Some(c) = configs.iter().find(|c| c.id == id.as_str()) {
            (c.version.clone(), c.id.clone(), c.name.clone(), c.xmx.clone(), c.xms.clone())
        } else {
            (id.to_string(), id.to_string(), id.to_string(), "2G".into(), "512M".into())
        }
    };
    let running_procs = Arc::clone(&running_procs_for_launch);
    let ui_weak = ui_weak_for_launch.clone();
    let logs = Arc::clone(&instance_logs_for_launch);
    if running_procs.lock().unwrap().contains_key(&instance_id) {
        return;
    }
    tokio::spawn(async move {
        match do_launch(version_id, instance_id.clone(), instance_name.clone(), xmx, xms, ui_weak.clone(), logs).await {
            Ok(child) => {
                running_procs.lock().unwrap().insert(instance_id.clone(), child);
                set_instance_status(&ui_weak, &instance_id, "running");
                let running_procs_watch = Arc::clone(&running_procs);
                let ui_weak_watch = ui_weak.clone();
                let id_watch = instance_id.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        let mut map = running_procs_watch.lock().unwrap();
                        let Some(child) = map.get_mut(&id_watch) else { break };
                        match child.try_wait() {
                            Ok(Some(_)) | Err(_) => {
                                map.remove(&id_watch);
                                drop(map);
                                set_instance_status(&ui_weak_watch, &id_watch, "ready");
                                break;
                            }
                            Ok(None) => {}
                        }
                    }
                });
            }
            Err(e) => {
                eprintln!("啟動失敗：{e:#}");
                set_install_state(&ui_weak, true, 0.0, &format!("啟動失敗：{e:#}"), true);
            }
        }
    });
});
```

- [ ] **Step 5: Replace on_new_instance stub and add InstanceCreateLogic callbacks**

Replace lines 210–213 (the `logic.on_new_instance` stub) with:
```rust
let ui_weak_for_new = ui.as_weak();
logic.on_new_instance(move || {
    let Some(ui) = ui_weak_for_new.upgrade() else { return };
    let create = ui.global::<InstanceCreateLogic>();
    create.set_name("".into());
    create.set_mod_loader("None".into());
    create.set_xmx("2G".into());
    create.set_xms("512M".into());
    create.set_logs_enabled(true);
    create.set_world_path("".into());
    create.set_resource_pack("".into());
    create.set_shader_pack("".into());
    create.set_error_msg("".into());
    create.set_active_tab(0);
    create.set_show_dialog(true);
});

let create_logic = ui.global::<InstanceCreateLogic>();

let ui_weak_for_cancel = ui.as_weak();
create_logic.on_cancel_create(move || {
    if let Some(ui) = ui_weak_for_cancel.upgrade() {
        ui.global::<InstanceCreateLogic>().set_show_dialog(false);
    }
});

let store_for_create = Arc::clone(&store);
let master_for_create = Arc::clone(&master_configs);
let ui_weak_for_confirm = ui.as_weak();
create_logic.on_confirm_create(move || {
    let Some(ui) = ui_weak_for_confirm.upgrade() else { return };
    let create = ui.global::<InstanceCreateLogic>();

    let name = create.get_name().to_string();
    let version = create.get_selected_version().to_string();

    if name.trim().is_empty() {
        create.set_error_msg("實例名稱不可為空".into());
        return;
    }
    if version.is_empty() {
        create.set_error_msg("請選擇 Minecraft 版本".into());
        return;
    }

    let config = InstanceConfig {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.trim().to_string(),
        version,
        mod_loader: create.get_mod_loader().to_string(),
        xmx: create.get_xmx().to_string(),
        xms: create.get_xms().to_string(),
        logs_enabled: create.get_logs_enabled(),
        world_path: create.get_world_path().to_string(),
        resource_pack: create.get_resource_pack().to_string(),
        shader_pack: create.get_shader_pack().to_string(),
        ..Default::default()
    };

    match store_for_create.lock().unwrap().append(config) {
        Ok(updated) => {
            *master_for_create.lock().unwrap() = updated.clone();
            let logic = ui.global::<InstanceLogic>();
            let new_items: Vec<InstanceData> = updated.iter().map(config_to_ui_data).collect();
            logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(new_items))));
            create.set_show_dialog(false);
        }
        Err(e) => {
            create.set_error_msg(format!("建立失敗：{e}").into());
        }
    }
});
```

- [ ] **Step 6: Update do_launch signature to accept xmx/xms**

Change `do_launch` function signature from:
```rust
async fn do_launch(
    version_id: String,
    instance_id: String,
    instance_name: String,
    ui_weak: slint::Weak<MainApp>,
    instance_logs: Arc<Mutex<HashMap<String, VecDeque<String>>>>,
) -> anyhow::Result<Child> {
```
to:
```rust
async fn do_launch(
    version_id: String,
    instance_id: String,
    instance_name: String,
    xmx: String,
    xms: String,
    ui_weak: slint::Weak<MainApp>,
    instance_logs: Arc<Mutex<HashMap<String, VecDeque<String>>>>,
) -> anyhow::Result<Child> {
```

And change the `LaunchContext` construction (replacing the hardcoded `xmx`/`xms`):
```rust
// Before:
xmx: "2G".into(),
xms: "512M".into(),

// After:
xmx,
xms,
```

- [ ] **Step 7: Build and verify**

Run: `cargo build`
Expected: Successful build, 0 errors

- [ ] **Step 8: Commit**

```bash
git add src/view.rs
git commit -m "feat: wire InstanceCreateLogic callbacks and replace hardcoded instances with TOML store"
```

---

*Plan generated 2026-05-27 via writing-plans skill*
