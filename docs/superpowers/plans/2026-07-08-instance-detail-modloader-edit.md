# Instance Detail — Editable Mod Loader Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a user change an existing instance's mod loader (None/Fabric/Forge/NeoForge) and loader version from the instance detail dialog's Version tab, and persist it together with the Minecraft version via a single "Save" action.

**Architecture:** Extract the network-fetching portion of the Create dialog's loader-selection logic (`src/view/create.rs`) into a reusable `ModLoaderApi::fetch_loader_state` function in `src/mc_modloader.rs`. Reuse it from a new, parallel loader-picker flow in `src/view/instance_detail.rs`, wired to new `InstanceDetailLogic` properties in `ui/global.slint` and a ported copy of the Create dialog's loader-picker widget in `ui/components/pages/instance-detail-window.slint`.

**Tech Stack:** Rust (edition 2024), Slint UI (compiled via `build.rs`), tokio async runtime, existing `mc_modloader::ModLoaderApi` for Forge/Fabric/NeoForge metadata APIs.

## Global Constraints

- Edition: Rust 2024 (`Cargo.toml`).
- Lint gate: `cargo clippy --all-targets -- -D warnings` must pass clean (matches `.github/workflows/ci.yml:58`).
- No code comments unless documenting a non-obvious constraint/workaround (project convention, `CLAUDE.md`).
- Commit format: Conventional Commits, e.g. `feat(instance): ...`, `refactor(instance): ...`, `test(instance): ...` (`.claude/docs/git-conventions.md`).
- Every Slint property/callback referenced in Rust must exist in `ui/global.slint` before the Rust code that uses it is compiled — tasks are ordered so the build stays green after each commit.
- Existing behavior of the Create Instance dialog (`InstanceCreateLogic`) must not change — Task 2 is a pure refactor verified by existing tests.

---

### Task 1: `ModLoaderApi::fetch_loader_state` helper + tests

**Files:**
- Modify: `src/mc_modloader.rs`

**Interfaces:**
- Produces:
  - `pub struct LoaderFetchResult { pub availability: LoaderAvailability, pub versions: Vec<String>, pub error: Option<String> }`
  - `pub async fn ModLoaderApi::fetch_loader_state(mc_version: &str, loader: Option<ModLoaderType>) -> LoaderFetchResult`
    - Always calls `Self::check_availability(mc_version)`.
    - If `loader` is `Some(lt)` and `availability.supports(lt)`, also calls `Self::get_loader_versions(lt, mc_version)`; on `Ok(v)` returns `versions: v, error: None`; on `Err(e)` returns `versions: vec![], error: Some(e.to_string())`.
    - Otherwise (loader is `None`, or unavailable) returns `versions: vec![], error: None`.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block at the bottom of `src/mc_modloader.rs` (after the existing `test_get_forge_versions` test):

```rust
    #[tokio::test]
    async fn test_fetch_loader_state_returns_versions_for_available_loader() {
        let result =
            ModLoaderApi::fetch_loader_state("1.20.4", Some(ModLoaderType::Fabric)).await;
        assert!(result.availability.fabric);
        assert!(!result.versions.is_empty());
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn test_fetch_loader_state_none_loader_returns_no_versions() {
        let result = ModLoaderApi::fetch_loader_state("1.20.4", None).await;
        assert!(result.versions.is_empty());
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn test_fetch_loader_state_unsupported_mc_returns_empty_versions_no_error() {
        let result =
            ModLoaderApi::fetch_loader_state("1.12.2", Some(ModLoaderType::Fabric)).await;
        assert!(result.versions.is_empty());
        assert!(result.error.is_none());
    }
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: `cargo test --lib mc_modloader::tests::test_fetch_loader_state -- --test-threads=1`
Expected: compile error — `fetch_loader_state` and/or `LoaderFetchResult` not found.

- [ ] **Step 3: Implement `LoaderFetchResult` and `fetch_loader_state`**

In `src/mc_modloader.rs`, add the struct right after the existing `LoaderAvailability` impl block (after line 86, before `pub struct ModLoaderApi;`):

```rust
#[derive(Debug, Clone)]
pub struct LoaderFetchResult {
    pub availability: LoaderAvailability,
    pub versions: Vec<String>,
    pub error: Option<String>,
}
```

Then add the method inside `impl ModLoaderApi { ... }`, right after `get_loader_versions` (after line 100):

```rust
    pub async fn fetch_loader_state(
        mc_version: &str,
        loader: Option<ModLoaderType>,
    ) -> LoaderFetchResult {
        let availability = Self::check_availability(mc_version).await;

        let (versions, error) = match loader {
            Some(lt) if availability.supports(lt) => {
                match Self::get_loader_versions(lt, mc_version).await {
                    Ok(v) => (v, None),
                    Err(e) => (Vec::new(), Some(e.to_string())),
                }
            }
            _ => (Vec::new(), None),
        };

        LoaderFetchResult {
            availability,
            versions,
            error,
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib mc_modloader::tests::test_fetch_loader_state -- --test-threads=1`
Expected: 3 tests PASS (these hit live Fabric/Forge/NeoForge metadata APIs, same as the pre-existing `test_get_fabric_versions` etc. in this file — requires network access).

- [ ] **Step 5: Commit**

```bash
git add src/mc_modloader.rs
git commit -m "feat(instance): add ModLoaderApi::fetch_loader_state helper"
```

---

### Task 2: Refactor `create.rs` to use `fetch_loader_state`

**Files:**
- Modify: `src/view/create.rs:169-243`

**Interfaces:**
- Consumes: `ModLoaderApi::fetch_loader_state(mc_version: &str, loader: Option<ModLoaderType>) -> LoaderFetchResult` (Task 1).
- Produces: no new interface — pure refactor, `InstanceCreateLogic` behavior unchanged.

- [ ] **Step 1: Replace the inline fetch block**

In `src/view/create.rs`, inside `on_loader_changed`, replace the `tokio::spawn(async move { ... })` block (lines 172–243 in the original file — from `let ui_handle_inner = ui.as_weak();` through the closing `});` of the outer `tokio::spawn`) with:

```rust
                let ui_handle_inner = ui.as_weak();
                let gen_for_fetch = Arc::clone(&gen_for_ui);

                tokio::spawn(async move {
                    let result = crate::mc_modloader::ModLoaderApi::fetch_loader_state(
                        &mc_version,
                        loader_type,
                    )
                    .await;

                    let _ = slint::invoke_from_event_loop(move || {
                        if gen_for_fetch.load(Ordering::SeqCst) != my_gen {
                            return;
                        }
                        let Some(ui) = ui_handle_inner.upgrade() else {
                            return;
                        };
                        let logic = ui.global::<InstanceCreateLogic>();
                        logic.set_is_loader_loading(false);
                        logic.set_fabric_available(result.availability.fabric);
                        logic.set_forge_available(result.availability.forge);
                        logic.set_neoforge_available(result.availability.neoforge);

                        let Some(lt) = loader_type else { return };
                        if !result.availability.supports(lt) {
                            logic.set_mod_loader("None".into());
                            return;
                        }

                        if let Some(e) = result.error {
                            tracing::error!(error = %e, loader = %mod_loader_str, "Failed to fetch mod loader versions");
                            logic.set_loader_load_error(
                                "Failed to load versions. Please check your network and try again."
                                    .into(),
                            );
                            return;
                        }

                        if result.versions.is_empty() {
                            logic.set_loader_load_error(
                                format!(
                                    "{} has no available versions for Minecraft {}",
                                    mod_loader_str, mc_version
                                )
                                .into(),
                            );
                            return;
                        }

                        let default_idx = crate::mc_modloader::ModLoaderApi::default_version_index(
                            &result.versions,
                        );
                        let slint_versions: Vec<slint::SharedString> = result
                            .versions
                            .into_iter()
                            .map(slint::SharedString::from)
                            .collect();
                        let default_ver = slint_versions[default_idx].clone();
                        logic.set_mod_loader_versions(ModelRc::from(Rc::new(VecModel::from(
                            slint_versions,
                        ))));
                        logic.set_selected_mod_loader_version(default_ver);
                        logic.set_selected_loader_index(default_idx as i32);
                    });
                });
```

- [ ] **Step 2: Build to confirm it compiles**

Run: `cargo build`
Expected: no errors. (`avail`/`selected_available`/`versions_result` locals from the old code are gone; `result.availability` / `result.versions` / `result.error` replace them.)

- [ ] **Step 3: Run existing create.rs tests to confirm no regression**

Run: `cargo test --lib view::create::tests`
Expected: all 5 existing tests (`test_empty_name_rejected`, `test_missing_version_rejected`, `test_vanilla_without_loader_version_ok`, `test_loader_selected_but_no_version_rejected`, `test_valid_loader_version_ok`) PASS unchanged.

- [ ] **Step 4: Commit**

```bash
git add src/view/create.rs
git commit -m "refactor(instance): reuse fetch_loader_state in create dialog loader logic"
```

---

### Task 3: Extract `validate_version_and_loader` in `create.rs`

**Files:**
- Modify: `src/view/create.rs:6-22` (the `validate_create_input` function and its test module)

**Interfaces:**
- Produces: `pub(crate) fn validate_version_and_loader(version: &str, mod_loader: &str, loader_version: &str) -> Result<(), String>` — usable from `instance_detail.rs` (Task 6) without requiring an instance name.
- `validate_create_input` keeps its existing signature and behavior, now implemented in terms of `validate_version_and_loader`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `src/view/create.rs` (after `use super::validate_create_input;`, add a second import and a new test):

```rust
    use super::validate_version_and_loader;

    #[test]
    fn test_validate_version_and_loader_missing_version_rejected() {
        let err = validate_version_and_loader("", "None", "").unwrap_err();
        assert!(err.contains("Minecraft version"));
    }

    #[test]
    fn test_validate_version_and_loader_missing_loader_version_rejected() {
        let err = validate_version_and_loader("1.20.4", "Forge", "").unwrap_err();
        assert!(err.contains("Forge"));
    }

    #[test]
    fn test_validate_version_and_loader_vanilla_ok() {
        assert!(validate_version_and_loader("1.20.4", "None", "").is_ok());
    }
```

- [ ] **Step 2: Run test to verify it fails to compile**

Run: `cargo test --lib view::create::tests::test_validate_version_and_loader -- --test-threads=1`
Expected: compile error — `validate_version_and_loader` not found in `create` module.

- [ ] **Step 3: Implement the extraction**

Replace the existing `validate_create_input` function (`src/view/create.rs:6-22`) with:

```rust
pub(crate) fn validate_version_and_loader(
    version: &str,
    mod_loader: &str,
    loader_version: &str,
) -> Result<(), String> {
    if version.is_empty() {
        return Err("Please select Minecraft version".to_string());
    }
    if mod_loader != "None" && !mod_loader.is_empty() && loader_version.is_empty() {
        return Err(format!("Please select a {} version", mod_loader));
    }
    Ok(())
}

pub(crate) fn validate_create_input(
    name: &str,
    version: &str,
    mod_loader: &str,
    loader_version: &str,
) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Instance name cannot be empty".to_string());
    }
    validate_version_and_loader(version, mod_loader, loader_version)
}
```

- [ ] **Step 4: Run tests to verify everything passes**

Run: `cargo test --lib view::create::tests`
Expected: all 8 tests (5 original + 3 new) PASS.

- [ ] **Step 5: Commit**

```bash
git add src/view/create.rs
git commit -m "refactor(instance): extract validate_version_and_loader for reuse"
```

---

### Task 4: `InstanceDetailLogic` loader-editing properties (Slint)

**Files:**
- Modify: `ui/global.slint:221-225`

**Interfaces:**
- Produces (Slint properties, all on `InstanceDetailLogic`, generating matching `get_*`/`set_*` methods on the Rust `InstanceDetailLogic` handle):
  - `selected-mod-loader: string` (default `"None"`)
  - `mod-loader-versions: [string]`
  - `selected-mod-loader-version: string`
  - `selected-loader-index: int`
  - `is-loader-loading: bool`
  - `loader-load-error: string`
  - `fabric-available: bool` (default `true`)
  - `forge-available: bool` (default `true`)
  - `neoforge-available: bool` (default `true`)
  - `callback loader-changed();`

This task is purely additive — no existing property or callback is removed or renamed, so the build stays green with zero Rust changes.

- [ ] **Step 1: Add the properties**

In `ui/global.slint`, inside `export global InstanceDetailLogic { ... }`, replace:

```
    in-out property <string> version: "";
    in-out property <string> mod-loader: "";
    in-out property <[string]> version-list: [];
    in-out property <string> selected-version: "";
    callback save-version();
```

with:

```
    in-out property <string> version: "";
    in-out property <string> mod-loader: "";
    in-out property <[string]> version-list: [];
    in-out property <string> selected-version: "";

    in-out property <string> selected-mod-loader: "None";
    in-out property <[string]> mod-loader-versions: [];
    in-out property <string> selected-mod-loader-version: "";
    in-out property <int> selected-loader-index: -1;
    in-out property <bool> is-loader-loading: false;
    in-out property <string> loader-load-error: "";
    in-out property <bool> fabric-available: true;
    in-out property <bool> forge-available: true;
    in-out property <bool> neoforge-available: true;
    callback loader-changed();

    callback save-version();
```

(`save-version` is kept as-is for now; it's renamed to `save` in Task 6 together with its Rust handler and UI call site, to avoid a broken intermediate state.)

- [ ] **Step 2: Build to confirm it compiles**

Run: `cargo check`
Expected: no errors (new properties are unused so far, which is fine for Slint globals).

- [ ] **Step 3: Commit**

```bash
git add ui/global.slint
git commit -m "feat(ui): add mod loader edit state to InstanceDetailLogic"
```

---

### Task 5: Loader fetch + init wiring (`instance_detail.rs`)

**Files:**
- Modify: `src/view/instance_detail.rs`

**Interfaces:**
- Consumes:
  - `ModLoaderApi::fetch_loader_state` (Task 1)
  - `InstanceDetailLogic` properties from Task 4
- Produces:
  - `fn refresh_loader_state(ui_weak: slint::Weak<MainApp>, preferred_version: Option<String>)` — free function in `instance_detail.rs`, called both at dialog-open time and from the `loader-changed` callback.
  - `InstanceDetailLogic::on_loader_changed` handler registered in `setup_instance_detail_logic`.

- [ ] **Step 1: Add the generation counter and `refresh_loader_state`**

In `src/view/instance_detail.rs`, add this after the `spawn_log_reader` function (after its closing `}` at what is currently line 295, before `pub fn setup_instance_detail_logic`):

```rust
static DETAIL_LOADER_FETCH_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn refresh_loader_state(ui_weak: slint::Weak<MainApp>, preferred_version: Option<String>) {
    use std::sync::atomic::Ordering;

    let Some(ui) = ui_weak.upgrade() else {
        return;
    };
    let detail = ui.global::<InstanceDetailLogic>();

    let mc_version = detail.get_selected_version().to_string();
    let mod_loader_str = detail.get_selected_mod_loader().to_string();

    detail.set_mod_loader_versions(shared_model(Vec::new()));
    detail.set_selected_mod_loader_version("".into());
    detail.set_selected_loader_index(-1);
    detail.set_loader_load_error("".into());

    if mc_version.is_empty() {
        detail.set_is_loader_loading(false);
        return;
    }

    let loader_type = crate::mc_modloader::ModLoaderType::from_name(&mod_loader_str);
    detail.set_is_loader_loading(loader_type.is_some());

    let my_gen = DETAIL_LOADER_FETCH_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let ui_handle_async = ui_weak.clone();

    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if DETAIL_LOADER_FETCH_GEN.load(Ordering::SeqCst) != my_gen {
            return;
        }

        let result =
            crate::mc_modloader::ModLoaderApi::fetch_loader_state(&mc_version, loader_type).await;

        let _ = slint::invoke_from_event_loop(move || {
            if DETAIL_LOADER_FETCH_GEN.load(Ordering::SeqCst) != my_gen {
                return;
            }
            let Some(ui) = ui_handle_async.upgrade() else {
                return;
            };
            let detail = ui.global::<InstanceDetailLogic>();
            detail.set_is_loader_loading(false);
            detail.set_fabric_available(result.availability.fabric);
            detail.set_forge_available(result.availability.forge);
            detail.set_neoforge_available(result.availability.neoforge);

            let Some(lt) = loader_type else { return };
            if !result.availability.supports(lt) {
                detail.set_selected_mod_loader("None".into());
                return;
            }

            if let Some(e) = result.error {
                tracing::error!(error = %e, loader = %mod_loader_str, "Failed to fetch mod loader versions");
                detail.set_loader_load_error(
                    "Failed to load versions. Please check your network and try again."
                        .into(),
                );
                return;
            }

            if result.versions.is_empty() {
                detail.set_loader_load_error(
                    format!(
                        "{} has no available versions for Minecraft {}",
                        mod_loader_str, mc_version
                    )
                    .into(),
                );
                return;
            }

            let default_idx =
                crate::mc_modloader::ModLoaderApi::default_version_index(&result.versions);
            let idx = preferred_version
                .as_ref()
                .and_then(|pref| result.versions.iter().position(|v| v == pref))
                .unwrap_or(default_idx);

            let slint_versions: Vec<slint::SharedString> = result
                .versions
                .iter()
                .map(|s| s.as_str().into())
                .collect();
            let selected = slint_versions[idx].clone();
            detail.set_mod_loader_versions(shared_model(slint_versions));
            detail.set_selected_mod_loader_version(selected);
            detail.set_selected_loader_index(idx as i32);
        });
    });
}
```

- [ ] **Step 2: Initialize loader state when the detail dialog opens**

In `open_instance_detail`, find this line (currently line 237):

```rust
    detail.set_selected_version(c.version.as_str().into());
```

and add immediately after it:

```rust
    detail.set_selected_mod_loader(
        if c.mod_loader.is_empty() {
            "None"
        } else {
            c.mod_loader.as_str()
        }
        .into(),
    );
    let preferred_loader_version = (!c.mod_loader_version.is_empty()).then(|| c.mod_loader_version.clone());
    refresh_loader_state(ui.as_weak(), preferred_loader_version);
```

- [ ] **Step 3: Register the `loader-changed` callback**

In `setup_instance_detail_logic`, add this block right after the `on_tab_changed` registration (after its closing `});`, currently ending around line 348, before the `on_open_subfolder` registration):

```rust
    let ui_weak_for_loader = ui.as_weak();
    detail_logic.on_loader_changed(move || {
        refresh_loader_state(ui_weak_for_loader.clone(), None);
    });
```

- [ ] **Step 4: Build to confirm it compiles**

Run: `cargo check`
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add src/view/instance_detail.rs
git commit -m "feat(instance): fetch mod loader versions in instance detail dialog"
```

---

### Task 6: Unified Save (version + mod loader + loader version)

**Files:**
- Modify: `ui/global.slint:225` (rename `save-version` to `save`)
- Modify: `src/view/instance_detail.rs:456-507` — approximate original location of `on_save_version`; after Task 5's insertions the block has shifted down but is still the last handler in `setup_instance_detail_logic`, named `on_save_version`.
- Modify: `ui/components/pages/instance-detail-window.slint:429-433` (Save button)

**Interfaces:**
- Consumes: `crate::view::create::validate_version_and_loader` (Task 3).
- Produces: `InstanceDetailLogic::save()` callback (renamed from `save-version()`), whose handler persists `version`, `mod_loader`, and `mod_loader_version` together.

- [ ] **Step 1: Rename the callback in global.slint**

In `ui/global.slint`, change:

```
    callback save-version();
```

to:

```
    callback save();
```

- [ ] **Step 2: Rewrite the Rust handler**

In `src/view/instance_detail.rs`, replace the entire `on_save_version` block:

```rust
    let store_for_version = Arc::clone(&store);
    let master_for_version = Arc::clone(&master_configs);
    let ui_weak_for_version_save = ui.as_weak();
    detail_logic.on_save_version(move || {
        let Some(ui) = ui_weak_for_version_save.upgrade() else {
            return;
        };
        let detail = ui.global::<InstanceDetailLogic>();
        let id = detail.get_instance_id().to_string();
        let new_version = detail.get_selected_version().to_string();
        if new_version.is_empty() {
            return;
        }
        let updated = {
            let mut master = master_for_version.lock().unwrap();
            let Some(c) = master.iter_mut().find(|c| c.id == id) else {
                return;
            };
            c.version = new_version.clone();
            c.clone()
        };
        match store_for_version.lock().unwrap().save_one(&updated) {
            Ok(()) => {
                detail.set_version(new_version.as_str().into());
                detail.set_status_msg("✓ Saved".into());
            }
            Err(e) => detail.set_status_msg(format!("Save failed: {e}").into()),
        }
    });
```

with:

```rust
    let store_for_save = Arc::clone(&store);
    let master_for_save = Arc::clone(&master_configs);
    let ui_weak_for_save = ui.as_weak();
    detail_logic.on_save(move || {
        let Some(ui) = ui_weak_for_save.upgrade() else {
            return;
        };
        let detail = ui.global::<InstanceDetailLogic>();
        let id = detail.get_instance_id().to_string();
        let new_version = detail.get_selected_version().to_string();
        let mod_loader = detail.get_selected_mod_loader().to_string();
        let loader_version = detail.get_selected_mod_loader_version().to_string();

        if let Err(msg) =
            crate::view::create::validate_version_and_loader(&new_version, &mod_loader, &loader_version)
        {
            detail.set_status_msg(msg.into());
            return;
        }

        let updated = {
            let mut master = master_for_save.lock().unwrap();
            let Some(c) = master.iter_mut().find(|c| c.id == id) else {
                return;
            };
            c.version = new_version.clone();
            c.mod_loader = mod_loader.clone();
            c.mod_loader_version = loader_version.clone();
            c.clone()
        };
        match store_for_save.lock().unwrap().save_one(&updated) {
            Ok(()) => {
                detail.set_version(new_version.as_str().into());
                detail.set_mod_loader(mod_loader.as_str().into());
                detail.set_status_msg("✓ Saved".into());
            }
            Err(e) => detail.set_status_msg(format!("Save failed: {e}").into()),
        }
    });
```

- [ ] **Step 3: Update the Save button in the UI**

In `ui/components/pages/instance-detail-window.slint`, change:

```
                        HorizontalLayout {
                            alignment: end;
                            ToolButton {
                                label: @tr("Save Version");
                                primary: true;
                                clicked => { InstanceDetailLogic.save-version(); }
                            }
                        }
```

to:

```
                        HorizontalLayout {
                            alignment: end;
                            ToolButton {
                                label: @tr("Save");
                                primary: true;
                                clicked => { InstanceDetailLogic.save(); }
                            }
                        }
```

- [ ] **Step 4: Build to confirm it compiles**

Run: `cargo build`
Expected: no errors. (If `cargo build` reports an unresolved `on_save_version`/`save-version` anywhere, grep for stragglers: `grep -rn "save-version\|on_save_version" ui/ src/` should return nothing after this task.)

- [ ] **Step 5: Commit**

```bash
git add ui/global.slint ui/components/pages/instance-detail-window.slint src/view/instance_detail.rs
git commit -m "feat(instance): persist mod loader and loader version on Save"
```

---

### Task 7: Mod Loader picker UI in the Version tab

**Files:**
- Modify: `ui/components/pages/instance-detail-window.slint:409-425` (insert new block between "Change Version" and the Save button)

**Interfaces:**
- Consumes: `InstanceDetailLogic.selected-mod-loader`, `.mod-loader-versions`, `.selected-mod-loader-version`, `.selected-loader-index`, `.is-loader-loading`, `.loader-load-error`, `.fabric-available`, `.forge-available`, `.neoforge-available`, `.loader-changed()` (Task 4/5).

- [ ] **Step 1: Wire the version ComboBox to also trigger a loader refetch**

In `ui/components/pages/instance-detail-window.slint`, change:

```
                            ComboBox {
                                horizontal-stretch: 1;
                                height: 36px;
                                model: InstanceDetailLogic.version-list;
                                current-value <=> InstanceDetailLogic.selected-version;
                            }
```

to:

```
                            ComboBox {
                                horizontal-stretch: 1;
                                height: 36px;
                                model: InstanceDetailLogic.version-list;
                                current-value <=> InstanceDetailLogic.selected-version;
                                selected(v) => {
                                    InstanceDetailLogic.selected-version = v;
                                    InstanceDetailLogic.loader-changed();
                                }
                            }
```

- [ ] **Step 2: Insert the Mod Loader picker block**

Immediately after that `ComboBox`'s closing `}` and the enclosing `HorizontalLayout`'s closing `}` (i.e. right before the `HorizontalLayout { alignment: end; ToolButton { label: @tr("Save"); ... } }` block), insert:

```
                        VerticalLayout {
                            spacing: 8px;
                            Text { text: @tr("Mod Loader"); font-size: 12px; color: #9e9e9e; }
                            HorizontalLayout {
                                spacing: 12px; alignment: start;

                                HorizontalLayout {
                                    spacing: 8px;
                                    Rectangle {
                                        width: 20px; height: 20px; border-radius: 10px;
                                        border-width: 2px;
                                        border-color: InstanceDetailLogic.selected-mod-loader == "None" ? #00D000 : #555;
                                        background: transparent;
                                        if InstanceDetailLogic.selected-mod-loader == "None": Rectangle {
                                            width: 10px; height: 10px; x: 5px; y: 5px;
                                            border-radius: 5px; background: #00D000;
                                        }
                                        d-r0-ta := TouchArea { clicked => { InstanceDetailLogic.selected-mod-loader = "None"; InstanceDetailLogic.loader-changed(); } }
                                    }
                                    Text { text: "None"; color: #e0e0e0; font-size: 16px; vertical-alignment: center; }
                                }

                                HorizontalLayout {
                                    spacing: 8px;
                                    opacity: InstanceDetailLogic.fabric-available ? 1.0 : 0.35;
                                    Rectangle {
                                        width: 20px; height: 20px; border-radius: 10px;
                                        border-width: 2px;
                                        border-color: InstanceDetailLogic.selected-mod-loader == "Fabric" ? #00D000 : #555;
                                        background: transparent;
                                        if InstanceDetailLogic.selected-mod-loader == "Fabric": Rectangle {
                                            width: 10px; height: 10px; x: 5px; y: 5px;
                                            border-radius: 5px; background: #00D000;
                                        }
                                        d-r1-ta := TouchArea {
                                            enabled: InstanceDetailLogic.fabric-available;
                                            clicked => { InstanceDetailLogic.selected-mod-loader = "Fabric"; InstanceDetailLogic.loader-changed(); }
                                        }
                                    }
                                    Text { text: "Fabric"; color: #e0e0e0; font-size: 16px; vertical-alignment: center; }
                                }

                                HorizontalLayout {
                                    spacing: 8px;
                                    opacity: InstanceDetailLogic.forge-available ? 1.0 : 0.35;
                                    Rectangle {
                                        width: 20px; height: 20px; border-radius: 10px;
                                        border-width: 2px;
                                        border-color: InstanceDetailLogic.selected-mod-loader == "Forge" ? #00D000 : #555;
                                        background: transparent;
                                        if InstanceDetailLogic.selected-mod-loader == "Forge": Rectangle {
                                            width: 10px; height: 10px; x: 5px; y: 5px;
                                            border-radius: 5px; background: #00D000;
                                        }
                                        d-r2-ta := TouchArea {
                                            enabled: InstanceDetailLogic.forge-available;
                                            clicked => { InstanceDetailLogic.selected-mod-loader = "Forge"; InstanceDetailLogic.loader-changed(); }
                                        }
                                    }
                                    Text { text: "Forge"; color: #e0e0e0; font-size: 16px; vertical-alignment: center; }
                                }

                                HorizontalLayout {
                                    spacing: 8px;
                                    opacity: InstanceDetailLogic.neoforge-available ? 1.0 : 0.35;
                                    Rectangle {
                                        width: 20px; height: 20px; border-radius: 10px;
                                        border-width: 2px;
                                        border-color: InstanceDetailLogic.selected-mod-loader == "NeoForge" ? #00D000 : #555;
                                        background: transparent;
                                        if InstanceDetailLogic.selected-mod-loader == "NeoForge": Rectangle {
                                            width: 10px; height: 10px; x: 5px; y: 5px;
                                            border-radius: 5px; background: #00D000;
                                        }
                                        d-r3-ta := TouchArea {
                                            enabled: InstanceDetailLogic.neoforge-available;
                                            clicked => { InstanceDetailLogic.selected-mod-loader = "NeoForge"; InstanceDetailLogic.loader-changed(); }
                                        }
                                    }
                                    Text { text: "NeoForge"; color: #e0e0e0; font-size: 16px; vertical-alignment: center; }
                                }
                            }
                        }

                        if InstanceDetailLogic.selected-mod-loader != "None": VerticalLayout {
                            spacing: 6px;
                            Text { text: @tr("Mod Loader Version"); font-size: 12px; color: #9e9e9e; }
                            d-loader-trigger := Rectangle {
                                property <length> scroll-y: 0px;
                                property <length> row-h: 32px;
                                property <length> popup-max-h: 120px;
                                property <length> content-h: InstanceDetailLogic.mod-loader-versions.length * self.row-h;
                                height: 36px; border-radius: 6px;
                                border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                                background: #242424;

                                d-loader-popup := PopupWindow {
                                    x: 0; y: 36px;
                                    width: parent.width;
                                    height: min(d-loader-trigger.popup-max-h, d-loader-trigger.content-h);
                                    Rectangle {
                                        width: 100%; height: 100%;
                                        background: #1a1a24;
                                        border-radius: 6px;
                                        border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                                        clip: true;
                                        ListView {
                                            viewport-y <=> d-loader-trigger.scroll-y;
                                            for v[i] in InstanceDetailLogic.mod-loader-versions: Rectangle {
                                                height: 32px;
                                                background: v == InstanceDetailLogic.selected-mod-loader-version ? #76bc51.with-alpha(0.15) : (d-lv-ta.has-hover ? #ffffff.with-alpha(0.1) : transparent);
                                                Text {
                                                    x: 12px; height: parent.height;
                                                    text: v; color: #e0e0e0; font-size: 13px;
                                                    vertical-alignment: center;
                                                }
                                                d-lv-ta := TouchArea {
                                                    clicked => {
                                                        InstanceDetailLogic.selected-mod-loader-version = v;
                                                        InstanceDetailLogic.selected-loader-index = i;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                HorizontalLayout {
                                    padding-left: 12px; padding-right: 8px;
                                    Text {
                                        horizontal-stretch: 1;
                                        text: InstanceDetailLogic.is-loader-loading
                                            ? @tr("Loading versions...")
                                            : (InstanceDetailLogic.selected-mod-loader-version == "" ? @tr("Select...") : InstanceDetailLogic.selected-mod-loader-version);
                                        color: InstanceDetailLogic.selected-mod-loader-version == "" ? #555 : #e0e0e0;
                                        font-size: 13px; vertical-alignment: center;
                                    }
                                    Text {
                                        text: "▾";
                                        color: #888; font-size: 12px; vertical-alignment: center;
                                    }
                                }
                                TouchArea {
                                    clicked => {
                                        d-loader-trigger.scroll-y = -min(
                                            max(0px, (InstanceDetailLogic.selected-loader-index - 1) * d-loader-trigger.row-h),
                                            max(0px, d-loader-trigger.content-h - d-loader-trigger.popup-max-h));
                                        d-loader-popup.show();
                                    }
                                }
                            }
                            if InstanceDetailLogic.loader-load-error != "": Text {
                                text: InstanceDetailLogic.loader-load-error;
                                color: #e74c3c; font-size: 11px; wrap: word-wrap;
                            }
                        }
```

(Element ids are prefixed `d-` to keep them visually distinct from the Create dialog's identical block in a different file — not required for correctness since they're different components, but avoids confusion when grepping across the codebase.)

- [ ] **Step 3: Build to confirm it compiles**

Run: `cargo build`
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add ui/components/pages/instance-detail-window.slint
git commit -m "feat(ui): add mod loader picker to instance detail Version tab"
```

---

### Task 8: Full verification

**Files:** none (verification only)

- [ ] **Step 1: Run the full test suite**

Run: `cargo test`
Expected: all tests PASS (including the new ones from Tasks 1 and 3; network-dependent tests require internet access, matching pre-existing test behavior in this file).

- [ ] **Step 2: Run clippy with the CI's exact flags**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings/errors.

- [ ] **Step 3: Manual GUI verification**

Run: `cargo run`

1. Open an existing vanilla (no mod loader) instance's detail dialog → Version tab.
2. Confirm "Current Mod Loader" still shows "None" and the new Mod Loader picker below "Change Version" defaults to "None" selected.
3. Click "Forge". Confirm the loader-version dropdown shows "Loading versions..." then populates; confirm a version is preselected.
4. Pick a specific Forge version from the dropdown, click "Save".
5. Confirm the dialog shows "✓ Saved", "Current Mod Loader" now shows "Forge", and the sidebar now shows a "Mods" tab (previously hidden for vanilla).
6. Close and reopen the detail dialog for the same instance → Version tab. Confirm the Mod Loader picker now defaults to "Forge" with the previously-saved version preselected in the dropdown (not just the API's suggested default).
7. Launch the instance and confirm it starts with the correct Forge profile (per the design doc's manual test plan).
