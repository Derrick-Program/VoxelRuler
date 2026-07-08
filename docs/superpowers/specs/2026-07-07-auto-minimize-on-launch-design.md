# Auto-Minimize Main Window on Instance Launch

Date: 2026-07-07
Status: Approved

## Problem

Today the main window only minimizes as a side effect of the user trying to *close* it while a game is running (`on_close_requested` in `src/view/mod.rs:193-202`, added by the prior "minimize instead of closing while a game is running" feature). There is no behavior tied to the moment a game actually *launches* — the launcher window just sits there, still in front, while Minecraft starts up and takes over.

## Behavior

The moment an instance's Minecraft process successfully spawns, the main window minimizes automatically — the user doesn't have to click close or do anything themselves. When the game later exits (cleanly or by crashing), the window is left minimized; it is not automatically restored. The user brings it back manually via the OS taskbar/dock, exactly as they already do today after the existing close-while-running minimize.

No new user-facing setting is introduced. The existing minimize-while-running feature has no toggle either, so this stays consistent with that precedent (YAGNI).

## Where This Hooks In

`src/view/launch.rs`, inside `setup_launch_logic`'s `on_launch_instance` handler, in the `tokio::spawn` block that awaits `do_launch(...)`. Specifically the `Ok(child) => { ... }` branch (currently starting at line 619), which already does the "instance is now running" bookkeeping: recording the launch start, inserting into `running_procs`, and calling `set_instance_status(&ui_weak, &instance_id, "running")` (line 632). This is the single call site for launching an instance — `on_launch_instance` is the only place `do_launch` is invoked.

The fix adds one call — `ui.window().set_minimized(true)` — right alongside that existing bookkeeping, gated the same way those other calls are (via `ui_weak.upgrade()`).

## Explicitly Not Changed

- **`on_close_requested`** (`src/view/mod.rs:193-202`): the existing "minimize instead of quit while running" behavior is untouched. It's a different trigger (user-initiated close) and keeps working exactly as it does today.
- **Crash-restore special case** (`src/view/launch.rs:686-693` and the analogous branch at `:710-717`): when a game exits abnormally *and* `pending_quit` was set (i.e. the user had tried to quit while it was running), the window is restored so the user can see the crash/error message. This is orthogonal to the new behavior — it stays as-is. The new auto-minimize-on-launch does not set `pending_quit`, so it does not interact with this path.
- No changes to `running_procs`/`launching_procs` concurrency handling. If a second instance launches while a first is already running, `set_minimized(true)` fires again — a harmless no-op on an already-minimized window.
- No auto-restore on any exit path (clean or crashed, absent the existing `pending_quit` case above) — confirmed explicitly with the project owner.

## Testing

This is a single OS-window-state side effect triggered inside an async spawn block that also does process management, logging, and status updates — there's no meaningful unit-test seam for "did the window minimize" (Slint's `set_minimized` has no observable return value or state we can assert on outside a running UI event loop). Consistent with how the neighboring `set_minimized(false)` restore calls in the same function are untested today. Verification is manual: launch an instance, confirm the main window minimizes automatically once the game process starts (no need to touch the close button), then quit the game and confirm the window stays minimized until manually restored from the taskbar/dock.

## Out of Scope

- No settings/toggle to disable this behavior.
- No change to what happens on game exit (still no auto-restore, matching the approved design).
- No change to the existing close-while-running or crash-restore behaviors.
