# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build

# Build (release)
cargo build --release

# Run (requires Microsoft OAuth client_id as first argument)
cargo run -- <client_id>

# Check for compile errors without building
cargo check

# Run tests
cargo test

# Run a single test
cargo test <test_name>

# Lint
cargo clippy
```

## Architecture

VoxelRuler is a Minecraft launcher built with Rust + [Slint](https://slint.dev/) for the GUI. The UI is defined entirely in `.slint` files compiled at build time via `build.rs`.

### Build System

`build.rs` compiles `ui/main.slint` with three registered library aliases:
- `@material` → `ui/libs/material-1.0/material.slint` (Material Design component library)
- `@theme` → `ui/theme.slint` (global `AppTheme` singleton)
- `@assets` → `ui/assets/index.slint` (global `Assets` singleton for images)

Translations are bundled from `ui/translations/` (supports `zh_TW` and `en_US`). Use `@tr("...")` in `.slint` files for translatable strings.

### UI Layer (`ui/`)

```
ui/
├── main.slint          # Root window (MainApp), composes SideBar + Pages + Footer
├── theme.slint         # AppTheme global: colors, radius, font sizes, dark/light toggle
├── assets/index.slint  # Assets global: all image references
├── components/
│   ├── index.slint     # Re-exports SideBar, Footer, Pages
│   ├── footer.slint    # Status bar at the bottom
│   ├── sidebar/        # SideBar with SideTabId enum, SideButton, Avatar
│   └── pages/          # Pages component that conditionally renders tabs
│       ├── index.slint # Routes SideTabId to the correct page component
│       ├── home.slint
│       └── instances.slint  # InstanceCard grid, Search, AddInstance
└── libs/material-1.0/  # Third-party Material Design component library
```

The `SideTabId` enum (`home`, `instances`, `mods`, `downloads`, `account`, `settings`) drives navigation. Pages for `mods`, `downloads`, `account`, `settings` currently show a WIP placeholder.

### Rust Layer (`src/main.rs`)

Currently the `main.rs` is wired for Microsoft OAuth / Minecraft authentication flow (not the Slint UI). The commented-out block at the top shows the intended Slint entrypoint:

```rust
slint::include_modules!();  // Imports generated types from .slint files
let app = MainApp::new()?;
slint::select_bundled_translation("zh_TW").unwrap();
app.run()?;
```

### Theme System

`AppTheme` in `ui/theme.slint` is a global singleton. Toggle dark/light mode via `AppTheme.is-dark`. Colors are accessed as `AppTheme.colors.<token>` (e.g., `AppTheme.colors.sidebar`, `AppTheme.colors.background`). Minecraft-themed color constants (`mc-green`, `mc-grass`, `mc-dirt`, `mc-stone`, `mc-diamond`) are also available.

### Adding New Icons

1. Add the SVG to `ui/assets/icons/`
2. Register it in `ui/assets/index.slint` as an `out property <image>`
3. Use it in `.slint` files via `Assets.<name>`
