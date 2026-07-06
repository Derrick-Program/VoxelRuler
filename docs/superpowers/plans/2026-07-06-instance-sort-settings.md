# Instance Sort Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Settings-page control (sort mode + ascending/descending) that changes the instance card order on the Instances page, applied immediately and persisted to `settings.toml`.

**Architecture:** `InstanceStore` (in `src/mc_instance.rs`) gains an internal `sort_mode`/`sort_ascending` state and a `set_sort()` setter; `load()` uses that state instead of a hardcoded comparator. Because `InstanceStore::load()` is already the single point every code path (initial boot, new-instance creation, file-watch refresh) funnels through to rebuild `master_configs`, setting the sort state once via `set_sort()` is enough — no other call site needs to change to pick up the new order. The Settings page UI writes the choice to `AppSettings` (`src/settings.rs`) and calls `store.set_sort(...)` + reloads.

**Tech Stack:** Rust, Slint UI (compiled via `build.rs`), `serde`/`toml` for persistence, existing `tempfile`-based test harness.

## Global Constraints

- No new crate dependencies — the version comparator must be written by hand (no `itertools`, no semver crate).
- Comment policy from `CLAUDE.md`: no comments except where a hidden constraint/non-obvious invariant would otherwise confuse a future reader.
- `#[serde(default)]` required on any new `AppSettings` field so existing `settings.toml` files without it still load.
- Sort must apply immediately on change (no Save button for this control) and persist to `settings.toml`.
- Never-played instances (`last_played == ""`) always sort last, regardless of ascending/descending.
- Version comparator: split on `.`, compare each segment numerically when both sides parse as `u64`, otherwise fall back to string comparison for that segment — no special-casing of snapshot/beta/alpha formats.

---

### Task 1: `SortMode` enum + `AppSettings` fields

**Files:**
- Modify: `src/settings.rs`
- Test: `src/settings.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Produces: `pub enum SortMode { Name, Version, CreatedAt, LastPlayed }` (in `src/settings.rs`, `Debug + Clone + Copy + PartialEq + Eq + Serialize + Deserialize`, `Default` impl returns `SortMode::CreatedAt`), and `AppSettings.sort_mode: SortMode` / `AppSettings.sort_ascending: bool` fields.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module at the bottom of `src/settings.rs` (after `test_settings_file_io`, before the closing `}` of `mod tests`):

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib settings::tests -- --nocapture`
Expected: compile error — `SortMode` and the `sort_mode`/`sort_ascending` fields don't exist yet.

- [ ] **Step 3: Implement `SortMode` and extend `AppSettings`**

At the top of `src/settings.rs`, right after the `use` block (before `pub struct AppSettings`), add:

```rust
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
```

Then change the `AppSettings` struct definition from:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub java_mode: String,
    pub java_path: String,
}
```

to:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub java_mode: String,
    pub java_path: String,
    pub sort_mode: SortMode,
    pub sort_ascending: bool,
}
```

(The struct-level `#[serde(default)]` already covers every field, so no extra per-field attribute is needed.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib settings::tests -- --nocapture`
Expected: PASS (6 tests: the 3 pre-existing plus the 3 new ones)

- [ ] **Step 5: Commit**

```bash
git add src/settings.rs
git commit -m "$(cat <<'EOF'
feat(settings): add SortMode enum and persisted sort preference

EOF
)"
```

---

### Task 2: Sort algorithm in `InstanceStore`

**Files:**
- Modify: `src/mc_instance.rs`
- Test: `src/mc_instance.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `crate::settings::SortMode` (from Task 1).
- Produces: `InstanceStore::set_sort(&mut self, mode: SortMode, ascending: bool)`; `InstanceStore::load()` keeps its existing signature (`&self -> anyhow::Result<Vec<InstanceConfig>>`) but now sorts according to whatever `set_sort` last configured.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module at the bottom of `src/mc_instance.rs` (after `test_instance_store_delete_one`, before the closing `}` of `mod tests`):

```rust
    #[test]
    fn test_compare_versions_numeric_segments() {
        assert_eq!(compare_versions("1.7.2", "1.21.7"), std::cmp::Ordering::Less);
        assert_eq!(compare_versions("1.7.10", "1.7.2"), std::cmp::Ordering::Greater);
        assert_eq!(compare_versions("1.20.4", "1.20.4"), std::cmp::Ordering::Equal);
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
            .save_one(&InstanceConfig { id: "b".into(), name: "Banana".into(), ..Default::default() })
            .unwrap();
        store
            .save_one(&InstanceConfig { id: "a".into(), name: "Apple".into(), ..Default::default() })
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
            .save_one(&InstanceConfig { id: "old".into(), version: "1.7.2".into(), ..Default::default() })
            .unwrap();
        store
            .save_one(&InstanceConfig { id: "new".into(), version: "1.21.7".into(), ..Default::default() })
            .unwrap();

        store.set_sort(SortMode::Version, false);
        let loaded = store.load().unwrap();
        assert_eq!(loaded[0].id, "new", "1.21.7 should sort above 1.7.2 descending");
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
            .save_one(&InstanceConfig { id: "never".into(), last_played: "".into(), ..Default::default() })
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
        let old = InstanceConfig { id: "old".into(), created_at: 100, ..Default::default() };
        let new = InstanceConfig { id: "new".into(), created_at: 200, ..Default::default() };
        store.save_one(&old).unwrap();
        store.save_one(&new).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded[0].id, "new");
        assert_eq!(loaded[1].id, "old");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib mc_instance::tests -- --nocapture`
Expected: compile errors — `compare_versions` and `InstanceStore::set_sort` don't exist yet.

- [ ] **Step 3: Implement the sort state and algorithm**

Add the import at the top of `src/mc_instance.rs` (after the existing `use` block):

```rust
use crate::settings::SortMode;
```

Change the `InstanceStore` struct and its `new` constructor from:

```rust
pub struct InstanceStore {
    base_dir: PathBuf,
}

impl InstanceStore {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }
```

to:

```rust
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
```

Replace the final two lines of `load()`:

```rust
        instances.sort_by(|a, b| b.1.cmp(&a.1));
        Ok(instances.into_iter().map(|(c, _)| c).collect())
```

with:

```rust
        sort_instances(&mut instances, self.sort_mode, self.sort_ascending);
        Ok(instances.into_iter().map(|(c, _)| c).collect())
```

Then add these two free functions just above `impl InstanceStore` (i.e. right after the `InstanceConfig` `Default` impl, before `pub struct InstanceStore`):

```rust
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
```

Finally, in the test module, add `use super::*;` already covers `compare_versions`/`sort_instances`/`SortMode` (re-exported via the `use crate::settings::SortMode;` above), but `tmp_store()` currently returns `(InstanceStore, tempfile::TempDir)` — no change needed there since `InstanceStore` already derives no `Copy`, and the new tests reassign `store` as `mut` at the binding site (`let mut store = store;` / `let (mut store, _dir) = tmp_store();`) rather than changing `tmp_store()`'s signature.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib mc_instance::tests -- --nocapture`
Expected: PASS (all previous tests plus the 5 new ones — 11 total)

- [ ] **Step 5: Commit**

```bash
git add src/mc_instance.rs
git commit -m "$(cat <<'EOF'
feat(mc_instance): make InstanceStore.load() sort configurable

InstanceStore now holds an explicit sort_mode/sort_ascending state set
via set_sort(), instead of hardcoding newest-created-first. Defaults
match the previous behavior so all existing callers are unaffected
until set_sort() is wired up to the Settings UI.
EOF
)"
```

---

### Task 3: Preserve sort settings when saving Java settings

**Why this is its own task:** `src/view/launch.rs`'s `on_save_settings` handler currently builds `AppSettings { java_mode, java_path }` as a struct literal. Once `AppSettings` gains `sort_mode`/`sort_ascending` (Task 1), this literal would silently reset those two fields to their defaults every time the user clicks "Save" on the Java settings card — clobbering whatever sort preference was set from the new Instance Sorting card. This must be fixed before Task 5 wires up the sort UI, otherwise the two settings cards will fight each other.

**Files:**
- Modify: `src/view/launch.rs:773-776`

**Interfaces:**
- Consumes: `AppSettings::load()` (existing, `src/settings.rs`).

- [ ] **Step 1: Confirm the current clobbering behavior compiles today**

Run: `cargo check 2>&1 | grep -A3 "missing field"`
Expected: no output yet (Task 1 already added `#[serde(default)]`-backed fields with a `Default` derive on the struct, so the struct literal in `launch.rs` still compiles today — it just silently drops the two new fields to their defaults on every save). This step is a sanity check, not a red test; there's no compiler error to see because the bug is a silent behavioral one, not a type error.

- [ ] **Step 2: Fix the struct literal to preserve existing fields**

In `src/view/launch.rs`, change:

```rust
        let new_settings = AppSettings {
            java_mode: java_mode.to_string(),
            java_path,
        };
        match new_settings.save() {
```

to:

```rust
        let mut new_settings = AppSettings::load();
        new_settings.java_mode = java_mode.to_string();
        new_settings.java_path = java_path;
        match new_settings.save() {
```

- [ ] **Step 3: Verify it builds**

Run: `cargo check`
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add src/view/launch.rs
git commit -m "$(cat <<'EOF'
fix(settings): preserve sort preference when saving Java settings

on_save_settings previously constructed AppSettings from scratch,
which would reset sort_mode/sort_ascending to their defaults on every
Java-settings save once those fields exist.
EOF
)"
```

---

### Task 4: Slint UI — sort controls on the Settings page

**Files:**
- Modify: `ui/global.slint:442-451` (`SettingsLogic` global)
- Modify: `ui/components/pages/settings.slint`

**Interfaces:**
- Produces (new `SettingsLogic` properties/callback, consumed by Task 5's Rust wiring): `sort-mode-list: [string]`, `selected-sort-mode: string`, `sort-ascending: bool`, `callback sort-changed()`.

- [ ] **Step 1: Add the new properties and callback to `SettingsLogic`**

In `ui/global.slint`, change:

```slint
export global SettingsLogic {
    in-out property <[string]> java-mode-list: [];
    in-out property <string>   selected-java-mode: "";
    in-out property <[string]> detected-java-list: [];
    in-out property <string>   java-path: "";
    in-out property <string>   status-msg: "";

    callback save-settings();
    callback browse-java();
}
```

to:

```slint
export global SettingsLogic {
    in-out property <[string]> java-mode-list: [];
    in-out property <string>   selected-java-mode: "";
    in-out property <[string]> detected-java-list: [];
    in-out property <string>   java-path: "";
    in-out property <string>   status-msg: "";
    in-out property <[string]> sort-mode-list: [];
    in-out property <string>   selected-sort-mode: "";
    in-out property <bool>     sort-ascending: false;

    callback save-settings();
    callback browse-java();
    callback sort-changed();
}
```

- [ ] **Step 2: Add the "Instance Sorting" card to the Settings page**

In `ui/components/pages/settings.slint`, insert a new `Rectangle` card between the existing Java card's closing brace and the outer `VerticalLayout`'s closing brace (i.e. right after line 144 `}` which closes the Java `Rectangle`, before line 145 `}` which closes the outer `VerticalLayout`):

```slint
        Rectangle {
            border-radius: 12px;
            border-width: 1px;
            border-color: #ffffff.with-alpha(0.1);
            background: AppTheme.colors.background.brighter(8%);

            VerticalLayout {
                padding: 20px;
                spacing: 16px;
                alignment: start;

                Text {
                    text: @tr("Instance Sorting");
                    font-size: 15px;
                    font-weight: 700;
                    color: AppTheme.colors.foreground;
                }

                Text {
                    text: @tr("Controls the card order on the Instances page. Applies immediately.");
                    font-size: 12px;
                    color: #888;
                    wrap: word-wrap;
                }

                VerticalLayout {
                    spacing: 6px;
                    Text { text: @tr("Sort by"); font-size: 12px; color: #9e9e9e; }
                    HorizontalLayout {
                        spacing: 8px;
                        ComboBox {
                            height: 36px;
                            model: SettingsLogic.sort-mode-list;
                            current-value <=> SettingsLogic.selected-sort-mode;
                            selected(value) => { SettingsLogic.sort-changed(); }
                        }
                        Rectangle {
                            width: 130px; height: 36px; border-radius: 8px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.2);
                            background: dir-ta.has-hover ? #ffffff.with-alpha(0.1) : #2a2a2a;
                            dir-ta := TouchArea {
                                clicked => {
                                    SettingsLogic.sort-ascending = !SettingsLogic.sort-ascending;
                                    SettingsLogic.sort-changed();
                                }
                            }
                            Text {
                                text: SettingsLogic.sort-ascending ? @tr("↑ Ascending") : @tr("↓ Descending");
                                color: #e0e0e0; font-size: 12px;
                                horizontal-alignment: center; vertical-alignment: center;
                            }
                        }
                    }
                }
            }
        }
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: no errors (this exercises `build.rs`'s Slint compilation step; a Slint syntax error surfaces here as a `build.rs` failure, not a normal Rust error).

- [ ] **Step 4: Commit**

```bash
git add ui/global.slint ui/components/pages/settings.slint
git commit -m "$(cat <<'EOF'
feat(ui): add Instance Sorting card to Settings page

ComboBox for sort mode plus an ascending/descending toggle button,
both firing SettingsLogic.sort-changed() immediately on interaction.
EOF
)"
```

---

### Task 5: Rust wiring — apply and persist the sort setting

**Files:**
- Modify: `src/view/mod.rs`

**Interfaces:**
- Consumes: `InstanceStore::set_sort` (Task 2), `SettingsLogic` properties/callback (Task 4), `AppSettings::load()`/`.save()` (Task 1/existing).
- Produces: `sort_mode_to_label(mode: SortMode) -> &'static str`, `label_to_sort_mode(label: &str) -> SortMode`, `fn refresh_instance_list(logic: &InstanceLogic, configs: &[InstanceConfig], running_ids: &std::collections::HashSet<String>)` — all private to `view/mod.rs`, used only within this task's own changes.

- [ ] **Step 1: Add the label-mapping helpers**

In `src/view/mod.rs`, right after the existing `java_label_to_mode` function (after line 59, before the `JavaSource` enum), add:

```rust
const SORT_LABEL_NAME: &str = "Name (A-Z)";
const SORT_LABEL_VERSION: &str = "Version";
const SORT_LABEL_CREATED_AT: &str = "Created Time";
const SORT_LABEL_LAST_PLAYED: &str = "Last Played";

fn sort_mode_to_label(mode: crate::settings::SortMode) -> &'static str {
    use crate::settings::SortMode;
    match mode {
        SortMode::Name => SORT_LABEL_NAME,
        SortMode::Version => SORT_LABEL_VERSION,
        SortMode::CreatedAt => SORT_LABEL_CREATED_AT,
        SortMode::LastPlayed => SORT_LABEL_LAST_PLAYED,
    }
}

fn label_to_sort_mode(label: &str) -> crate::settings::SortMode {
    use crate::settings::SortMode;
    match label {
        SORT_LABEL_NAME => SortMode::Name,
        SORT_LABEL_VERSION => SortMode::Version,
        SORT_LABEL_LAST_PLAYED => SortMode::LastPlayed,
        _ => SortMode::CreatedAt,
    }
}
```

- [ ] **Step 2: Add the shared `refresh_instance_list` helper**

Right after the existing `config_to_ui_data` function (after line 101, before `pub async fn open_view`), add:

```rust
fn refresh_instance_list(
    logic: &InstanceLogic,
    configs: &[InstanceConfig],
    running_ids: &std::collections::HashSet<String>,
) {
    let search_text = logic.get_search_text().to_string().to_lowercase();
    let mut ui_items: Vec<InstanceData> = configs
        .iter()
        .filter(|c| search_text.is_empty() || c.name.to_lowercase().contains(&search_text))
        .map(|c| {
            let mut item = config_to_ui_data(c);
            if running_ids.contains(&c.id) {
                item.status = "running".into();
            }
            item
        })
        .collect();
    if ui_items.is_empty() {
        ui_items.push(InstanceData {
            id: "".into(),
            name: "".into(),
            version: "".into(),
            mod_loader: "".into(),
            last_played: "".into(),
            play_time: "".into(),
            image: Default::default(),
            status: "".into(),
        });
    }
    logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(ui_items))));
}
```

- [ ] **Step 3: Use the helper in the existing file-watch refresh path**

In `src/view/mod.rs`, inside the `tokio::spawn` file-watch loop, replace this block (currently lines 149-177):

```rust
                    let logic = ui_handle.global::<InstanceLogic>();
                    let search_text = logic.get_search_text().to_string().to_lowercase();
                    let mut ui_items: Vec<InstanceData> = latest_configs
                        .iter()
                        .filter(|c| {
                            search_text.is_empty() || c.name.to_lowercase().contains(&search_text)
                        })
                        .map(|c| {
                            let mut item = config_to_ui_data(c);
                            if running_ids.contains(&c.id) {
                                item.status = "running".into();
                            }
                            item
                        })
                        .collect();
                    if ui_items.is_empty() {
                        ui_items.push(InstanceData {
                            id: "".into(),
                            name: "".into(),
                            version: "".into(),
                            mod_loader: "".into(),
                            last_played: "".into(),
                            play_time: "".into(),
                            image: Default::default(),
                            status: "".into(),
                        });
                    }

                    logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(ui_items))));
                    info!("UI list securely synced with disk");
```

with:

```rust
                    let logic = ui_handle.global::<InstanceLogic>();
                    refresh_instance_list(&logic, &latest_configs, &running_ids);
                    info!("UI list securely synced with disk");
```

(`running_ids` at this point in the existing code is already a `std::collections::HashSet<String>` built a few lines above — no change needed there.)

- [ ] **Step 4: Apply the persisted sort before the very first render**

In `src/view/mod.rs`, change the startup block (currently lines 108-114):

```rust
    let store = Arc::new(Mutex::new(InstanceStore::new(
        McPaths::new()?.instances_base_dir(),
    )));
    let master_configs: Arc<Mutex<Vec<InstanceConfig>>> = {
        let loaded = store.lock().unwrap().load().unwrap_or_default();
        Arc::new(Mutex::new(loaded))
    };
```

to:

```rust
    let store = Arc::new(Mutex::new(InstanceStore::new(
        McPaths::new()?.instances_base_dir(),
    )));
    {
        let boot_settings = AppSettings::load();
        store
            .lock()
            .unwrap()
            .set_sort(boot_settings.sort_mode, boot_settings.sort_ascending);
    }
    let master_configs: Arc<Mutex<Vec<InstanceConfig>>> = {
        let loaded = store.lock().unwrap().load().unwrap_or_default();
        Arc::new(Mutex::new(loaded))
    };
```

- [ ] **Step 5: Initialize the `SettingsLogic` sort properties at startup**

In `src/view/mod.rs`, change (currently lines 358-366):

```rust
        let settings_items: Vec<slint::SharedString> = vec![
            JAVA_MODE_LABEL_MINECRAFT.into(),
            JAVA_MODE_LABEL_CUSTOM.into(),
        ];
        let app_settings = AppSettings::load();
        let sl = ui.global::<SettingsLogic>();
        sl.set_java_mode_list(ModelRc::from(Rc::new(VecModel::from(settings_items))));
        sl.set_selected_java_mode(java_mode_to_label(&app_settings.java_mode, false).into());
        sl.set_java_path(app_settings.java_path.as_str().into());
    }
```

to:

```rust
        let settings_items: Vec<slint::SharedString> = vec![
            JAVA_MODE_LABEL_MINECRAFT.into(),
            JAVA_MODE_LABEL_CUSTOM.into(),
        ];
        let app_settings = AppSettings::load();
        let sl = ui.global::<SettingsLogic>();
        sl.set_java_mode_list(ModelRc::from(Rc::new(VecModel::from(settings_items))));
        sl.set_selected_java_mode(java_mode_to_label(&app_settings.java_mode, false).into());
        sl.set_java_path(app_settings.java_path.as_str().into());

        let sort_items: Vec<slint::SharedString> = vec![
            SORT_LABEL_NAME.into(),
            SORT_LABEL_VERSION.into(),
            SORT_LABEL_CREATED_AT.into(),
            SORT_LABEL_LAST_PLAYED.into(),
        ];
        sl.set_sort_mode_list(ModelRc::from(Rc::new(VecModel::from(sort_items))));
        sl.set_selected_sort_mode(sort_mode_to_label(app_settings.sort_mode).into());
        sl.set_sort_ascending(app_settings.sort_ascending);
    }
```

- [ ] **Step 6: Wire the `on_sort_changed` callback**

Immediately after the block from Step 5 (still inside the same enclosing scope, right before `let ui_weak_for_scan = ui.as_weak();`), add:

```rust
    let store_for_sort = Arc::clone(&store);
    let master_for_sort = Arc::clone(&master_configs);
    let running_for_sort = Arc::clone(&running_procs);
    let ui_weak_for_sort = ui.as_weak();
    ui.global::<SettingsLogic>().on_sort_changed(move || {
        let Some(ui) = ui_weak_for_sort.upgrade() else {
            return;
        };
        let sl = ui.global::<SettingsLogic>();
        let mode = label_to_sort_mode(sl.get_selected_sort_mode().as_str());
        let ascending = sl.get_sort_ascending();

        let mut settings = AppSettings::load();
        settings.sort_mode = mode;
        settings.sort_ascending = ascending;
        if let Err(e) = settings.save() {
            warn!(error = %e, "Failed to save sort settings");
        }

        let reloaded = {
            let mut store = store_for_sort.lock().unwrap();
            store.set_sort(mode, ascending);
            store.load().unwrap_or_default()
        };
        *master_for_sort.lock().unwrap() = reloaded.clone();

        let logic = ui.global::<InstanceLogic>();
        let running_ids: std::collections::HashSet<String> =
            running_for_sort.lock().unwrap().keys().cloned().collect();
        refresh_instance_list(&logic, &reloaded, &running_ids);
    });
```

**Note on placement:** this must be inside `open_view()` in the same scope where `store`, `master_configs`, and `running_procs` are still owned as `Arc`s (i.e. before they get moved/cloned into `launch::setup_launch_logic(...)` later in the function) — that's already true of the block from Step 5, so adding this immediately after it works without reordering anything else.

- [ ] **Step 7: Run the full test suite**

Run: `cargo test`
Expected: PASS — all `mc_instance` and `settings` tests from Tasks 1-2 still pass, plus whatever pre-existing tests exist in `view/mod.rs`, `view/create.rs`, etc. No new unit tests are added in this task (Slint callback wiring isn't unit-tested in this codebase's existing pattern — see Step 8 for manual verification).

- [ ] **Step 8: Manual verification**

Run: `cargo run`

1. Create at least 3 instances with different names (e.g. "Zeta", "Alpha", "Mid"), different versions (e.g. `1.7.2`, `1.21.7`, `1.16.5`), at different times.
2. Go to Settings page → confirm the new "Instance Sorting" card appears below the Java card.
3. Select "Name (A-Z)", toggle ascending → Instances page cards reorder alphabetically immediately, no Save click needed.
4. Toggle to descending → order flips (Z→A).
5. Switch to "Version" → confirm `1.21.7` sorts above `1.7.2` (not below, which is the original bug being fixed).
6. Switch to "Last Played" → confirm any instance that has never been launched sinks to the bottom in both ascending and descending.
7. Quit and relaunch the app → confirm the previously chosen sort mode/direction is still selected on the Settings page and the Instances page order matches it.

- [ ] **Step 9: Commit**

```bash
git add src/view/mod.rs
git commit -m "$(cat <<'EOF'
feat(view): wire Settings-page sort control to the instance list

Persists sort_mode/sort_ascending to settings.toml on change, applies
immediately via InstanceStore::set_sort(), and restores the choice on
next launch. Extracts refresh_instance_list() so the sort-changed path
reuses the same filter+render logic as the existing file-watch refresh
instead of a third copy-pasted block.
EOF
)"
```

---

## Self-Review Notes

- **Spec coverage:** Section 1 (data/persistence) → Task 1. Section 2 (sort algorithm, including the hermeticity correction found during planning) → Task 2. Section 3 (UI) → Task 4. Section 4 (Rust wiring) → Task 5, plus Task 3 which the original spec didn't call out explicitly but is a necessary consequence of adding fields to `AppSettings` (caught by reading `launch.rs`'s existing `on_save_settings` struct literal). Section 5 (testing) → covered across Tasks 1, 2, and 5's manual verification.
- **Type consistency:** `SortMode` defined once in `settings.rs` (Task 1), consumed identically in `mc_instance.rs` (Task 2) and `view/mod.rs` (Task 5) via `crate::settings::SortMode`. `SettingsLogic` property names (`sort-mode-list`, `selected-sort-mode`, `sort-ascending`, `sort-changed`) match 1:1 between Task 4 (Slint) and Task 5 (Rust getter/setter calls `get_selected_sort_mode`/`set_selected_sort_mode`/`get_sort_ascending`/`set_sort_ascending`/`on_sort_changed`, which are the Slint-generated bindings for those exact property/callback names).
