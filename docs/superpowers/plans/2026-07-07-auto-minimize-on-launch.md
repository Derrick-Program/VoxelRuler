# Auto-Minimize Main Window on Instance Launch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The moment an instance's Minecraft process successfully spawns, automatically minimize the main window — no user action required.

**Architecture:** A single call added at the existing "instance is now running" bookkeeping site in `do_launch`'s success path (`src/view/launch.rs`), marshaled onto the Slint event loop the same way the neighboring `set_instance_status` helper and the existing restore-on-crash calls already do.

**Tech Stack:** Rust, Slint (`slint::invoke_from_event_loop`, `Window::set_minimized`).

## Global Constraints

- Trigger point: the moment the Minecraft process successfully spawns (i.e. `do_launch` returns `Ok(child)`), not during the earlier install/download phase.
- On game exit (clean or crashed), the window is left minimized — no auto-restore. This is a deliberate, confirmed decision; do not add restore behavior.
- No new user-facing setting/toggle. The existing minimize-while-running feature has none either.
- Do not modify `on_close_requested` (`src/view/mod.rs:193-202`) or the crash-restore-while-pending-quit branches (`src/view/launch.rs`, the two `set_minimized(false)` call sites) — both are orthogonal and already correct.
- UI mutations from this background `tokio::spawn` context must go through `slint::invoke_from_event_loop` — touching `ui.window()` directly from this task without it would be unsound, matching why `set_instance_status` (`src/view/mod.rs:589-613`) and the existing `set_minimized(false)` restore calls already wrap themselves in it.

---

### Task 1: Minimize the main window when an instance launch succeeds

**Files:**
- Modify: `src/view/launch.rs:628` (inside `setup_launch_logic`'s `on_launch_instance` handler, the `Ok(child) => { ... }` branch)

**Interfaces:**
- Consumes: `ui_weak: slint::Weak<MainApp>` (already bound in this closure, cloned at the top of `on_launch_instance` as `let ui_weak = ui_weak_for_launch.clone();`), `slint::invoke_from_event_loop`, `ui.window().set_minimized(bool)` (both already used elsewhere in this same file).
- Produces: nothing new consumed by later tasks — this plan has only one task.

- [ ] **Step 1: Add the minimize call right after the existing "running" status update**

In `src/view/launch.rs`, inside the `Ok(child) => { ... }` branch of the `match res` in `on_launch_instance`'s spawned task, find:

```rust
                    running_procs
                        .lock()
                        .unwrap()
                        .insert(instance_id.clone(), child);
                    set_instance_status(&ui_weak, &instance_id, "running");
```

and change it to:

```rust
                    running_procs
                        .lock()
                        .unwrap()
                        .insert(instance_id.clone(), child);
                    set_instance_status(&ui_weak, &instance_id, "running");
                    {
                        let ui_weak_minimize = ui_weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_minimize.upgrade() {
                                ui.window().set_minimized(true);
                            }
                        });
                    }
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: succeeds, no errors.

- [ ] **Step 3: Run the full test suite to check for regressions**

Run: `cargo test`
Expected: all tests pass (this change has no unit-test seam of its own — `set_minimized` is an OS-level window-state side effect inside a background task with no observable return value or state to assert on outside a running UI event loop, consistent with the neighboring untested `set_minimized(false)` restore calls in the same function). This step only guards against an unrelated regression.

- [ ] **Step 4: Manual verification**

Documented rather than automated, since this requires a running graphical session and a real (or at least installable) Minecraft instance:
1. Run `cargo run`.
2. Launch any instance (an already-installed one launches fastest, skipping the download wait).
3. Once the game process starts (the instance's status flips to "running" in the instance list), confirm the main VoxelRuler window minimizes automatically — without clicking close or doing anything else.
4. Quit the game (or let it exit normally).
5. Confirm the main window stays minimized — it should NOT automatically pop back up. Bring it back manually via the OS taskbar/dock and confirm it's in a normal, usable state.
6. Separately, re-confirm the pre-existing behavior is untouched: with a game running, click the window's close button — the window should minimize via the existing `on_close_requested` path (unchanged), and the app should not quit while the game is still running.

- [ ] **Step 5: Commit**

```bash
git add src/view/launch.rs
git commit -m "feat(view): auto-minimize main window when an instance launch succeeds"
```
