# Resource/World/Shader Path Picker + Launch-Time Redirection

Date: 2026-07-07
Status: Approved

## Problem

The Create Instance dialog (`ui/components/pages/create-instance-dialog.slint`, resources tab / `active-tab == 2`) has three optional path fields backed by `InstanceCreateLogic`: `world-path`, `resource-pack`, `shader-pack`. They are plain `TextInput`s — users can only type or paste a path. The values are stored verbatim on `InstanceConfig` (`world_path`, `resource_pack`, `shader_pack` in `src/mc_instance.rs`) but nothing downstream ever reads them; setting a custom path currently has zero effect on the running game.

Two things are missing:
1. A native "browse" affordance next to each field (users can still type/paste; the picker is an addition, not a replacement).
2. Actual behavior: when a field is set, Minecraft must use that folder instead of the instance's own `saves`/`resourcepacks`/`shaderpacks` folder.

## Scope

These three fields exist only in the Create Instance dialog — there is no equivalent in the instance-edit UI, confirmed by grep across `instance-edit*`/`InstanceEditLogic`. So `world_path`/`resource_pack`/`shader_pack` are fixed at instance creation and never change afterward. This matters for the launch-time redirection design below: the instance's local `saves`/`resourcepacks`/`shaderpacks` directories are always either brand-new (first launch after creation) or already correctly linked (subsequent launches) — never a pre-existing real directory with user data that a naive symlink-replace could clobber.

## Behavior

- Empty field (default): instance uses its own local `<instance_dir>/saves`, `/resourcepacks`, `/shaderpacks` — unchanged from today.
- Non-empty field: the *entire* corresponding directory is redirected to the user-chosen external folder (not merged, not copied). E.g. setting World Save Path to `/Users/x/SharedSaves` means `<instance_dir>/saves` becomes a link to `/Users/x/SharedSaves`, and whatever worlds live in that folder are what Minecraft's world list shows. Same semantics for Resource Pack Path → `resourcepacks` and Shader Pack Path → `shaderpacks`. All three fields behave identically — this is deliberate for consistency and implementation simplicity.
- Redirection happens at launch time (`do_launch`), not at instance-creation time, and never copies/moves files.

## UI Changes

`ui/global.slint` (`InstanceCreateLogic`): add three callbacks —
```
callback browse-world-path();
callback browse-resource-pack();
callback browse-shader-pack();
```

`ui/components/pages/create-instance-dialog.slint` (tab index 2, lines ~582-644): wrap each existing `Rectangle { ... TextInput ... }` block in a `HorizontalLayout` with a new 36×36 icon button to its right, using the existing `Assets.folder` icon (already defined in `ui/assets/index.slint:5`). Button style matches existing small-button conventions in the codebase (`border-radius: 6px`, `background: #2a2a2a`, hover → `#ffffff.with-alpha(0.1)`, per the `settings.slint` Browse button). Clicking calls the corresponding `browse-*` callback.

No change to placeholder text behavior or the `<=>` two-way bindings on the text fields — typing/pasting keeps working exactly as now.

## Rust Wiring — Picker (`src/view/create.rs`)

Add three `on_browse_*` handlers in `setup_create_logic`, following the existing `on_browse_java` pattern in `src/view/mod.rs:483-496` (async, `slint::spawn_local`, `rfd::AsyncFileDialog`). Difference: use `.pick_folder()` instead of `.pick_file()`, since all three fields are whole-directory redirects:

```rust
let ui_weak = ui_weak_for_world_browse.clone();
create_logic.on_browse_world_path(move || {
    let ui_weak = ui_weak.clone();
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
```

Same shape for `browse_resource_pack` → `resource-pack` (title "Select Resource Packs Folder") and `browse_shader_pack` → `shader-pack` (title "Select Shader Packs Folder").

## Launch-Time Redirection

New function in `src/instance_assets.rs` (co-located with other instance-directory utilities like `copy_dir_recursive`):

```rust
pub fn sync_custom_dirs(instance_dir: &Path, config: &InstanceConfig) -> anyhow::Result<()>
```

For each of the three `(field, dir_name)` pairs — `(world_path, "saves")`, `(resource_pack, "resourcepacks")`, `(shader_pack, "shaderpacks")`:

- If `field` is empty: skip (leave whatever is there — the instance's own local folder).
- If `field` is non-empty:
  - Resolve `custom = PathBuf::from(field)`; if it doesn't exist or isn't a directory, return an error naming the field and path (surfaces through `do_launch`'s existing error path).
  - `link = instance_dir.join(dir_name)`.
  - If `link` doesn't exist: create parent dirs as needed, then create the symlink/junction `link -> custom`.
  - If `link` exists and is already a symlink/junction pointing at `custom`: no-op (idempotent — this is the common case on every launch after the first).
  - If `link` exists and points somewhere else (or isn't a link at all): remove it and recreate pointing at `custom`. Per the Scope section, this only happens if creation-time state is out of sync (e.g. field edited by hand in a stored config file) — never expected to discard real user data in normal use, but the removal is still logged at `warn!` level for visibility.

Call `sync_custom_dirs(&paths.instance_dir(&instance_id), &config)` in `do_launch` (`src/view/launch.rs`) right after `paths.instance_dir(&instance_id)` is first known (near line 427, before `LaunchContext` is built), so a bad/missing custom path fails fast before any download work.

## Cross-Platform Symlink Strategy

- **macOS / Linux**: `std::os::unix::fs::symlink(target, link)` — no special privileges needed for directories.
- **Windows**: NTFS junction, not `std::os::windows::fs::symlink_dir`. Directory symlinks on Windows require Developer Mode or admin elevation, which we cannot assume for end users. Junctions do not have that requirement for local directories. Add the `junction` crate (small, no unsafe code required on our side) as a `[target.'cfg(target_os = "windows")'.dependencies]` entry in `Cargo.toml`, alongside the existing `windows` dep. `sync_custom_dirs` dispatches via `#[cfg(...)]` to `std::os::unix::fs::symlink` or `junction::create`.

## Error Handling

No new UI surface — errors from `sync_custom_dirs` propagate as `anyhow::Error` through `do_launch`'s existing `?`-based error flow, which already renders failures via the current error-message UI path (same mechanism as other pre-launch failures like missing classpath files or bad custom Java paths).

## Testing

- `sync_custom_dirs`: unit tests with `tempfile::tempdir()` covering: empty field is no-op, first-time link creation, idempotent re-run (no error, no recreation), missing custom path returns an error naming the field. Platform-specific link creation covered via `#[cfg(unix)]` / `#[cfg(windows)]` test variants (or by asserting via `std::fs::read_link` / junction-detection rather than assuming one platform).
- No new automated test for the `rfd` picker callbacks themselves — consistent with the existing `on_browse_java` handlers, which are also untested (native OS dialogs aren't practically unit-testable).
- Existing `validate_create_input` tests in `src/view/create.rs` are unaffected.

## Out of Scope

- No changes to the instance-edit dialog or instance-detail window (their resourcepacks/shaderpacks file-management UI is a separate, already-implemented feature using copy-based `add_file`).
- No support for redirecting to a single pack file (all three fields are whole-directory redirects only, per the approved design).
- No migration/backfill for instances created before this change — their fields are already empty strings (default), so behavior is unaffected until a user recreates or otherwise sets a custom path (not possible today since there's no edit UI for these fields).
