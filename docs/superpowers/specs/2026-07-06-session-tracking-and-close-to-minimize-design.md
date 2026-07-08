# Play-Session Tracking + Close-to-Minimize Design

**Date:** 2026-07-06
**Status:** Approved

## Problem

Two related gaps in the launch lifecycle:

1. `InstanceConfig.last_played` and `.play_time_secs` (`src/mc_instance.rs:26-27`) are defined and read (used for sorting and UI display) but never actually written except when explicitly reset to empty/0 on instance duplication (`src/view/launch.rs:840-841`). No code updates them when a game is actually played.
2. The main window has no `on_close_requested` handling — clicking the window's close ("X") button always quits the app immediately (default Slint behavior), even while a Minecraft game process spawned by the launcher is still running. Since child processes are not detached from the launcher process (verified: no `kill_on_drop`/process-group flags anywhere in `src/view/launch.rs`), quitting the launcher while a game is running risks killing the game too.

Both share the same underlying state: `running_procs: Arc<Mutex<HashMap<String, Child>>>` and the per-instance exit-watch loop already in `src/view/launch.rs:615-657`. **Ordering note:** at the exit-detection point in that loop, Part A's `play_time_secs` write must happen and complete (it's a synchronous `std::fs::write` via `save_one`, not async) *before* Part B's pending-quit check runs — so a deferred quit never races ahead of persisting the session's play time.

## Part A: Last Played + Play Time tracking

**When `last_played` is written:** the moment a launch succeeds — inside `on_launch_instance`'s `Ok(child) => { ... }` branch (`src/view/launch.rs:606`), before spawning the exit-watch loop. This matches the official launcher / Prism convention (record as soon as the session starts, not when it ends), so a hard-killed launcher process doesn't lose the record of "you played this."

```rust
Ok(child) => {
    let session_start = std::time::Instant::now();
    {
        let mut master = master_for_launch_watch.lock().unwrap();
        if let Some(c) = master.iter_mut().find(|c| c.id == instance_id) {
            c.last_played = chrono::Utc::now().to_rfc3339();
            let _ = store_for_launch_watch.lock().unwrap().save_one(c);
        }
    }
    running_procs.lock().unwrap().insert(instance_id.clone(), child);
    // ...existing set_instance_status / exit-watch spawn...
}
```

**When `play_time_secs` is written:** inside the existing 2-second-poll exit-watch loop (`src/view/launch.rs:615-657`), at the point the loop currently detects exit and removes the entry from `running_procs` — both the `Ok(Some(status))` branch and the `Err(_)` branch (either way, the session has ended from the launcher's perspective):

```rust
let elapsed_secs = session_start.elapsed().as_secs();
let mut master = master_for_launch_watch.lock().unwrap();
if let Some(c) = master.iter_mut().find(|c| c.id == id_watch) {
    c.play_time_secs += elapsed_secs;
    let _ = store_for_launch_watch.lock().unwrap().save_one(c);
}
```

**Persistence / UI refresh:** `store.save_one()` writes `instance.toml`, which the existing file-watcher (`src/mc_instance.rs`'s `watch_changes` + the debounced refresh loop in `src/view/mod.rs`) already picks up and uses to refresh `master_configs` and the Instances page — no manual `refresh_instance_list()` call needed from `launch.rs`, matching the existing pattern already used by `on_save_version` in `src/view/instance_detail.rs:482-507`.

**No new dependencies:** `chrono` is already a dependency (`Cargo.toml:36`, already used elsewhere via `chrono::Utc::now()` for token expiry) and already produces the RFC3339 format the existing test fixtures and `SortMode::LastPlayed` string comparison assume (e.g. `"2024-01-01T00:00:00Z"` in `src/mc_instance.rs:287`).

## Part B: Close-to-minimize with deferred quit

**New shared state:** `pending_quit: Arc<AtomicBool>`, created in `open_view()` (`src/view/mod.rs`) alongside `running_procs`, initialized `false`.

**Close-request handler**, registered once in `open_view()`:

```rust
let running_procs_for_close = Arc::clone(&running_procs);
let pending_quit_for_close = Arc::clone(&pending_quit);
let ui_weak_for_close = ui.as_weak();
ui.window().on_close_requested(move || {
    if running_procs_for_close.lock().unwrap().is_empty() {
        return slint::CloseRequestResponse::HideWindow;
    }
    pending_quit_for_close.store(true, std::sync::atomic::Ordering::SeqCst);
    if let Some(ui) = ui_weak_for_close.upgrade() {
        ui.window().set_minimized(true);
    }
    slint::CloseRequestResponse::KeepWindowShown
});
```

- No game running → `HideWindow` (identical to today's default: the app closes/quits normally).
- Game(s) running → mark `pending_quit`, minimize instead of closing, `KeepWindowShown` prevents the actual hide/quit.

**`setup_launch_logic` gains a new `pending_quit: Arc<AtomicBool>` parameter**, threaded into the exit-watch loop (`src/view/launch.rs:615-657`). At the point the loop detects exit and removes the instance from `running_procs`:

- **Clean exit** (`status.success()`) **and** `pending_quit` is true **and** `running_procs` is now empty (this was the last running instance) → call `slint::quit_event_loop()`. This is thread-safe and callable from any thread per Slint's docs, so no `invoke_from_event_loop` marshaling needed for this call specifically.
- **Abnormal exit** (`!status.success()`, or the `Err(_)` branch from `try_wait`) **and** `pending_quit` is true → reset `pending_quit` to `false`, and restore the window via `invoke_from_event_loop` (matching the existing marshaling pattern used by `set_instance_status`/`set_install_state` in `src/view/mod.rs:554-595`):
  ```rust
  let ui_weak_restore = ui_weak_watch.clone();
  let _ = slint::invoke_from_event_loop(move || {
      if let Some(ui) = ui_weak_restore.upgrade() {
          ui.window().set_minimized(false);
      }
  });
  ```
  The existing crash-diagnosis popup (`set_install_state(&ui_weak_watch, true, 0.0, &msg, true)`, already present at `src/view/launch.rs:644`) fires in the same branch, so the user sees both the restored window and the diagnostic message together.
- **Other instances still running** (`running_procs` non-empty after removal) → do nothing further; `pending_quit` stays `true`, the loop for the still-running instance(s) will make the same check when *they* exit.

**Multi-instance rule** (explicitly confirmed): any single instance crashing while `pending_quit` is set immediately cancels the deferred quit and restores the window — it does not wait for other running instances to finish first. Only when *every* running instance has exited cleanly does the app actually quit.

**No new dependencies:** `on_close_requested`, `CloseRequestResponse::{HideWindow, KeepWindowShown}`, `Window::set_minimized`, and `slint::quit_event_loop()` are all part of the `slint` crate already in use (verified via Slint's own Rust API docs).

## Testing

- `mc_instance.rs` / `launch.rs` unit tests: none of this logic is easily unit-testable without a running child process and a live Slint window — consistent with this codebase's existing convention that launch/process-lifecycle code (`do_launch`, the exit-watch loop) has no unit tests today.
- Manual verification (documented in the implementation plan): launch an instance, confirm `last_played` updates immediately in the instance TOML / UI; let it exit normally, confirm `play_time_secs` increased by roughly the elapsed wall-clock time; click the window's X while a game is running, confirm it minimizes instead of closing; let that game exit normally, confirm the app actually quits; repeat but force the game to crash (or kill its process externally) and confirm the window restores with the crash-diagnosis popup instead of quitting.

## Out of scope

- Any behavior change while an instance is only *launching/installing* (not yet running) — the close-to-minimize behavior is scoped strictly to `running_procs` being non-empty, per explicit confirmation. Closing during install/download is unaffected by this design.
- System tray icon / full hide — explicitly declined in favor of plain taskbar/dock minimize, to avoid a new `tray-icon` dependency.
- Auto-restoring the window on a *clean* exit — explicitly declined; a clean exit while `pending_quit` is set quits the app entirely instead.
