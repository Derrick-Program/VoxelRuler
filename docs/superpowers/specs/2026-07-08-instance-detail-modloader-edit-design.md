# Instance Detail — Editable Mod Loader (Design)

Date: 2026-07-08
Status: Approved for planning

## Problem

The Version tab in the instance detail dialog (`InstanceDetailLogic`, rendered in
`ui/components/pages/instance-detail-window.slint`) lets a user change the Minecraft
version but only *displays* the mod loader (Forge/Fabric/NeoForge/None) as read-only
text. There is no way to change an existing instance's mod loader or its loader
version after creation — users would have to delete and recreate the instance.

The Create Instance dialog (`InstanceCreateLogic`, `create.rs`,
`create-instance-dialog.slint`) already has this exact capability: a loader picker
(None/Fabric/Forge/NeoForge), a loader-version dropdown populated from
`mc_modloader::ModLoaderApi`, availability checks per Minecraft version, and
debounced/race-safe refetching when either the MC version or the loader selection
changes.

## Goal

Bring the same capability to the instance detail Version tab, reusing the Create
dialog's visual style and interaction pattern, with a single "Save" action that
persists Minecraft version + mod loader + mod loader version together.

## Non-goals

- No migration/cleanup of old mod loader files on disk when switching loaders —
  matches existing behavior for MC version changes (files are re-downloaded lazily
  on next launch, per the existing "Changing the version will re-download game
  files on next launch" notice).
- No separate save buttons for version vs. mod loader (rejected in favor of a single
  unified Save, matching Create dialog's single-submit model).

## Design

### 1. Shared fetch helper (`src/mc_modloader.rs`)

Extract the HTTP-fetching portion of `create.rs::on_loader_changed` (currently ~40
lines: `check_availability` + conditional `get_loader_versions`) into a reusable
async function on `ModLoaderApi`:

```rust
pub struct LoaderFetchResult {
    pub availability: LoaderAvailability,
    pub versions: Vec<String>,   // empty if loader is None or has no versions for this MC version
    pub error: Option<String>,   // set on fetch failure; distinct from "no versions"
}

impl ModLoaderApi {
    pub async fn fetch_loader_state(mc_version: &str, loader: Option<ModLoaderType>) -> LoaderFetchResult
}
```

Behavior: always checks availability for `mc_version`. If `loader` is `Some` and
that loader is available, also fetches its version list. If `loader` is `Some` but
unavailable, `versions` is empty and `error` is `None` (caller decides how to react
— e.g. reset selection to None). Fetch failures populate `error` with a
user-presentable message; "loader has zero published versions for this MC version"
is also surfaced via `error` (mirrors current `create.rs` messaging).

Debounce (100ms sleep) and the generation-counter race guard (`AtomicU64`, to
discard stale responses from rapid consecutive changes) stay in each call site
(`create.rs`, `instance_detail.rs`) since they're UI-timing concerns, not fetch
logic. Each call site also keeps its own "which index to preselect" logic, since
Create always wants the API-suggested default while Instance Detail's *initial*
load wants to preserve the instance's already-saved loader version if still valid.

`create.rs::on_loader_changed` is refactored to call `fetch_loader_state` instead of
inlining the fetch, with no behavior change.

### 2. State (`ui/global.slint`, `InstanceDetailLogic`)

Add an editable-state property group, parallel to `InstanceCreateLogic`'s naming:

```
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
```

The existing `version` / `mod-loader` properties remain as the read-only "currently
saved" display; the new properties represent in-progress edits, mirroring the
Create dialog's split between form state and submission.

### 3. UI (`ui/components/pages/instance-detail-window.slint`, `active-tab == 1`)

- Keep the existing "Current Version" and "Current Mod Loader" read-only rows.
- "Change Version" `ComboBox` gains a `selected(v) => { InstanceDetailLogic.selected-version = v; InstanceDetailLogic.loader-changed(); }` handler (changing MC version can invalidate the current loader/version choice, same as Create).
- Port the Mod Loader picker block from `create-instance-dialog.slint` (lines
  ~353–508: the None/Fabric/Forge/NeoForge circular radio row, the conditional
  loader-version dropdown with its `PopupWindow`/`ListView`, and the loader-load
  error text), rebinding every `InstanceCreateLogic.*` reference to
  `InstanceDetailLogic.*`. Place it between "Change Version" and the Save button.
- Rename the "Save Version" button label to "Save" (it now persists three fields,
  not one).

### 4. Rust logic (`src/view/instance_detail.rs`)

**Tab load / dialog open** (`load_detail_tab`, case `1`, currently falls into the
catch-all `_ => {}`): set `selected-mod-loader` to the instance's current
`mod_loader` (normalizing `""` to `"None"`), then run the fetch (via a shared
internal function, see below) with the instance's saved `mod_loader_version` as the
*preferred* selection — if that version string is present in the fetched list,
select it; otherwise fall back to the API-suggested default index
(`ModLoaderApi::default_version_index`).

**`on_loader_changed`** (new callback, fired by the loader radio buttons and by the
version ComboBox's `selected` handler): same debounce + generation-counter shape as
`create.rs::on_loader_changed`, calling `fetch_loader_state`. Always selects the
API-suggested default (no "preferred version" — the user is actively changing
something, so there's no saved value to preserve).

Both of the above share one internal function,
`fn refresh_loader_state(ui: &MainApp, gen: &Arc<AtomicU64>, preferred_version: Option<String>)`,
parameterized by `preferred_version` (`Some(saved_version)` on initial tab load,
`None` on user-driven changes), to avoid duplicating the debounce/generation-counter
scaffolding twice within this file.

**`on_save_version` → renamed `on_save`**: reads `selected-version`,
`selected-mod-loader`, `selected-mod-loader-version`. Validates with the same rule
as `validate_create_input` (loader ≠ "None" requires a non-empty loader version) —
reuse `create::validate_create_input` rather than duplicating the check. Updates
`InstanceConfig.version` / `.mod_loader` / `.mod_loader_version`, persists via the
store, and on success updates the read-only `version` / `mod-loader` display
properties so the "Mods" tab's visibility (`is-vanilla` check in
`instance-detail-window.slint`) reacts immediately.

## Error handling

- Fetch failures set `loader-load-error`, shown under the loader-version dropdown
  (same placement/style as Create).
- Save validation failure reuses `status-msg` (existing error surface for this
  dialog) rather than introducing a new property.
- If a previously-selected loader becomes unavailable for a newly-chosen MC version
  (detected via `LoaderFetchResult.availability`), reset `selected-mod-loader` to
  `"None"` — same behavior as Create.

## Testing

- Unit tests for `ModLoaderApi::fetch_loader_state`: available loader with versions,
  unavailable loader (empty versions, no error), and a fetch-failure path — follow
  existing test conventions in `mc_modloader.rs`/`create.rs` (checking whether
  network mocking is already set up there; if not, scope these to the
  pure-logic branches that don't require a live network call, consistent with
  current test coverage in the codebase).
- Unit tests for `on_save` validation reusing `validate_create_input` — extend the
  existing test table in `create.rs` if it's exported, or add equivalent cases in
  `instance_detail.rs`'s test module.
- Manual GUI verification (required before this is considered done, per project
  convention of testing UI changes live): open a vanilla instance's detail dialog →
  switch to Forge → pick a loader version → Save → confirm the "Mods" tab appears
  and the instance launches with the correct Forge profile on next launch.
