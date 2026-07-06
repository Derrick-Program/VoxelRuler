# Instance Sort Settings — Design

**Date:** 2026-07-06
**Status:** Approved

## Problem

Instances page always shows instance cards sorted by creation time (newest first), hardcoded in `InstanceStore::load()`. Users want to choose among 4 sort criteria, each with ascending/descending direction, configured from the Settings page.

## Scope

- Sort modes: 字典排序 (name, alphabetical) / 版本排序 (version) / 创建时间 (created_at) / 最后游玩 (last_played)
- Each mode supports ascending/descending toggle.
- Setting lives on the Settings page; it controls the instance card order on the Instances page.
- Applies immediately on change (no Save button needed) and persists to `settings.toml`.

## Key architectural insight

`InstanceStore::load()` ([mc_instance.rs:67](../../../src/mc_instance.rs#L67)) is the single sort authority already: initial startup load, `append()` (new instance), and the file-watch refresh loop in `view/mod.rs` all funnel through it to rebuild `master_configs`. Downstream consumers (search filter in `launch.rs`) only `iter().filter()` over `master_configs`, preserving whatever order it's already in. Therefore the sort preference only needs to be applied once, inside `load()` — no other call site needs to duplicate sort logic.

## 1. Data & persistence (`settings.rs`)

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
    fn default() -> Self { SortMode::CreatedAt }
}
```

`AppSettings` gains:
```rust
#[serde(default)]
pub sort_mode: SortMode,
#[serde(default)]
pub sort_ascending: bool, // false = preserves current "newest first" default
```

`#[serde(default)]` ensures old `settings.toml` files without these fields still load fine.

## 2. Sort algorithm (`mc_instance.rs`)

**Correction found during planning:** `load()` must *not* call `AppSettings::load()` internally — that would make `mc_instance.rs`'s unit tests (which use a tempdir for `base_dir`) implicitly depend on whatever real `settings.toml` happens to exist on the machine running the tests, breaking test hermeticity. Instead, `InstanceStore` holds the sort preference as explicit state, set from outside:

```rust
pub struct InstanceStore {
    base_dir: PathBuf,
    sort_mode: SortMode,
    sort_ascending: bool,
}

impl InstanceStore {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir, sort_mode: SortMode::default(), sort_ascending: false }
    }

    pub fn set_sort(&mut self, mode: SortMode, ascending: bool) {
        self.sort_mode = mode;
        self.sort_ascending = ascending;
    }
}
```

`load()` uses `self.sort_mode` / `self.sort_ascending` instead of a hardcoded comparator. Defaults (`SortMode::CreatedAt`, `ascending: false`) match today's hardcoded "newest first" behavior, so all existing tests keep passing unchanged.

Callers read `AppSettings::load()` themselves and call `store.set_sort(...)` once before the first `load()`/`append()` — `view/mod.rs` does this at startup, and again in the new `on_sort_changed` handler whenever the user changes the setting. Because `InstanceStore` is shared via `Arc<Mutex<InstanceStore>>`, every other caller of `load()`/`append()` (file-watch refresh, create-instance flow) automatically picks up whatever sort was last configured — no other call site needs to change.

Replace the hardcoded `instances.sort_by(|a, b| b.1.cmp(&a.1))` at the end of `load()` with a mode-aware sort using `self.sort_mode` / `self.sort_ascending`:

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
        if ord != Equal { return ord; }
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
        SortMode::CreatedAt => { // existing created_at-or-mtime fallback key
            let ord = a_key.cmp(b_key);
            if ascending { ord } else { ord.reverse() }
        }
        // Emptiness placement (never played sinks last) must NOT flip with
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

Notes:
- `created_at` keeps the existing fallback-to-mtime `sort_key` computation already in `load()`.
- `last_played` empty string (never launched) always sorts last regardless of `ascending`.
- Version comparison splits on `.` and compares numeric segments as integers when possible, falling back to string comparison per-segment when either side isn't a valid number (covers snapshot/beta formats like `24w14a`, `b1.7.3` without special-casing them).

## 3. UI (`global.slint` + `settings.slint`)

`SettingsLogic` additions:
```
in-out property <[string]> sort-mode-list: ["Name (A-Z)", "Version", "Created Time", "Last Played"];
in-out property <string>   selected-sort-mode: "Created Time";
in-out property <bool>     sort-ascending: false;
callback sort-changed();
```

`settings.slint` gets a new card "Instance Sorting", styled like the existing Java card:
- `ComboBox` bound to `sort-mode-list` / `selected-sort-mode`
- A direction toggle button showing generic `↑ Ascending` / `↓ Descending` text (not per-mode labels — keeps the Slint file simple)
- Both fire `SettingsLogic.sort-changed()` on change/click — no Save button needed, matches "apply immediately" decision.

## 4. Rust wiring (`view/mod.rs`)

- At startup, right after `master_configs` is first built (~[mod.rs:111-114](../../../src/view/mod.rs#L111-L114)), read `AppSettings::load()` and call `store.lock().unwrap().set_sort(settings.sort_mode, settings.sort_ascending)` **before** the initial `.load()` call so the very first render is already sorted correctly.
- Alongside the existing `AppSettings::load()` block (~[mod.rs:362](../../../src/view/mod.rs#L362)) that populates Java settings, also set `selected-sort-mode` / `sort-ascending` on `SettingsLogic` from the same loaded settings via a new `sort_mode_to_label()` helper (mirrors existing `java_mode_to_label`/`java_label_to_mode` pattern).
- New `on_sort_changed` handler on `SettingsLogic`:
  1. Read `selected-sort-mode` (map back to `SortMode` via `label_to_sort_mode()`) and `sort-ascending` from the UI.
  2. Load `AppSettings`, update the two fields, save back to `settings.toml`.
  3. Call `store.set_sort(new_mode, new_ascending)` then `store.load()` (now sorted per the new settings) and replace `master_configs`.
  4. Refresh the Instances page list via a new shared helper `refresh_instance_list(...)` — re-applies the current search filter and rebuilds `InstanceData` items exactly like the existing file-watch refresh path does.
- Extract `refresh_instance_list()` from the duplicated "filter by search text → map to `InstanceData` → push empty-placeholder if empty → `set_instance_list`" block that currently exists in both the initial-load and file-watch-refresh code paths ([mod.rs:151-177](../../../src/view/mod.rs#L151-L177)), so the new sort-changed call site becomes the third user instead of a third copy-paste.

## 5. Testing

- `mc_instance.rs` unit tests:
  - `compare_versions`: `"1.7.2"` vs `"1.21.7"` → `Less`; `"1.7.10"` vs `"1.7.2"` → `Greater`; mixed formats like `"24w14a"` vs `"1.20.4"` fall back to string compare without panicking.
  - `sort_instances`: each of the 4 modes in both directions; `last_played` empty-string instances always sink to the bottom regardless of `ascending`.
- `settings.rs` unit test: `SortMode` TOML round-trip; confirm settings files without the new fields still deserialize via `#[serde(default)]`.
- Manual verification (Slint UI, not unit-testable): launch app → Settings page → change sort mode/direction → confirm Instances page card order updates immediately, and the choice persists across app restart.

## Out of scope

- Per-instance-list sort control on the Instances page itself (explicitly deferred — this setting lives only in Settings page per user request).
- Special-casing version family ordering (alpha/beta/snapshot vs release) beyond the numeric-segment-with-string-fallback comparator.
