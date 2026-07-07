# Resource/World/Shader Path Picker + Launch-Time Redirection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users pick World Save / Resource Pack / Shader Pack folders via a native OS folder picker in the Create Instance dialog, and make a custom path actually take effect by redirecting the instance's `saves`/`resourcepacks`/`shaderpacks` directory to it at launch time.

**Architecture:** A new pure function `instance_assets::sync_custom_dirs` does the directory-link work (symlink on Unix, NTFS junction on Windows) and is called once from `do_launch` right before the instance's game directory is used. Separately, three new Slint callbacks on `InstanceCreateLogic`, wired in `src/view/create.rs` using the existing `rfd::AsyncFileDialog` pattern, let the user fill the three text fields via a native folder picker instead of typing.

**Tech Stack:** Rust, Slint, `rfd` (already a dependency), new Windows-only `junction` crate dependency, `anyhow`, `tracing`, `tempfile` (tests).

## Global Constraints

- Empty field (`world_path`/`resource_pack`/`shader_pack` on `InstanceConfig`) = default = instance's own local `saves`/`resourcepacks`/`shaderpacks` — untouched.
- Non-empty field = the **entire** corresponding directory is redirected to the external folder (not merged, not copied). All three fields behave identically.
- Redirection happens at launch time only (`do_launch`), never at instance-creation time, and never copies/moves files.
- Manual typing/pasting into the three text fields must keep working exactly as today — the picker is additive, not a replacement.
- macOS/Linux: `std::os::unix::fs::symlink`. Windows: NTFS junction via the `junction` crate — not `std::os::windows::fs::symlink_dir`, since that requires Developer Mode or admin elevation which end users won't have.
- Scope is the Create Instance dialog only. No changes to the instance-edit dialog or instance-detail window (confirmed no equivalent fields exist there).

---

### Task 1: Add the `junction` Windows dependency

**Files:**
- Modify: `Cargo.toml:56-61` (the `[target.'cfg(target_os = "windows")'.dependencies]` block)

**Interfaces:**
- Produces: `junction` crate available for `#[cfg(windows)]` code in Task 2 — functions used: `junction::create(target, junction_point) -> std::io::Result<()>`.

- [ ] **Step 1: Add the dependency**

Edit the existing Windows target block in `Cargo.toml`:

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.62.2", features = [
  "Win32_Security_Cryptography",
  "Win32_Foundation",
] }
junction = "1.2.0"
```

- [ ] **Step 2: Verify it resolves**

Run: `cargo check`
Expected: succeeds (this dependency is behind the Windows `cfg` target, so on macOS/Linux `cargo metadata`/`cargo check` just needs to resolve the dependency graph without errors; it will not be compiled into the current-platform binary).

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "build: add junction crate for Windows directory link support"
```

---

### Task 2: `instance_assets::sync_custom_dirs` — the directory-link logic

**Files:**
- Modify: `src/instance_assets.rs` (add imports at top, add new functions, add tests to the existing `#[cfg(test)] mod tests` block)

**Interfaces:**
- Consumes: `crate::mc_instance::InstanceConfig` fields `world_path: String`, `resource_pack: String`, `shader_pack: String` (already exist, `src/mc_instance.rs:23-25`).
- Produces: `pub fn sync_custom_dirs(instance_dir: &Path, config: &InstanceConfig) -> anyhow::Result<()>` — called by Task 3.

- [ ] **Step 1: Add imports**

At the top of `src/instance_assets.rs`, change:

```rust
use anyhow::{Context as _, bail};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
```

to:

```rust
use anyhow::{Context as _, bail};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use tracing::warn;

use crate::mc_instance::InstanceConfig;
```

- [ ] **Step 2: Write the failing tests**

Add to the existing `#[cfg(test)] mod tests { use super::*; ... }` block at the bottom of `src/instance_assets.rs` (after the last existing test):

```rust
    fn config_with_paths(world: &str, resource: &str, shader: &str) -> InstanceConfig {
        InstanceConfig {
            world_path: world.to_string(),
            resource_pack: resource.to_string(),
            shader_pack: shader.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_sync_custom_dirs_empty_fields_are_noop() {
        let instance_dir = tempfile::tempdir().unwrap();
        let config = config_with_paths("", "", "");
        sync_custom_dirs(instance_dir.path(), &config).unwrap();
        assert!(!instance_dir.path().join("saves").exists());
        assert!(!instance_dir.path().join("resourcepacks").exists());
        assert!(!instance_dir.path().join("shaderpacks").exists());
    }

    #[test]
    fn test_sync_custom_dirs_missing_custom_path_errors() {
        let instance_dir = tempfile::tempdir().unwrap();
        let config = config_with_paths("/does/not/exist/anywhere", "", "");
        let err = sync_custom_dirs(instance_dir.path(), &config).unwrap_err();
        assert!(err.to_string().contains("World Save Path"));
    }

    #[test]
    fn test_sync_custom_dirs_links_saves_to_custom_folder() {
        let instance_dir = tempfile::tempdir().unwrap();
        let custom = tempfile::tempdir().unwrap();
        std::fs::write(custom.path().join("marker.txt"), b"hi").unwrap();

        let config = config_with_paths(custom.path().to_str().unwrap(), "", "");
        sync_custom_dirs(instance_dir.path(), &config).unwrap();

        let linked = instance_dir.path().join("saves");
        assert!(linked.join("marker.txt").exists());
    }

    #[test]
    fn test_sync_custom_dirs_is_idempotent() {
        let instance_dir = tempfile::tempdir().unwrap();
        let custom = tempfile::tempdir().unwrap();

        let config = config_with_paths(custom.path().to_str().unwrap(), "", "");
        sync_custom_dirs(instance_dir.path(), &config).unwrap();
        // Second run against the same already-correct link must not error.
        sync_custom_dirs(instance_dir.path(), &config).unwrap();

        let linked = instance_dir.path().join("saves");
        assert!(linked.exists());
    }

    #[test]
    fn test_sync_custom_dirs_relinks_when_target_changes() {
        let instance_dir = tempfile::tempdir().unwrap();
        let custom_a = tempfile::tempdir().unwrap();
        let custom_b = tempfile::tempdir().unwrap();
        std::fs::write(custom_b.path().join("only_in_b.txt"), b"hi").unwrap();

        let config_a = config_with_paths(custom_a.path().to_str().unwrap(), "", "");
        sync_custom_dirs(instance_dir.path(), &config_a).unwrap();

        let config_b = config_with_paths(custom_b.path().to_str().unwrap(), "", "");
        sync_custom_dirs(instance_dir.path(), &config_b).unwrap();

        let linked = instance_dir.path().join("saves");
        assert!(linked.join("only_in_b.txt").exists());
    }

    #[test]
    fn test_sync_custom_dirs_covers_all_three_fields() {
        let instance_dir = tempfile::tempdir().unwrap();
        let world = tempfile::tempdir().unwrap();
        let resource = tempfile::tempdir().unwrap();
        let shader = tempfile::tempdir().unwrap();

        let config = config_with_paths(
            world.path().to_str().unwrap(),
            resource.path().to_str().unwrap(),
            shader.path().to_str().unwrap(),
        );
        sync_custom_dirs(instance_dir.path(), &config).unwrap();

        assert!(instance_dir.path().join("saves").exists());
        assert!(instance_dir.path().join("resourcepacks").exists());
        assert!(instance_dir.path().join("shaderpacks").exists());
    }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test sync_custom_dirs`
Expected: FAIL to compile — `sync_custom_dirs` is not defined yet.

- [ ] **Step 4: Implement `sync_custom_dirs` and its helpers**

Add this to `src/instance_assets.rs`, right after `copy_dir_recursive` (before the `#[cfg(test)]` block):

```rust
pub fn sync_custom_dirs(instance_dir: &Path, config: &InstanceConfig) -> anyhow::Result<()> {
    sync_one("World Save Path", &config.world_path, instance_dir, "saves")?;
    sync_one(
        "Resource Pack Path",
        &config.resource_pack,
        instance_dir,
        "resourcepacks",
    )?;
    sync_one("Shader Pack Path", &config.shader_pack, instance_dir, "shaderpacks")?;
    Ok(())
}

fn sync_one(field_label: &str, field: &str, instance_dir: &Path, dir_name: &str) -> anyhow::Result<()> {
    if field.is_empty() {
        return Ok(());
    }

    let custom = PathBuf::from(field);
    if !custom.is_dir() {
        bail!(
            "{field_label} does not exist or is not a folder: {}",
            custom.display()
        );
    }
    let custom = std::fs::canonicalize(&custom)
        .with_context(|| format!("Failed to resolve {field_label}: {}", custom.display()))?;

    std::fs::create_dir_all(instance_dir)?;
    let link = instance_dir.join(dir_name);

    if let Ok(meta) = std::fs::symlink_metadata(&link) {
        let already_correct = std::fs::canonicalize(&link)
            .map(|resolved| resolved == custom)
            .unwrap_or(false);
        if already_correct {
            return Ok(());
        }

        warn!(
            link = %link.display(),
            target = %custom.display(),
            dir = dir_name,
            "Replacing existing directory entry with a link to custom path"
        );
        if meta.file_type().is_symlink() {
            remove_link(&link)?;
        } else if meta.is_dir() {
            std::fs::remove_dir_all(&link)?;
        } else {
            std::fs::remove_file(&link)?;
        }
    }

    create_dir_link(&custom, &link).with_context(|| {
        format!(
            "Failed to link {dir_name} to {field_label}: {}",
            custom.display()
        )
    })
}

#[cfg(unix)]
fn create_dir_link(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_dir_link(target: &Path, link: &Path) -> std::io::Result<()> {
    junction::create(target, link)
}

#[cfg(unix)]
fn remove_link(link: &Path) -> std::io::Result<()> {
    std::fs::remove_file(link)
}

#[cfg(windows)]
fn remove_link(link: &Path) -> std::io::Result<()> {
    std::fs::remove_dir(link)
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test sync_custom_dirs`
Expected: PASS (6 tests: `test_sync_custom_dirs_empty_fields_are_noop`, `test_sync_custom_dirs_missing_custom_path_errors`, `test_sync_custom_dirs_links_saves_to_custom_folder`, `test_sync_custom_dirs_is_idempotent`, `test_sync_custom_dirs_relinks_when_target_changes`, `test_sync_custom_dirs_covers_all_three_fields`)

- [ ] **Step 6: Verify the Windows branch compiles**

Run: `cargo check --target x86_64-pc-windows-msvc`
Expected: succeeds (this only type-checks the `#[cfg(windows)]` branch — `junction::create`/`std::fs::remove_dir` usage — it does not run the Windows tests, which require an actual Windows machine or CI runner to execute).

- [ ] **Step 7: Run the full test suite to check for regressions**

Run: `cargo test`
Expected: all tests pass, including the pre-existing `instance_assets` and `mc_instance` tests.

- [ ] **Step 8: Commit**

```bash
git add src/instance_assets.rs
git commit -m "feat(instance): add sync_custom_dirs to redirect saves/resourcepacks/shaderpacks to custom folders"
```

---

### Task 3: Wire `sync_custom_dirs` into `do_launch`

**Files:**
- Modify: `src/view/launch.rs:424-441`

**Interfaces:**
- Consumes: `instance_assets::sync_custom_dirs(instance_dir: &Path, config: &InstanceConfig) -> anyhow::Result<()>` from Task 2.

- [ ] **Step 1: Insert the call before `LaunchContext` is built**

In `src/view/launch.rs`, replace:

```rust
    let ctx = LaunchContext {
        version,
        java_path,
        game_dir: paths.instance_dir(&instance_id),
        libraries_dir: paths.libraries_dir(),
```

with:

```rust
    let instance_dir = paths.instance_dir(&instance_id);
    crate::instance_assets::sync_custom_dirs(&instance_dir, &config)
        .context("Failed to apply custom World Save / Resource Pack / Shader Pack paths")?;

    let ctx = LaunchContext {
        version,
        java_path,
        game_dir: instance_dir,
        libraries_dir: paths.libraries_dir(),
```

(`anyhow::Context` is already imported at `src/view/launch.rs:11`, and `instance_assets` is already a top-level module referenced elsewhere via `crate::instance_assets::...`, e.g. `src/view/instance_detail.rs`.)

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: succeeds, no borrow/move errors (`instance_dir` is moved into `ctx.game_dir` after being borrowed by `sync_custom_dirs`, which only needs `&Path`).

- [ ] **Step 3: Run the full test suite**

Run: `cargo test`
Expected: all tests pass (no test exercises `do_launch` directly — it requires live network/Minecraft installs — so this step is a regression check on the rest of the suite).

- [ ] **Step 4: Manual smoke test**

This is documented rather than automated, since `do_launch` requires a real Minecraft install and cannot be unit-tested:
1. Run `cargo run`.
2. Create a new instance, and in the Resources tab set World Save Path to an existing folder on disk that contains a `level.dat`-bearing world subfolder (or any folder — an empty one is fine to just confirm the link is made).
3. Launch the instance.
4. In a terminal, confirm the link exists: `ls -la <instance_dir>/saves` should show it as a symlink (macOS/Linux) pointing at the folder you chose. `<instance_dir>` is under the app's data dir (`directories::ProjectDirs` data dir, e.g. `~/Library/Application Support/com.Duacodie.VoxelRuler/instances/<id>` on macOS) — the exact path is also visible in the debug log line `"Launch command"` (`game_dir = ...`).
5. Confirm Minecraft's in-game "Select World" screen shows the world(s) from the custom folder.

- [ ] **Step 5: Commit**

```bash
git add src/view/launch.rs
git commit -m "feat(launch): redirect saves/resourcepacks/shaderpacks to custom paths before launch"
```

---

### Task 4: Add folder-picker callbacks and buttons to the Create Instance dialog UI

**Files:**
- Modify: `ui/global.slint:530-573` (the `InstanceCreateLogic` global)
- Modify: `ui/components/pages/create-instance-dialog.slint:1-3` (imports) and `:582-644` (the Resources tab body)

**Interfaces:**
- Produces: three new callbacks `InstanceCreateLogic.browse-world-path()`, `InstanceCreateLogic.browse-resource-pack()`, `InstanceCreateLogic.browse-shader-pack()` — consumed by Task 5's Rust handlers.

- [ ] **Step 1: Add the callbacks to `InstanceCreateLogic`**

In `ui/global.slint`, inside `export global InstanceCreateLogic { ... }`, change:

```slint
    in-out property <string>   world-path: "";
    in-out property <string>   resource-pack: "";
    in-out property <string>   shader-pack: "";
```

to:

```slint
    in-out property <string>   world-path: "";
    in-out property <string>   resource-pack: "";
    in-out property <string>   shader-pack: "";
    callback browse-world-path();
    callback browse-resource-pack();
    callback browse-shader-pack();
```

- [ ] **Step 2: Import `Assets` in the create-instance dialog**

In `ui/components/pages/create-instance-dialog.slint`, change:

```slint
import { AppTheme } from "@theme";
import { InstanceCreateLogic } from "@global";
import { ScrollView, ListView } from "std-widgets.slint";
```

to:

```slint
import { AppTheme } from "@theme";
import { InstanceCreateLogic } from "@global";
import { Assets } from "@assets";
import { ScrollView, ListView } from "std-widgets.slint";
```

- [ ] **Step 3: Add the icon buttons to the three path fields**

In `ui/components/pages/create-instance-dialog.slint`, replace the whole Resources tab body (currently lines 582-644):

```slint
                if InstanceCreateLogic.active-tab == 2: VerticalLayout {
                    padding: 20px; spacing: 16px; alignment: start;

                    VerticalLayout {
                        spacing: 6px;
                        Text { text: @tr("World/Save Path"); font-size: 12px; color: #9e9e9e; }
                        Rectangle {
                            height: 36px; border-radius: 6px;
                            border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                            background: #242424;
                            world-input := TextInput {
                                x: 12px; width: parent.width - 24px; height: parent.height;
                                text <=> InstanceCreateLogic.world-path;
                                color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                            }
                            if world-input.text == "": Text {
                                x: 12px; height: parent.height;
                                text: @tr("Full path to world (optional)");
                                color: #555; font-size: 13px; vertical-alignment: center;
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
                            rp-input := TextInput {
                                x: 12px; width: parent.width - 24px; height: parent.height;
                                text <=> InstanceCreateLogic.resource-pack;
                                color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                            }
                            if rp-input.text == "": Text {
                                x: 12px; height: parent.height;
                                text: @tr("Full path to resource pack (optional)");
                                color: #555; font-size: 13px; vertical-alignment: center;
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
                            sp-input := TextInput {
                                x: 12px; width: parent.width - 24px; height: parent.height;
                                text <=> InstanceCreateLogic.shader-pack;
                                color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                            }
                            if sp-input.text == "": Text {
                                x: 12px; height: parent.height;
                                text: @tr("Full path to shader pack (optional)");
                                color: #555; font-size: 13px; vertical-alignment: center;
                            }
                        }
                    }
                }
```

with:

```slint
                if InstanceCreateLogic.active-tab == 2: VerticalLayout {
                    padding: 20px; spacing: 16px; alignment: start;

                    VerticalLayout {
                        spacing: 6px;
                        Text { text: @tr("World/Save Path"); font-size: 12px; color: #9e9e9e; }
                        HorizontalLayout {
                            spacing: 6px;
                            Rectangle {
                                horizontal-stretch: 1;
                                height: 36px; border-radius: 6px;
                                border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                                background: #242424;
                                world-input := TextInput {
                                    x: 12px; width: parent.width - 24px; height: parent.height;
                                    text <=> InstanceCreateLogic.world-path;
                                    color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                                }
                                if world-input.text == "": Text {
                                    x: 12px; height: parent.height;
                                    text: @tr("Full path to world (optional)");
                                    color: #555; font-size: 13px; vertical-alignment: center;
                                }
                            }
                            Rectangle {
                                width: 36px; height: 36px; border-radius: 6px;
                                border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                                background: world-browse-ta.has-hover ? #ffffff.with-alpha(0.1) : #2a2a2a;
                                world-browse-ta := TouchArea {
                                    clicked => { InstanceCreateLogic.browse-world-path(); }
                                }
                                Image {
                                    source: Assets.folder;
                                    width: 16px; height: 16px;
                                    image-fit: contain;
                                    colorize: #c0c0c0;
                                }
                            }
                        }
                    }

                    VerticalLayout {
                        spacing: 6px;
                        Text { text: @tr("Resource Pack Path"); font-size: 12px; color: #9e9e9e; }
                        HorizontalLayout {
                            spacing: 6px;
                            Rectangle {
                                horizontal-stretch: 1;
                                height: 36px; border-radius: 6px;
                                border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                                background: #242424;
                                rp-input := TextInput {
                                    x: 12px; width: parent.width - 24px; height: parent.height;
                                    text <=> InstanceCreateLogic.resource-pack;
                                    color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                                }
                                if rp-input.text == "": Text {
                                    x: 12px; height: parent.height;
                                    text: @tr("Full path to resource pack (optional)");
                                    color: #555; font-size: 13px; vertical-alignment: center;
                                }
                            }
                            Rectangle {
                                width: 36px; height: 36px; border-radius: 6px;
                                border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                                background: rp-browse-ta.has-hover ? #ffffff.with-alpha(0.1) : #2a2a2a;
                                rp-browse-ta := TouchArea {
                                    clicked => { InstanceCreateLogic.browse-resource-pack(); }
                                }
                                Image {
                                    source: Assets.folder;
                                    width: 16px; height: 16px;
                                    image-fit: contain;
                                    colorize: #c0c0c0;
                                }
                            }
                        }
                    }

                    VerticalLayout {
                        spacing: 6px;
                        Text { text: @tr("Shader Pack Path"); font-size: 12px; color: #9e9e9e; }
                        HorizontalLayout {
                            spacing: 6px;
                            Rectangle {
                                horizontal-stretch: 1;
                                height: 36px; border-radius: 6px;
                                border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                                background: #242424;
                                sp-input := TextInput {
                                    x: 12px; width: parent.width - 24px; height: parent.height;
                                    text <=> InstanceCreateLogic.shader-pack;
                                    color: #e0e0e0; font-size: 13px; vertical-alignment: center;
                                }
                                if sp-input.text == "": Text {
                                    x: 12px; height: parent.height;
                                    text: @tr("Full path to shader pack (optional)");
                                    color: #555; font-size: 13px; vertical-alignment: center;
                                }
                            }
                            Rectangle {
                                width: 36px; height: 36px; border-radius: 6px;
                                border-width: 1px; border-color: #ffffff.with-alpha(0.15);
                                background: sp-browse-ta.has-hover ? #ffffff.with-alpha(0.1) : #2a2a2a;
                                sp-browse-ta := TouchArea {
                                    clicked => { InstanceCreateLogic.browse-shader-pack(); }
                                }
                                Image {
                                    source: Assets.folder;
                                    width: 16px; height: 16px;
                                    image-fit: contain;
                                    colorize: #c0c0c0;
                                }
                            }
                        }
                    }
                }
```

- [ ] **Step 4: Verify it builds**

Run: `cargo check`
Expected: succeeds — `build.rs` recompiles the `.slint` files; a typo or unresolved callback/import would surface here as a build error.

- [ ] **Step 5: Commit**

```bash
git add ui/global.slint ui/components/pages/create-instance-dialog.slint
git commit -m "feat(ui): add folder-picker buttons to World/Resource Pack/Shader Pack fields"
```

---

### Task 5: Wire the browse callbacks to native folder pickers

**Files:**
- Modify: `src/view/create.rs` (add handlers inside `setup_create_logic`)

**Interfaces:**
- Consumes: `InstanceCreateLogic.on_browse_world_path/on_browse_resource_pack/on_browse_shader_pack` (generated Slint bindings from Task 4's callbacks), `rfd::AsyncFileDialog`.

- [ ] **Step 1: Add the three handlers**

In `src/view/create.rs`, inside `setup_create_logic`, right after the `create_logic` binding is obtained (after `let create_logic = ui.global::<InstanceCreateLogic>();` at line 72), add:

```rust
    // rfd 在 Linux 用 xdg-portal 後端（免 GTK 依賴，AppImage 友善）僅提供 async API，故用 spawn_local 等待
    let ui_weak_for_world_browse = ui.as_weak();
    create_logic.on_browse_world_path(move || {
        let ui_weak = ui_weak_for_world_browse.clone();
        let _ = slint::spawn_local(async move {
            if let Some(dir) = rfd::AsyncFileDialog::new()
                .set_title("Select World Save Folder")
                .pick_folder()
                .await
                && let Some(ui) = ui_weak.upgrade()
            {
                ui.global::<InstanceCreateLogic>()
                    .set_world_path(dir.path().display().to_string().into());
            }
        });
    });

    let ui_weak_for_rp_browse = ui.as_weak();
    create_logic.on_browse_resource_pack(move || {
        let ui_weak = ui_weak_for_rp_browse.clone();
        let _ = slint::spawn_local(async move {
            if let Some(dir) = rfd::AsyncFileDialog::new()
                .set_title("Select Resource Packs Folder")
                .pick_folder()
                .await
                && let Some(ui) = ui_weak.upgrade()
            {
                ui.global::<InstanceCreateLogic>()
                    .set_resource_pack(dir.path().display().to_string().into());
            }
        });
    });

    let ui_weak_for_sp_browse = ui.as_weak();
    create_logic.on_browse_shader_pack(move || {
        let ui_weak = ui_weak_for_sp_browse.clone();
        let _ = slint::spawn_local(async move {
            if let Some(dir) = rfd::AsyncFileDialog::new()
                .set_title("Select Shader Packs Folder")
                .pick_folder()
                .await
                && let Some(ui) = ui_weak.upgrade()
            {
                ui.global::<InstanceCreateLogic>()
                    .set_shader_pack(dir.path().display().to_string().into());
            }
        });
    });
```

- [ ] **Step 2: Verify it builds**

Run: `cargo check`
Expected: succeeds.

- [ ] **Step 3: Run the full test suite**

Run: `cargo test`
Expected: all tests pass (`create.rs`'s existing `validate_create_input` tests included — this task doesn't touch that function).

- [ ] **Step 4: Manual verification**

Since native OS dialogs aren't practically unit-testable (consistent with the existing untested `on_browse_java` handlers in `src/view/mod.rs`):
1. Run `cargo run`.
2. Open "New Instance", go to the Resources tab.
3. Click each of the three folder icons; confirm the native OS folder picker opens, and selecting a folder fills the corresponding text field with its path.
4. Confirm you can still type/paste directly into the text fields as before.

- [ ] **Step 5: Commit**

```bash
git add src/view/create.rs
git commit -m "feat(create): wire folder-picker buttons to native dialogs for World/Resource Pack/Shader Pack paths"
```
