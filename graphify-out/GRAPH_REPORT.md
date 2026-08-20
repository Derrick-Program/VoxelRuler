# Graph Report - .  (2026-08-20)

## Corpus Check
- 255 files · ~317,265 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1033 nodes · 2258 edges · 61 communities (37 shown, 24 thin omitted)
- Extraction: 97% EXTRACTED · 3% INFERRED · 0% AMBIGUOUS · INFERRED: 67 edges (avg confidence: 0.81)
- Token cost: 42,944 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Mod Loader Metadata (FabricForge)|Mod Loader Metadata (Fabric/Forge)]]
- [[_COMMUNITY_Dev Standards & Feature Specs|Dev Standards & Feature Specs]]
- [[_COMMUNITY_Create Instance Dialog Agent Plan|Create Instance Dialog Agent Plan]]
- [[_COMMUNITY_Instance Assets & NBT Utils|Instance Assets & NBT Utils]]
- [[_COMMUNITY_Launch Command Builder|Launch Command Builder]]
- [[_COMMUNITY_Caveman-Compress Benchmark & CLI|Caveman-Compress Benchmark & CLI]]
- [[_COMMUNITY_Instance Detail Mod Loader UI|Instance Detail Mod Loader UI]]
- [[_COMMUNITY_Minecraft Version JSON Schema|Minecraft Version JSON Schema]]
- [[_COMMUNITY_Mojang API Client|Mojang API Client]]
- [[_COMMUNITY_SkinCape Upload Logic|Skin/Cape Upload Logic]]
- [[_COMMUNITY_Install Pipeline (DownloadVerify)|Install Pipeline (Download/Verify)]]
- [[_COMMUNITY_Data Directory Layout|Data Directory Layout]]
- [[_COMMUNITY_Apple Silicon Compat Fixes|Apple Silicon Compat Fixes]]
- [[_COMMUNITY_TODO.md Milestone Tracker|TODO.md Milestone Tracker]]
- [[_COMMUNITY_3D Skin Preview Renderer|3D Skin Preview Renderer]]
- [[_COMMUNITY_Instance Creation Logic|Instance Creation Logic]]
- [[_COMMUNITY_App Settings Persistence|App Settings Persistence]]
- [[_COMMUNITY_Java Runtime Resolution|Java Runtime Resolution]]
- [[_COMMUNITY_Skin History Store|Skin History Store]]
- [[_COMMUNITY_UI Wiring Entrypoint|UI Wiring Entrypoint]]
- [[_COMMUNITY_Auto-Minimize & Mod Loader Plan|Auto-Minimize & Mod Loader Plan]]
- [[_COMMUNITY_Deep-Link Entrypoint|Deep-Link Entrypoint]]
- [[_COMMUNITY_Legacy FML Libraries|Legacy FML Libraries]]
- [[_COMMUNITY_Create Dialog & Path Picker Impl|Create Dialog & Path Picker Impl]]
- [[_COMMUNITY_System Java Discovery|System Java Discovery]]
- [[_COMMUNITY_Process Lifecycle Watch|Process Lifecycle Watch]]
- [[_COMMUNITY_Single-Instance IPC|Single-Instance IPC]]
- [[_COMMUNITY_Vendored Material 1.0 Library|Vendored Material 1.0 Library]]
- [[_COMMUNITY_Caveman-Compress Skill Docs|Caveman-Compress Skill Docs]]
- [[_COMMUNITY_Instances Page Icons & Screenshot|Instances Page Icons & Screenshot]]
- [[_COMMUNITY_Cavecrew Subagent Skill Docs|Cavecrew Subagent Skill Docs]]
- [[_COMMUNITY_2026-08-20 Learn-Codebase Session|2026-08-20 Learn-Codebase Session]]
- [[_COMMUNITY_macOS URL Scheme Handler|macOS URL Scheme Handler]]
- [[_COMMUNITY_Caveman-Commit Skill Docs|Caveman-Commit Skill Docs]]
- [[_COMMUNITY_BookLogNote Icons|Book/Log/Note Icons]]
- [[_COMMUNITY_Arrow Navigation Icons|Arrow Navigation Icons]]
- [[_COMMUNITY_Caveman-Compress Package Init|Caveman-Compress Package Init]]
- [[_COMMUNITY_Caveman-Stats Skill Docs|Caveman-Stats Skill Docs]]
- [[_COMMUNITY_Account Page Redesign Log|Account Page Redesign Log]]
- [[_COMMUNITY_GitHub Actions CICD Migration|GitHub Actions CI/CD Migration]]
- [[_COMMUNITY_VoxelRuler README & Screenshot|VoxelRuler README & Screenshot]]
- [[_COMMUNITY_SunSwitch Icons|Sun/Switch Icons]]
- [[_COMMUNITY_Dropdown Arrow Icons|Dropdown Arrow Icons]]
- [[_COMMUNITY_CalendarClock Icons|Calendar/Clock Icons]]
- [[_COMMUNITY_CloseRemove Icons|Close/Remove Icons]]
- [[_COMMUNITY_Progress Check Log|Progress Check Log]]
- [[_COMMUNITY_Globe Icon|Globe Icon]]
- [[_COMMUNITY_Puzzle Icon|Puzzle Icon]]
- [[_COMMUNITY_Search Icon|Search Icon]]
- [[_COMMUNITY_Server Icon|Server Icon]]
- [[_COMMUNITY_Settings Gear Icon|Settings Gear Icon]]
- [[_COMMUNITY_Stop Icon|Stop Icon]]
- [[_COMMUNITY_User Icon|User Icon]]
- [[_COMMUNITY_App Logo|App Logo]]
- [[_COMMUNITY_Window Icon|Window Icon]]
- [[_COMMUNITY_Back Arrow Icon|Back Arrow Icon]]
- [[_COMMUNITY_Check Icon|Check Icon]]
- [[_COMMUNITY_Edit Pencil Icon|Edit Pencil Icon]]
- [[_COMMUNITY_Keyboard Icon|Keyboard Icon]]
- [[_COMMUNITY_Menu Hamburger Icon|Menu Hamburger Icon]]

## God Nodes (most connected - your core abstractions)
1. `McSpecificVersionDetail` - 32 edges
2. `InstanceConfig` - 30 edges
3. `do_launch()` - 28 edges
4. `McPaths` - 25 edges
5. `setup_launch_logic()` - 25 edges
6. `ModLoaderApi` - 24 edges
7. `InstanceStore` - 23 edges
8. `setup_instance_detail_logic()` - 19 edges
9. `McAction<Unauthenticated>` - 18 edges
10. `install_libraries()` - 18 edges

## Surprising Connections (you probably didn't know these)
- `setup_launch_logic()` --calls--> `InstanceStore::record_launch_started`  [EXTRACTED]
  src/view/launch.rs → docs/superpowers/plans/2026-07-06-session-tracking-and-close-to-minimize.md
- `setup_launch_logic()` --calls--> `InstanceStore::record_play_session_end`  [EXTRACTED]
  src/view/launch.rs → docs/superpowers/plans/2026-07-06-session-tracking-and-close-to-minimize.md
- `Resource/World/Shader Path Picker + Launch-Time Redirection Implementation Plan` --implements--> `sync_custom_dirs()`  [EXTRACTED]
  docs/superpowers/plans/2026-07-07-resource-path-picker.md → src/instance_assets.rs
- `World/Resource/Shader Path Picker — Design Spec` --rationale_for--> `sync_custom_dirs()`  [EXTRACTED]
  docs/superpowers/specs/2026-07-07-resource-path-picker-design.md → src/instance_assets.rs
- `Resource/World/Shader Path Picker + Launch-Time Redirection Implementation Plan` --implements--> `sync_one()`  [EXTRACTED]
  docs/superpowers/plans/2026-07-07-resource-path-picker.md → src/instance_assets.rs

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Cavecrew subagent delegation system** — agents_skills_cavecrew_readme, agents_skills_cavecrew_skill, cavecrew_investigator, cavecrew_builder, cavecrew_reviewer [EXTRACTED 1.00]
- **Caveman Skill Toolkit Family** — agents_skills_caveman_skill, agents_skills_caveman_compress_skill, agents_skills_caveman_help_skill, agents_skills_caveman_review_skill, agents_skills_caveman_stats_skill [EXTRACTED 1.00]
- **Create Instance Dialog: 4-Task Agent Batch Execution** — claude_agent_tasks_task_01_infra, claude_agent_tasks_task_02_model, claude_agent_tasks_task_03_ui, claude_agent_tasks_task_04_view, concept_create_instance_dialog, claude_conversations_2026_05_27 [EXTRACTED 1.00]
- **macOS Apple Silicon LWJGL/GLFW Compatibility Debugging Saga** — claude_conversations_2026_06_11, claude_conversations_2026_07_02, claude_conversations_2026_07_03, concept_apple_silicon_lwjgl_natives_compat, concept_glfw_icon_boot_crash_fix [INFERRED 0.85]
- **macOS Graphics Crash Diagnosis & LWJGL Override Pipeline** — _claude_rule_todo_mc_compat_diagnose_graphics_crash, _claude_rule_todo_macos_override_for, _claude_rule_todo_lwjgl3_x64_override, _claude_rule_todo_macos_tahoe_lwjgl_fix, _claude_rule_todo_mmachina_patched_glfw [INFERRED 0.85]
- **World/Resource/Shader Pack Custom Directory Selection Feature (PR #13)** — _claude_rule_todo_pr13_file_choose, _claude_rule_todo_rfd_crate, _claude_rule_todo_junction_crate, _claude_rule_todo_instance_assets_sync_custom_dirs [EXTRACTED 1.00]
- **Minecraft Launch Pipeline (M3 Core)** — _claude_rule_todo_mc_api_rs, _claude_rule_todo_mc_install_rs, _claude_rule_todo_mc_parser_rs, _claude_rule_todo_do_launch, _claude_rule_todo_set_install_state [EXTRACTED 1.00]
- **Mirrored CI/CD Pipelines: Forgejo + GitHub** — forgejo_workflows_ci, forgejo_workflows_release, github_workflows_ci, github_workflows_release [INFERRED 0.95]
- **Spec-driven-development task pipeline (plan -> per-task brief -> per-task report -> progress ledger)** — docs_superpowers_plans_2026_07_08_instance_detail_modloader_edit, superpowers_sdd_progress, superpowers_sdd_task_1_brief, superpowers_sdd_task_1_report [INFERRED 0.85]
- **M1 Account Milestone: Spec + Assignment + Known Bugs Blocking Acceptance** — claude_docs_feature_specs, claude_docs_task_assignment, docs_bug_review_cancel_login_port_leak, docs_bug_review_logout_cache_leak [INFERRED 0.85]
- **Instance Sort Settings feature (AppSettings + InstanceStore + SettingsLogic + refresh_instance_list)** — docs_superpowers_plans_2026_07_06_instance_sort_settings, src_settings_appsettings, src_mc_instance_instancestore, ui_global_settingslogic, src_view_mod_refresh_instance_list [EXTRACTED 1.00]
- **Launcher Window Lifecycle: Close/Minimize/Auto-Minimize Feature Family** — docs_superpowers_specs_2026_07_06_session_tracking_and_close_to_minimize_design, docs_superpowers_specs_2026_07_07_auto_minimize_on_launch_design, claude_conversations_2026_07_08, src_view_mod_on_close_requested [EXTRACTED 1.00]
- **VoxelRuler Sidebar Navigation Icons** — ui_assets_icons_home, ui_assets_icons_folder, ui_assets_icons_download [EXTRACTED 1.00]
- **Instances Page Branding and Action Icons** — ui_assets_icons_cube, ui_assets_icons_play, ui_assets_icons_plus [EXTRACTED 1.00]
- **Candidate Instance-Detail Content Tab Icons (Log/Note/Image/Camera/Book)** — ui_assets_icons_log, ui_assets_icons_note, ui_assets_icons_image, ui_assets_icons_camera, ui_assets_icons_book [INFERRED 0.70]
- **Material Navigation/Dropdown Arrow Icon Set** — ui_libs_material_1_0_ui_icons_arrow_back, ui_libs_material_1_0_ui_icons_arrow_drop_down, ui_libs_material_1_0_ui_icons_arrow_drop_up [INFERRED 0.85]
- **Directional Navigation Icons** — ui_libs_material_1_0_ui_icons_chevron_backward, ui_libs_material_1_0_ui_icons_chevron_forward, ui_libs_material_1_0_ui_icons_arrow_right [INFERRED 0.80]
- **UI Action Icons (Confirm/Cancel/Edit)** — ui_libs_material_1_0_ui_icons_check, ui_libs_material_1_0_ui_icons_close, ui_libs_material_1_0_ui_icons_remove, ui_libs_material_1_0_ui_icons_edit [INFERRED 0.75]
- **Material Design Icon Library (chunk 7)** — ui_libs_material_1_0_ui_icons_arrow_right, ui_libs_material_1_0_ui_icons_calendar_today, ui_libs_material_1_0_ui_icons_check, ui_libs_material_1_0_ui_icons_chevron_backward, ui_libs_material_1_0_ui_icons_chevron_forward, ui_libs_material_1_0_ui_icons_close, ui_libs_material_1_0_ui_icons_edit, ui_libs_material_1_0_ui_icons_keyboard, ui_libs_material_1_0_ui_icons_menu, ui_libs_material_1_0_ui_icons_remove, ui_libs_material_1_0_ui_icons_schedule [EXTRACTED 1.00]
- **VoxelRuler TODO Status Snapshot (2026-08-20)** — claude_conversations_2026_08_20_todo_md, claude_conversations_2026_08_20_m4_milestone, claude_conversations_2026_08_20_addinstance_ui, claude_conversations_2026_08_20_pr15_modloader_edit [EXTRACTED 0.90]

## Communities (61 total, 24 thin omitted)

### Community 0 - "Mod Loader Metadata (Fabric/Forge)"
Cohesion: 0.08
Nodes (36): MetadataCache, build_zip(), create_fake_profile(), FabricLoaderEntry, FabricLoaderInfo, http(), LoaderAvailability, LoaderFetchResult (+28 more)

### Community 1 - "Dev Standards & Feature Specs"
Cohesion: 0.06
Nodes (56): AuthorizationCode, VoxelRuler CLAUDE.md (project instructions), 開發規範 (Development Standards), No-Comments-by-Default Policy, No Direct .unwrap() Policy, 功能規格書 (Feature Specifications), FS-03 啟動 Minecraft (Launch), FS-06 多平台打包與發布 (+48 more)

### Community 2 - "Create Instance Dialog Agent Plan"
Cohesion: 0.08
Nodes (50): Agent 協調中心 — Create Instance Dialog, Agent 執行狀態, Task 01 — Infrastructure (Cargo deps, module scaffold), Task 02 — Data Layer (mc_instance.rs), Task 03 — UI Layer (Slint dialog), Task 04 — View Layer Refactor (view.rs), 2026-05-27 Conversation Log — Create Instance Dialog Implementation, 2026-06-08 Conversation Log — Skin Management Page (+42 more)

### Community 3 - "Instance Assets & NBT Utils"
Cohesion: 0.10
Nodes (56): Read, add_file(), build_servers_dat(), checked_len(), config_with_paths(), copy_dir_recursive(), create_dir_link(), delete_entry() (+48 more)

### Community 4 - "Launch Command Builder"
Cohesion: 0.09
Nodes (46): Command, K, Regex, cmd_args(), collect_args(), dedup_jvm_args(), detect_java_archs(), detect_java_major_version() (+38 more)

### Community 5 - "Caveman-Compress Benchmark & CLI"
Cohesion: 0.07
Nodes (49): benchmark_pair(), count_tokens(), main(), print_table(), Path, main(), print_usage(), backup_dir_for() (+41 more)

### Community 6 - "Instance Detail Mod Loader UI"
Cohesion: 0.08
Nodes (55): Instance Detail — Editable Mod Loader Implementation Plan, InstanceFileEntry, R, ModLoaderApi::fetch_loader_state, on_loader_changed handler (src/view/create.rs), append_log_line(), detail_category_dir(), detail_category_tab() (+47 more)

### Community 7 - "Minecraft Version JSON Schema"
Cohesion: 0.10
Nodes (51): bare_version(), lib(), McArgumentItem, McArguments, McArgumentValue, McArtifactInfo, McAssetIndex, McAssetObject (+43 more)

### Community 8 - "Mojang API Client"
Cohesion: 0.09
Nodes (24): McJavaAll, PhantomData, Response, S, Authenticated, McAction, McAction<Authenticated>, McAction<Unauthenticated> (+16 more)

### Community 9 - "Skin/Cape Upload Logic"
Cohesion: 0.09
Nodes (42): AppearanceLogic, AppearanceWindow, SkinData, auto_detect_variant(), fetch_cape_bytes_cached(), fetch_skin_bytes(), handle_browse_skin_file(), handle_select_cape() (+34 more)

### Community 10 - "Install Pipeline (Download/Verify)"
Cohesion: 0.12
Nodes (39): Send, count_jars(), create_nosig_jar(), download_and_verify(), download_best_effort(), empty_version(), extract_natives(), extract_single_native_jar() (+31 more)

### Community 11 - "Data Directory Layout"
Cohesion: 0.16
Nodes (19): McPaths, paths_in(), PathBuf, Result, Self, TempDir, test_asset_indexes_is_under_assets(), test_assets_dir_created() (+11 more)

### Community 12 - "Apple Silicon Compat Fixes"
Cohesion: 0.08
Nodes (30): 2026-05-20 Conversation Log — Java 17/21 JVM Compat Args, 2026-06-11 Conversation Log — Native Libraries & Java Version Fixes, 2026-07-02 Conversation Log — Graphics Crash Diagnosis & Vulkan Research, 2026-07-03 Conversation Log — UI Fixes, Code Review, Performance, 2026-07-04 Conversation Log — App Hang, Offline Launch, Scroll Perf, 2026-07-07 Conversation Log — Resource Path Picker, Apple Silicon (arm64) LWJGL/Java Native Compatibility Layer, GLFW setWindowIcon / Service-Port Boot Crash Fix (macOS) (+22 more)

### Community 13 - "TODO.md Milestone Tracker"
Cohesion: 0.06
Nodes (34): VoxelRuler 專案進度追蹤 (TODO.md), account.slint, AddInstance UI 串接, 啟動即自動最小化主視窗 (feat/view/auto-minimize-on-launch), cargo-packer 打包設定, directories::ProjectDirs, do_launch(), GitHub Actions workflow（三平台編譯） (+26 more)

### Community 14 - "3D Skin Preview Renderer"
Cohesion: 0.15
Nodes (16): Rgba8Pixel, RgbaImage, SharedPixelBuffer, DynamicImage, Fn, Option, Self, Vec (+8 more)

### Community 15 - "Instance Creation Logic"
Cohesion: 0.13
Nodes (21): empty_string_model(), Arc, Child, HashMap, MainApp, ModelRc, Mutex, Result (+13 more)

### Community 16 - "App Settings Persistence"
Cohesion: 0.15
Nodes (8): AppSettings, Path, PathBuf, Result, Self, String, test_settings_file_io(), on_save_settings handler (src/view/launch.rs)

### Community 17 - "Java Runtime Resolution"
Cohesion: 0.18
Nodes (14): Unauthenticated, install_java_runtime(), java_runtime_dir_name(), load_or_fetch_version_detail(), resolve_player_identity(), MainApp, Option, Path (+6 more)

### Community 18 - "Skin History Store"
Cohesion: 0.23
Nodes (13): make_entry(), Path, Result, Self, String, Vec, SkinEntry, SkinHistory (+5 more)

### Community 19 - "UI Wiring Entrypoint"
Cohesion: 0.17
Nodes (13): java_label_to_mode(), java_mode_to_label(), JavaSource, label_to_sort_mode(), open_view(), resolve_java_source(), MainApp, PathBuf (+5 more)

### Community 20 - "Auto-Minimize & Mod Loader Plan"
Cohesion: 0.20
Nodes (14): 2026-07-08 Conversation Log — Auto-Minimize & Mod Loader Edit, App Hangs on Close While Game Running — Fix, Auto-Minimize Main Window on Instance Launch, Instance Detail: Editable Mod Loader (Version Tab), Play-Session Tracking (last_played/play_time_secs) + Close-to-Minimize, Play-Session Tracking + Close-to-Minimize Implementation Plan, Auto-Minimize Main Window on Instance Launch Implementation Plan, Play-Session Tracking + Close-to-Minimize — Design Spec (+6 more)

### Community 21 - "Deep-Link Entrypoint"
Cohesion: 0.22
Nodes (12): AuthArgs, DeepLinkAction, main(), Option, Result, Self, String, setup_logging() (+4 more)

### Community 22 - "Legacy FML Libraries"
Cohesion: 0.26
Nodes (11): copy_fmllibs_to_game_dir(), get_fmllib_filenames(), install_fmllibs(), Path, Result, test_1_3_x_returns_three_libs(), test_1_4_x_returns_four_libs(), test_1_5_1_returns_correct_deobf_zip() (+3 more)

### Community 23 - "Create Dialog & Path Picker Impl"
Cohesion: 0.15
Nodes (13): Create Instance Dialog Implementation Plan, Resource/World/Shader Path Picker + Launch-Time Redirection Implementation Plan, InstanceData, InstanceLogic, junction crate (Windows directory-link dependency), config_to_ui_data (src/view.rs), rfd::AsyncFileDialog folder-picker handlers (src/view/create.rs), config_to_ui_data() (+5 more)

### Community 24 - "System Java Discovery"
Cohesion: 0.31
Nodes (12): home_dir(), java_exe_name(), push_if_java(), Option, Path, PathBuf, String, Vec (+4 more)

### Community 25 - "Process Lifecycle Watch"
Cohesion: 0.26
Nodes (12): AtomicBool, do_launch(), Arc, Child, HashMap, HashSet, LogLine, Mutex (+4 more)

### Community 26 - "Single-Instance IPC"
Cohesion: 0.30
Nodes (11): cleanup(), dispatch_ipc_bytes(), first_deeplink_arg(), Option, PathBuf, String, setup_ipc(), sock_path() (+3 more)

### Community 27 - "Vendored Material 1.0 Library"
Cohesion: 0.20
Nodes (11): Material 1.0 LICENSE (MIT), SixtyFPS GmbH (copyright holder), Material 1.0 README (Material Design 3 component set for Slint), Android APK demo (slint_material.apk), material-cpp-template, Material Design 3 guidelines (m3.material.io), material-nodejs-template, material-python-template (+3 more)

### Community 28 - "Caveman-Compress Skill Docs"
Cohesion: 0.22
Nodes (9): caveman-compress README, caveman-compress SECURITY, caveman-compress SKILL, caveman-help README, caveman-help SKILL, caveman README, caveman-review README, caveman-review SKILL (+1 more)

### Community 29 - "Instances Page Icons & Screenshot"
Cohesion: 0.29
Nodes (7): VoxelRuler Instances Page Screenshot, Cube Icon (Minecraft-style, green), Download Icon, Folder Icon, Home Icon, Play Icon, Plus Icon

### Community 30 - "Cavecrew Subagent Skill Docs"
Cohesion: 0.70
Nodes (5): cavecrew skill README, cavecrew SKILL definition, cavecrew-builder subagent, cavecrew-investigator subagent, cavecrew-reviewer subagent

### Community 31 - "2026-08-20 Learn-Codebase Session"
Cohesion: 0.40
Nodes (5): AddInstance UI Integration, /learn-codebase Priming Session, M4 Milestone (實例 CRUD), PR #15 Modloader Edit Review, TODO.md Progress Tracker

### Community 32 - "macOS URL Scheme Handler"
Cohesion: 0.40
Nodes (3): register(), String, UnboundedReceiver

### Community 33 - "Caveman-Commit Skill Docs"
Cohesion: 1.00
Nodes (3): caveman-commit Skill README, caveman-commit SKILL.md, caveman-commit: Terse Conventional Commits Skill

### Community 34 - "Book/Log/Note Icons"
Cohesion: 0.67
Nodes (3): Book Icon, Log Icon, Note Icon

### Community 35 - "Arrow Navigation Icons"
Cohesion: 0.67
Nodes (3): Arrow Right Icon, Chevron Backward Icon, Chevron Forward Icon

## Ambiguous Edges - Review These
- `Sun Icon` → `Switch (Swap Arrows) Icon`  [AMBIGUOUS]
  ui/assets/icons/Switch.svg · relation: conceptually_related_to

## Knowledge Gaps
- **83 isolated node(s):** `caveman-compress SECURITY`, `caveman-help README`, `caveman-review README`, `caveman-stats README`, `caveman-stats SKILL` (+78 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **24 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **What is the exact relationship between `Sun Icon` and `Switch (Swap Arrows) Icon`?**
  _Edge tagged AMBIGUOUS (relation: conceptually_related_to) - confidence is low._
- **Why does `InstanceConfig` connect `Create Instance Dialog Agent Plan` to `Instance Assets & NBT Utils`, `Instance Detail Mod Loader UI`, `Instance Creation Logic`, `Java Runtime Resolution`, `UI Wiring Entrypoint`, `Auto-Minimize & Mod Loader Plan`, `Create Dialog & Path Picker Impl`, `Process Lifecycle Watch`?**
  _High betweenness centrality (0.288) - this node is a cross-community bridge._
- **Why does `McSpecificVersionDetail` connect `Minecraft Version JSON Schema` to `Launch Command Builder`, `Mojang API Client`, `Install Pipeline (Download/Verify)`, `Apple Silicon Compat Fixes`, `Java Runtime Resolution`?**
  _High betweenness centrality (0.122) - this node is a cross-community bridge._
- **Why does `ModLoaderApi` connect `Mod Loader Metadata (Fabric/Forge)` to `Instance Detail Mod Loader UI`?**
  _High betweenness centrality (0.112) - this node is a cross-community bridge._
- **Are the 3 inferred relationships involving `do_launch()` (e.g. with `spawn_log_reader()` and `resolve_java_source()`) actually correct?**
  _`do_launch()` has 3 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Caveman compress scripts.  This package provides tools to compress natural langu`, `Split YAML frontmatter from body. Returns (frontmatter, body).      Memory files`, `Resolve the out-of-tree backup directory for a given source file.      Backups m` to the rest of the system?**
  _112 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Mod Loader Metadata (Fabric/Forge)` be split into smaller, more focused modules?**
  _Cohesion score 0.08050655811849841 - nodes in this community are weakly interconnected._