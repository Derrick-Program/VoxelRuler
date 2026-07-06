# Play-Session Tracking + Close-to-Minimize Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Record `last_played`/`play_time_secs` around the real launch/exit lifecycle of a Minecraft instance, and make the main window minimize (instead of quitting) when the user clicks its close button while a game is running — quitting for real only once every running instance has exited cleanly, or restoring the window if any of them crash.

**Architecture:** Two small pure-ish methods on `InstanceStore` (`src/mc_instance.rs`) own the "update last_played" / "update play_time_secs" mutations so they're unit-testable without a real child process. `src/view/launch.rs`'s existing launch closure and 2-second exit-poll loop (`setup_launch_logic`) call those methods at the right moments. A new `pending_quit: Arc<AtomicBool>` flag, set by a new `on_close_requested` handler in `src/view/mod.rs`, is threaded into the same exit-poll loop to decide whether an instance's exit should actually quit the app or restore the window.

**Tech Stack:** Rust, Slint UI, `chrono` (already a dependency), `tokio` async tasks, `std::sync::atomic::AtomicBool`.

## Global Constraints

- No new crate dependencies — `slint::Window::on_close_requested`/`set_minimized`/`quit_event_loop` and `chrono::Utc::now().to_rfc3339()` are already available.
- Comment policy: no comments except where a hidden constraint/non-obvious invariant would otherwise confuse a future reader.
- `last_played` is written the moment a launch **succeeds** (not at exit) — matches official-launcher/Prism convention and survives a hard-killed launcher process.
- `play_time_secs` accumulates (`+=`), it is never overwritten/reset by this feature.
- Close-to-minimize triggers only when `running_procs` is non-empty at the moment of the close request; if empty, behavior is unchanged (`CloseRequestResponse::HideWindow`, i.e. normal quit).
- Multi-instance rule: **any** instance crashing while `pending_quit` is set immediately cancels the deferred quit and restores the window, regardless of other still-running instances. The app only actually quits once **every** running instance has exited cleanly.
- The `play_time_secs` write for an exiting session must complete (it's synchronous `std::fs::write` via `save_one`) before the pending-quit quit/restore decision is evaluated for that same exit event.
- No system tray / full-hide — minimize to taskbar/dock only.
- No auto-restore on a clean exit — a clean exit while `pending_quit` is set quits the app entirely instead of restoring the window.

---

### Task 1: `InstanceStore` session-recording methods

**Files:**
- Modify: `src/mc_instance.rs`
- Test: `src/mc_instance.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Produces: `InstanceStore::record_launch_started(&self, master: &mut [InstanceConfig], id: &str)` and `InstanceStore::record_play_session_end(&self, master: &mut [InstanceConfig], id: &str, elapsed_secs: u64)`. Both are no-ops if `id` isn't found in `master`. Both persist via the existing `self.save_one(&InstanceConfig)`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module at the bottom of `src/mc_instance.rs` (after the last existing test, before the closing `}` of `mod tests`):

```rust
    #[test]
    fn test_record_launch_started_sets_last_played() {
        let (store, _dir) = tmp_store();
        let cfg = InstanceConfig {
            id: "a".into(),
            name: "A".into(),
            ..Default::default()
        };
        store.save_one(&cfg).unwrap();
        let mut master = store.load().unwrap();
        assert!(master[0].last_played.is_empty());

        store.record_launch_started(&mut master, "a");

        assert!(!master[0].last_played.is_empty());
        assert!(chrono::DateTime::parse_from_rfc3339(&master[0].last_played).is_ok());

        let reloaded = store.load().unwrap();
        assert_eq!(reloaded[0].last_played, master[0].last_played);
    }

    #[test]
    fn test_record_launch_started_unknown_id_is_noop() {
        let (store, _dir) = tmp_store();
        let mut master: Vec<InstanceConfig> = vec![];
        store.record_launch_started(&mut master, "missing");
        assert!(master.is_empty());
    }

    #[test]
    fn test_record_play_session_end_accumulates_play_time() {
        let (store, _dir) = tmp_store();
        let cfg = InstanceConfig {
            id: "a".into(),
            name: "A".into(),
            play_time_secs: 100,
            ..Default::default()
        };
        store.save_one(&cfg).unwrap();
        let mut master = store.load().unwrap();

        store.record_play_session_end(&mut master, "a", 42);

        assert_eq!(master[0].play_time_secs, 142);
        let reloaded = store.load().unwrap();
        assert_eq!(reloaded[0].play_time_secs, 142);
    }

    #[test]
    fn test_record_play_session_end_unknown_id_is_noop() {
        let (store, _dir) = tmp_store();
        let mut master: Vec<InstanceConfig> = vec![];
        store.record_play_session_end(&mut master, "missing", 42);
        assert!(master.is_empty());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib mc_instance::tests -- --nocapture`
Expected: compile errors — `record_launch_started` and `record_play_session_end` don't exist yet on `InstanceStore`.

- [ ] **Step 3: Implement the two methods**

In `src/mc_instance.rs`, add these two methods to `impl InstanceStore`, directly after `save_one` (i.e. right before `delete_one`):

```rust
    pub fn record_launch_started(&self, master: &mut [InstanceConfig], id: &str) {
        if let Some(c) = master.iter_mut().find(|c| c.id == id) {
            c.last_played = chrono::Utc::now().to_rfc3339();
            let _ = self.save_one(c);
        }
    }

    pub fn record_play_session_end(&self, master: &mut [InstanceConfig], id: &str, elapsed_secs: u64) {
        if let Some(c) = master.iter_mut().find(|c| c.id == id) {
            c.play_time_secs += elapsed_secs;
            let _ = self.save_one(c);
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib mc_instance::tests -- --nocapture`
Expected: PASS (all previous tests plus the 4 new ones)

- [ ] **Step 5: Run `just fmt` and `just lint`, then the full suite**

Run: `just fmt && just lint && cargo test`
Expected: `just fmt` makes no further changes (or only whitespace), `just lint` passes with zero warnings, `cargo test` shows all tests passing (baseline before this task: 161 passed, 0 failed, 6 ignored — expect 165 passed after adding these 4).

- [ ] **Step 6: Commit**

```bash
git add src/mc_instance.rs
git commit -m "$(cat <<'EOF'
feat(mc_instance): add session-recording methods to InstanceStore

record_launch_started sets last_played to now; record_play_session_end
accumulates play_time_secs. Both are no-ops for an unknown id and
persist via the existing save_one. Kept on InstanceStore (rather than
inline in the launch closure) so they're unit-testable without a real
child process.
EOF
)"
```

---

### Task 2: Wire session recording into the launch/exit lifecycle

**Files:**
- Modify: `src/view/launch.rs:493-665` (`setup_launch_logic`, specifically the `on_launch_instance` handler)

**Interfaces:**
- Consumes: `InstanceStore::record_launch_started`/`record_play_session_end` (Task 1). `setup_launch_logic`'s existing `store: Arc<Mutex<InstanceStore>>` and `master_configs: Arc<Mutex<Vec<InstanceConfig>>>` parameters (already present, no signature change in this task).
- Produces: no new public interface — this task only changes the closure body.

- [ ] **Step 1: Add a per-setup clone of `store`**

In `src/view/launch.rs`, change (currently lines 536-540):

```rust
    let master_for_launch = Arc::clone(&master_configs);
    let running_procs_for_launch = Arc::clone(&running_procs);
    let launching_procs_for_launch = Arc::clone(&launching_procs);
    let instance_logs_for_launch = Arc::clone(&instance_logs);
    let ui_weak_for_launch = ui.as_weak();
```

to:

```rust
    let master_for_launch = Arc::clone(&master_configs);
    let store_for_launch = Arc::clone(&store);
    let running_procs_for_launch = Arc::clone(&running_procs);
    let launching_procs_for_launch = Arc::clone(&launching_procs);
    let instance_logs_for_launch = Arc::clone(&instance_logs);
    let ui_weak_for_launch = ui.as_weak();
```

- [ ] **Step 2: Add per-invocation clones inside `on_launch_instance`**

In the same file, change (currently lines 573-577):

```rust
        let instance_id = config.id.clone();
        let running_procs = Arc::clone(&running_procs_for_launch);
        let launching_procs = Arc::clone(&launching_procs_for_launch);
        let ui_weak = ui_weak_for_launch.clone();
        let logs = Arc::clone(&instance_logs_for_launch);
```

to:

```rust
        let instance_id = config.id.clone();
        let running_procs = Arc::clone(&running_procs_for_launch);
        let launching_procs = Arc::clone(&launching_procs_for_launch);
        let ui_weak = ui_weak_for_launch.clone();
        let logs = Arc::clone(&instance_logs_for_launch);
        let master_for_session = Arc::clone(&master_for_launch);
        let store_for_session = Arc::clone(&store_for_launch);
```

- [ ] **Step 3: Record `last_played` the moment the launch succeeds**

In the same file, change (currently lines 606-614):

```rust
                Ok(child) => {
                    running_procs
                        .lock()
                        .unwrap()
                        .insert(instance_id.clone(), child);
                    set_instance_status(&ui_weak, &instance_id, "running");
                    let running_procs_watch = Arc::clone(&running_procs);
                    let ui_weak_watch = ui_weak.clone();
                    let id_watch = instance_id.clone();
```

to:

```rust
                Ok(child) => {
                    let session_start = std::time::Instant::now();
                    {
                        let mut master = master_for_session.lock().unwrap();
                        store_for_session
                            .lock()
                            .unwrap()
                            .record_launch_started(&mut master, &instance_id);
                    }
                    running_procs
                        .lock()
                        .unwrap()
                        .insert(instance_id.clone(), child);
                    set_instance_status(&ui_weak, &instance_id, "running");
                    let running_procs_watch = Arc::clone(&running_procs);
                    let ui_weak_watch = ui_weak.clone();
                    let id_watch = instance_id.clone();
                    let master_for_watch = Arc::clone(&master_for_session);
                    let store_for_watch = Arc::clone(&store_for_session);
```

- [ ] **Step 4: Record `play_time_secs` when the exit-poll loop detects the process has ended**

In the same file, change the exit-poll `match` (currently lines 622-655):

```rust
                            match child.try_wait() {
                                Ok(Some(status)) => {
                                    map.remove(&id_watch);
                                    drop(map);
                                    set_instance_status(&ui_weak_watch, &id_watch, "ready");
                                    if !status.success() {
                                        let lines: Vec<String> = logs_watch
                                            .lock()
                                            .unwrap()
                                            .get(&id_watch)
                                            .map(|d| d.iter().map(|l| l.text.to_string()).collect())
                                            .unwrap_or_default();
                                        let msg = match crate::mc_compat::diagnose_graphics_crash(
                                            lines.iter().map(String::as_str),
                                        ) {
                                            Some(advice) => format!("Game crashed: {advice}"),
                                            None => format!(
                                                "Game exited abnormally ({status}). \
                                                 Check the instance log for details."
                                            ),
                                        };
                                        warn!(instance = %id_watch, "{msg}");
                                        set_install_state(&ui_weak_watch, true, 0.0, &msg, true);
                                    }
                                    break;
                                }
                                Err(_) => {
                                    map.remove(&id_watch);
                                    drop(map);
                                    set_instance_status(&ui_weak_watch, &id_watch, "ready");
                                    break;
                                }
                                Ok(None) => {}
                            }
```

to:

```rust
                            match child.try_wait() {
                                Ok(Some(status)) => {
                                    map.remove(&id_watch);
                                    drop(map);
                                    let elapsed_secs = session_start.elapsed().as_secs();
                                    {
                                        let mut master = master_for_watch.lock().unwrap();
                                        store_for_watch.lock().unwrap().record_play_session_end(
                                            &mut master,
                                            &id_watch,
                                            elapsed_secs,
                                        );
                                    }
                                    set_instance_status(&ui_weak_watch, &id_watch, "ready");
                                    if !status.success() {
                                        let lines: Vec<String> = logs_watch
                                            .lock()
                                            .unwrap()
                                            .get(&id_watch)
                                            .map(|d| d.iter().map(|l| l.text.to_string()).collect())
                                            .unwrap_or_default();
                                        let msg = match crate::mc_compat::diagnose_graphics_crash(
                                            lines.iter().map(String::as_str),
                                        ) {
                                            Some(advice) => format!("Game crashed: {advice}"),
                                            None => format!(
                                                "Game exited abnormally ({status}). \
                                                 Check the instance log for details."
                                            ),
                                        };
                                        warn!(instance = %id_watch, "{msg}");
                                        set_install_state(&ui_weak_watch, true, 0.0, &msg, true);
                                    }
                                    break;
                                }
                                Err(_) => {
                                    map.remove(&id_watch);
                                    drop(map);
                                    let elapsed_secs = session_start.elapsed().as_secs();
                                    {
                                        let mut master = master_for_watch.lock().unwrap();
                                        store_for_watch.lock().unwrap().record_play_session_end(
                                            &mut master,
                                            &id_watch,
                                            elapsed_secs,
                                        );
                                    }
                                    set_instance_status(&ui_weak_watch, &id_watch, "ready");
                                    break;
                                }
                                Ok(None) => {}
                            }
```

**Note:** `std::time::Instant` is `Copy`, so `session_start` moves into the inner `tokio::spawn(async move { ... })` block by value with no `Arc` wrapping needed — it's read-only after creation via `.elapsed()`.

- [ ] **Step 5: Verify it builds**

Run: `cargo check`
Expected: no errors. (No new automated tests in this task — this closure requires a real spawned child process and timing to exercise meaningfully, which is why Task 1 put the testable logic in `InstanceStore` instead. This matches the existing convention in this codebase: `do_launch` and the exit-poll loop have no unit tests today.)

- [ ] **Step 6: Manual verification**

Run: `just run` (or `RUST_BACKTRACE=full RUSTFLAGS="--cfg tokio_unstable" cargo run`)

1. Launch an instance. Immediately check its `instance.toml` on disk (or the Instances page after switching sort mode to "Last Played") — `last_played` should already be a recent RFC3339 timestamp, before the game has even finished loading.
2. Let the game run for at least ~10-15 seconds, then quit Minecraft normally.
3. Confirm the instance's `play_time_secs` increased by roughly the time you had it open (check `instance.toml` or the formatted "Xh Ym" play time shown on its card).
4. Launch it again, quit again — confirm `play_time_secs` accumulated further (didn't reset).

- [ ] **Step 7: Commit**

```bash
git add src/view/launch.rs
git commit -m "$(cat <<'EOF'
feat(launch): record last_played and play_time_secs around a session

last_played is set the instant a launch succeeds; play_time_secs
accumulates elapsed wall-clock time once the exit-poll loop detects
the process has ended (success or error).
EOF
)"
```

---

### Task 3: Close-to-minimize with deferred quit

**Files:**
- Modify: `src/view/mod.rs` (new `pending_quit` state + `on_close_requested` handler + updated `setup_launch_logic` call)
- Modify: `src/view/launch.rs` (new `pending_quit` parameter threaded into the exit-poll loop)

**Interfaces:**
- Consumes: `setup_launch_logic`'s existing parameter list (Task 2 left it unchanged); `running_procs: Arc<Mutex<HashMap<String, Child>>>` (existing).
- Produces: `setup_launch_logic` gains a new required parameter `pending_quit: std::sync::Arc<std::sync::atomic::AtomicBool>` — any other caller of this function must be updated too, but `src/view/mod.rs`'s call site is the only caller in the codebase.

- [ ] **Step 1: Add the `pending_quit` flag and the close-request handler in `src/view/mod.rs`**

In `src/view/mod.rs`, change (currently lines 185-190):

```rust
    let running_procs: Arc<Mutex<HashMap<String, Child>>> = Arc::new(Mutex::new(HashMap::new()));
    let launching_procs: Arc<Mutex<std::collections::HashSet<String>>> =
        Arc::new(Mutex::new(std::collections::HashSet::new()));
    let instance_logs: Arc<Mutex<HashMap<String, VecDeque<crate::view::LogLine>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let (_debouncer, rx) = store.lock().unwrap().watch_changes()?;
```

to:

```rust
    let running_procs: Arc<Mutex<HashMap<String, Child>>> = Arc::new(Mutex::new(HashMap::new()));
    let launching_procs: Arc<Mutex<std::collections::HashSet<String>>> =
        Arc::new(Mutex::new(std::collections::HashSet::new()));
    let instance_logs: Arc<Mutex<HashMap<String, VecDeque<crate::view::LogLine>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let pending_quit: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    let running_procs_for_close = Arc::clone(&running_procs);
    let pending_quit_for_close = Arc::clone(&pending_quit);
    let ui_weak_for_close = ui.as_weak();
    ui.window().on_close_requested(move || {
        if running_procs_for_close.lock().unwrap().is_empty() {
            return slint::CloseRequestResponse::HideWindow;
        }
        pending_quit_for_close.store(true, Ordering::SeqCst);
        if let Some(ui) = ui_weak_for_close.upgrade() {
            ui.window().set_minimized(true);
        }
        slint::CloseRequestResponse::KeepWindowShown
    });

    let (_debouncer, rx) = store.lock().unwrap().watch_changes()?;
```

- [ ] **Step 2: Pass `pending_quit` into `setup_launch_logic`**

In the same file, change (currently lines 499-506):

```rust
    launch::setup_launch_logic(
        &ui,
        Arc::clone(&store),
        Arc::clone(&master_configs),
        Arc::clone(&running_procs),
        Arc::clone(&launching_procs),
        Arc::clone(&instance_logs),
    );
```

to:

```rust
    launch::setup_launch_logic(
        &ui,
        Arc::clone(&store),
        Arc::clone(&master_configs),
        Arc::clone(&running_procs),
        Arc::clone(&launching_procs),
        Arc::clone(&instance_logs),
        Arc::clone(&pending_quit),
    );
```

- [ ] **Step 3: Verify `src/view/mod.rs` compiles up to this point**

Run: `cargo check 2>&1 | grep -A5 "setup_launch_logic"`
Expected: an error that `setup_launch_logic` takes 6 arguments but 7 were supplied (or similar) — confirms the call site is ahead of the not-yet-updated function signature. This is expected; proceed to the next step.

- [ ] **Step 4: Add the `pending_quit` parameter to `setup_launch_logic`'s signature in `src/view/launch.rs`**

In `src/view/launch.rs`, change the top of `use` block (currently line 16):

```rust
use std::sync::{Arc, Mutex};
```

to:

```rust
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
```

Then change the function signature (currently lines 493-506, already includes Task 2's unchanged parameters):

```rust
pub fn setup_launch_logic(
    ui: &MainApp,
    store: std::sync::Arc<std::sync::Mutex<crate::mc_instance::InstanceStore>>,
    master_configs: std::sync::Arc<std::sync::Mutex<Vec<crate::mc_instance::InstanceConfig>>>,
    running_procs: std::sync::Arc<
        std::sync::Mutex<std::collections::HashMap<String, std::process::Child>>,
    >,
    launching_procs: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    instance_logs: std::sync::Arc<
        std::sync::Mutex<
            std::collections::HashMap<String, std::collections::VecDeque<crate::view::LogLine>>,
        >,
    >,
) {
```

to:

```rust
pub fn setup_launch_logic(
    ui: &MainApp,
    store: std::sync::Arc<std::sync::Mutex<crate::mc_instance::InstanceStore>>,
    master_configs: std::sync::Arc<std::sync::Mutex<Vec<crate::mc_instance::InstanceConfig>>>,
    running_procs: std::sync::Arc<
        std::sync::Mutex<std::collections::HashMap<String, std::process::Child>>,
    >,
    launching_procs: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    instance_logs: std::sync::Arc<
        std::sync::Mutex<
            std::collections::HashMap<String, std::collections::VecDeque<crate::view::LogLine>>,
        >,
    >,
    pending_quit: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
```

- [ ] **Step 5: Verify it compiles again**

Run: `cargo check`
Expected: no errors now (the call site and signature match).

- [ ] **Step 6: Thread `pending_quit` through to the exit-poll loop**

In the same file, change (this is Task 2's Step 1 result, currently):

```rust
    let master_for_launch = Arc::clone(&master_configs);
    let store_for_launch = Arc::clone(&store);
    let running_procs_for_launch = Arc::clone(&running_procs);
    let launching_procs_for_launch = Arc::clone(&launching_procs);
    let instance_logs_for_launch = Arc::clone(&instance_logs);
    let ui_weak_for_launch = ui.as_weak();
```

to:

```rust
    let master_for_launch = Arc::clone(&master_configs);
    let store_for_launch = Arc::clone(&store);
    let running_procs_for_launch = Arc::clone(&running_procs);
    let launching_procs_for_launch = Arc::clone(&launching_procs);
    let instance_logs_for_launch = Arc::clone(&instance_logs);
    let pending_quit_for_launch = Arc::clone(&pending_quit);
    let ui_weak_for_launch = ui.as_weak();
```

Then change (this is Task 2's Step 2 result, currently):

```rust
        let instance_id = config.id.clone();
        let running_procs = Arc::clone(&running_procs_for_launch);
        let launching_procs = Arc::clone(&launching_procs_for_launch);
        let ui_weak = ui_weak_for_launch.clone();
        let logs = Arc::clone(&instance_logs_for_launch);
        let master_for_session = Arc::clone(&master_for_launch);
        let store_for_session = Arc::clone(&store_for_launch);
```

to:

```rust
        let instance_id = config.id.clone();
        let running_procs = Arc::clone(&running_procs_for_launch);
        let launching_procs = Arc::clone(&launching_procs_for_launch);
        let ui_weak = ui_weak_for_launch.clone();
        let logs = Arc::clone(&instance_logs_for_launch);
        let master_for_session = Arc::clone(&master_for_launch);
        let store_for_session = Arc::clone(&store_for_launch);
        let pending_quit = Arc::clone(&pending_quit_for_launch);
```

Then change (this is Task 2's Step 3 result, currently):

```rust
                Ok(child) => {
                    let session_start = std::time::Instant::now();
                    {
                        let mut master = master_for_session.lock().unwrap();
                        store_for_session
                            .lock()
                            .unwrap()
                            .record_launch_started(&mut master, &instance_id);
                    }
                    running_procs
                        .lock()
                        .unwrap()
                        .insert(instance_id.clone(), child);
                    set_instance_status(&ui_weak, &instance_id, "running");
                    let running_procs_watch = Arc::clone(&running_procs);
                    let ui_weak_watch = ui_weak.clone();
                    let id_watch = instance_id.clone();
                    let master_for_watch = Arc::clone(&master_for_session);
                    let store_for_watch = Arc::clone(&store_for_session);
```

to:

```rust
                Ok(child) => {
                    let session_start = std::time::Instant::now();
                    {
                        let mut master = master_for_session.lock().unwrap();
                        store_for_session
                            .lock()
                            .unwrap()
                            .record_launch_started(&mut master, &instance_id);
                    }
                    running_procs
                        .lock()
                        .unwrap()
                        .insert(instance_id.clone(), child);
                    set_instance_status(&ui_weak, &instance_id, "running");
                    let running_procs_watch = Arc::clone(&running_procs);
                    let ui_weak_watch = ui_weak.clone();
                    let id_watch = instance_id.clone();
                    let master_for_watch = Arc::clone(&master_for_session);
                    let store_for_watch = Arc::clone(&store_for_session);
                    let pending_quit_watch = Arc::clone(&pending_quit);
```

- [ ] **Step 7: Decide quit vs. restore in the exit-poll loop**

In the same file, change (this is Task 2's Step 4 result, currently):

```rust
                            match child.try_wait() {
                                Ok(Some(status)) => {
                                    map.remove(&id_watch);
                                    drop(map);
                                    let elapsed_secs = session_start.elapsed().as_secs();
                                    {
                                        let mut master = master_for_watch.lock().unwrap();
                                        store_for_watch.lock().unwrap().record_play_session_end(
                                            &mut master,
                                            &id_watch,
                                            elapsed_secs,
                                        );
                                    }
                                    set_instance_status(&ui_weak_watch, &id_watch, "ready");
                                    if !status.success() {
                                        let lines: Vec<String> = logs_watch
                                            .lock()
                                            .unwrap()
                                            .get(&id_watch)
                                            .map(|d| d.iter().map(|l| l.text.to_string()).collect())
                                            .unwrap_or_default();
                                        let msg = match crate::mc_compat::diagnose_graphics_crash(
                                            lines.iter().map(String::as_str),
                                        ) {
                                            Some(advice) => format!("Game crashed: {advice}"),
                                            None => format!(
                                                "Game exited abnormally ({status}). \
                                                 Check the instance log for details."
                                            ),
                                        };
                                        warn!(instance = %id_watch, "{msg}");
                                        set_install_state(&ui_weak_watch, true, 0.0, &msg, true);
                                    }
                                    break;
                                }
                                Err(_) => {
                                    map.remove(&id_watch);
                                    drop(map);
                                    let elapsed_secs = session_start.elapsed().as_secs();
                                    {
                                        let mut master = master_for_watch.lock().unwrap();
                                        store_for_watch.lock().unwrap().record_play_session_end(
                                            &mut master,
                                            &id_watch,
                                            elapsed_secs,
                                        );
                                    }
                                    set_instance_status(&ui_weak_watch, &id_watch, "ready");
                                    break;
                                }
                                Ok(None) => {}
                            }
```

to:

```rust
                            match child.try_wait() {
                                Ok(Some(status)) => {
                                    map.remove(&id_watch);
                                    drop(map);
                                    let elapsed_secs = session_start.elapsed().as_secs();
                                    {
                                        let mut master = master_for_watch.lock().unwrap();
                                        store_for_watch.lock().unwrap().record_play_session_end(
                                            &mut master,
                                            &id_watch,
                                            elapsed_secs,
                                        );
                                    }
                                    set_instance_status(&ui_weak_watch, &id_watch, "ready");
                                    if status.success() {
                                        if pending_quit_watch.load(Ordering::SeqCst)
                                            && running_procs_watch.lock().unwrap().is_empty()
                                        {
                                            let _ = slint::quit_event_loop();
                                        }
                                    } else {
                                        let lines: Vec<String> = logs_watch
                                            .lock()
                                            .unwrap()
                                            .get(&id_watch)
                                            .map(|d| d.iter().map(|l| l.text.to_string()).collect())
                                            .unwrap_or_default();
                                        let msg = match crate::mc_compat::diagnose_graphics_crash(
                                            lines.iter().map(String::as_str),
                                        ) {
                                            Some(advice) => format!("Game crashed: {advice}"),
                                            None => format!(
                                                "Game exited abnormally ({status}). \
                                                 Check the instance log for details."
                                            ),
                                        };
                                        warn!(instance = %id_watch, "{msg}");
                                        set_install_state(&ui_weak_watch, true, 0.0, &msg, true);
                                        if pending_quit_watch.swap(false, Ordering::SeqCst) {
                                            let ui_weak_restore = ui_weak_watch.clone();
                                            let _ = slint::invoke_from_event_loop(move || {
                                                if let Some(ui) = ui_weak_restore.upgrade() {
                                                    ui.window().set_minimized(false);
                                                }
                                            });
                                        }
                                    }
                                    break;
                                }
                                Err(_) => {
                                    map.remove(&id_watch);
                                    drop(map);
                                    let elapsed_secs = session_start.elapsed().as_secs();
                                    {
                                        let mut master = master_for_watch.lock().unwrap();
                                        store_for_watch.lock().unwrap().record_play_session_end(
                                            &mut master,
                                            &id_watch,
                                            elapsed_secs,
                                        );
                                    }
                                    set_instance_status(&ui_weak_watch, &id_watch, "ready");
                                    if pending_quit_watch.swap(false, Ordering::SeqCst) {
                                        let ui_weak_restore = ui_weak_watch.clone();
                                        let _ = slint::invoke_from_event_loop(move || {
                                            if let Some(ui) = ui_weak_restore.upgrade() {
                                                ui.window().set_minimized(false);
                                            }
                                        });
                                    }
                                    break;
                                }
                                Ok(None) => {}
                            }
```

**Why `load` for the success case but `swap` for the two abnormal cases:** on a clean exit while quitting, the flag doesn't need resetting (the whole process is about to quit). On an abnormal exit, `swap(false, ...)` atomically reads-and-clears in one step, so a second crash later doesn't try to restore an already-visible window from a stale `true`.

- [ ] **Step 8: Verify it builds, then run `just fmt` and `just lint`**

Run: `cargo check && just fmt && just lint`
Expected: no errors, no lint warnings. Re-run `git diff --stat` after `just fmt` to confirm only whitespace/formatting changed, if anything.

- [ ] **Step 9: Run the full test suite**

Run: `cargo test`
Expected: PASS, same count as after Task 1/2 (this task adds no new automated tests — window close/minimize/quit behavior isn't unit-testable in this codebase's existing pattern, same reasoning as Task 2).

- [ ] **Step 10: Manual verification**

Run: `just run`

1. With no instance running, click the window's close (X) button — confirm the app quits normally (unchanged from today).
2. Launch an instance, wait for it to reach "running" status, then click the window's close (X) button — confirm the window **minimizes** to the taskbar/dock instead of closing, and the app process is still alive (check via `ps`/Activity Monitor/Task Manager that the game process and the launcher process are both still running).
3. Let that game exit **normally** (quit Minecraft from its own menu) — confirm the whole VoxelRuler process actually terminates shortly after (the deferred quit fires).
4. Repeat step 2, but this time force the game to crash (or kill its process externally, e.g. `kill -9 <minecraft-pid>`) — confirm the window **restores** (un-minimizes) and shows the existing crash-diagnosis popup, and the launcher does **not** quit.
5. If feasible, launch two instances at once, close the window, let one crash while the other is still running — confirm the window restores immediately on the crash (doesn't wait for the second instance), and the app does not quit even after the second instance later exits cleanly (since the deferred quit was already cancelled by the crash).

- [ ] **Step 11: Commit**

```bash
git add src/view/mod.rs src/view/launch.rs
git commit -m "$(cat <<'EOF'
feat(view): minimize instead of closing while a game is running

Clicking the window's close button while any instance is running now
minimizes the window and defers the quit until every running instance
has exited. A clean exit (once nothing else is running) actually
quits the app; a crash cancels the deferred quit and restores the
window instead, so the existing crash-diagnosis popup is visible.
EOF
)"
```

---

## Self-Review Notes

- **Spec coverage:** Part A (last_played at launch, play_time_secs at exit, no new deps, save-then-let-watcher-refresh) → Tasks 1-2. Part B (pending_quit, on_close_requested, quit-only-when-all-clean, restore-on-any-crash, no tray dependency) → Task 3. The spec's ordering note (play_time write must complete before the pending-quit decision) is satisfied by construction: Task 3's Step 7 diff places the `record_play_session_end` block textually and temporally before the `if status.success() { ... }` / `else { ... }` block in both branches.
- **Type consistency:** `InstanceStore::record_launch_started`/`record_play_session_end` signatures (Task 1) are called identically in Task 2/3's `launch.rs` edits (`&mut master, &instance_id` / `&mut master, &id_watch, elapsed_secs`). `setup_launch_logic`'s parameter list gains exactly one new trailing parameter (`pending_quit`) between Task 2 (unchanged signature) and Task 3 (adds it) — the call site in `src/view/mod.rs` and the function definition in `src/view/launch.rs` are updated in the same task (Task 3), so there's no intermediate non-compiling state left across a task boundary.
- **Placeholder scan:** no TBD/TODO; every step shows complete before/after code.
