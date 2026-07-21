# Graph Report - .  (2026-07-21)

## Corpus Check
- 254 files · ~317,242 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1028 nodes · 2254 edges · 56 communities (32 shown, 24 thin omitted)
- Extraction: 97% EXTRACTED · 3% INFERRED · 0% AMBIGUOUS · INFERRED: 67 edges (avg confidence: 0.81)
- Token cost: 769,845 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Instance Launch & Lifecycle|Instance Launch & Lifecycle]]
- [[_COMMUNITY_Mod Loader Installation (FabricForgeNeoForge)|Mod Loader Installation (Fabric/Forge/NeoForge)]]
- [[_COMMUNITY_OAuth Session & Project Documentation|OAuth Session & Project Documentation]]
- [[_COMMUNITY_Instance Data Store & Sorting|Instance Data Store & Sorting]]
- [[_COMMUNITY_Compatibility Diagnostics & Game Install|Compatibility Diagnostics & Game Install]]
- [[_COMMUNITY_Instance Asset Management (modsworldslogs)|Instance Asset Management (mods/worlds/logs)]]
- [[_COMMUNITY_Caveman-Compress Skill Scripts|Caveman-Compress Skill Scripts]]
- [[_COMMUNITY_Launch Command Parser|Launch Command Parser]]
- [[_COMMUNITY_Minecraft API Client|Minecraft API Client]]
- [[_COMMUNITY_Minecraft Version Type Models|Minecraft Version Type Models]]
- [[_COMMUNITY_Skin & Appearance UI|Skin & Appearance UI]]
- [[_COMMUNITY_Mod Loader Edit Feature (SDD Tasks)|Mod Loader Edit Feature (SDD Tasks)]]
- [[_COMMUNITY_Instance Detail Window & Log Viewer|Instance Detail Window & Log Viewer]]
- [[_COMMUNITY_Minecraft Path Resolution|Minecraft Path Resolution]]
- [[_COMMUNITY_Project TODO  Milestones|Project TODO / Milestones]]
- [[_COMMUNITY_Skin 3D Renderer|Skin 3D Renderer]]
- [[_COMMUNITY_App Settings Persistence|App Settings Persistence]]
- [[_COMMUNITY_macOS Apple Silicon Compatibility Saga|macOS Apple Silicon Compatibility Saga]]
- [[_COMMUNITY_Skin History Store|Skin History Store]]
- [[_COMMUNITY_App Entry & Deep Link Auth|App Entry & Deep Link Auth]]
- [[_COMMUNITY_Legacy Forge FML Installer|Legacy Forge FML Installer]]
- [[_COMMUNITY_Java Runtime Scanner|Java Runtime Scanner]]
- [[_COMMUNITY_Single-Instance IPC|Single-Instance IPC]]
- [[_COMMUNITY_Material UI Library Docs|Material UI Library Docs]]
- [[_COMMUNITY_Caveman Skill Docs|Caveman Skill Docs]]
- [[_COMMUNITY_Instances Page Icons|Instances Page Icons]]
- [[_COMMUNITY_Cavecrew Skill|Cavecrew Skill]]
- [[_COMMUNITY_URL Scheme Handler|URL Scheme Handler]]
- [[_COMMUNITY_Caveman-Commit Skill|Caveman-Commit Skill]]
- [[_COMMUNITY_Detail Tab Icons (BookLogNote)|Detail Tab Icons (Book/Log/Note)]]
- [[_COMMUNITY_ChevronArrow Icons|Chevron/Arrow Icons]]
- [[_COMMUNITY_Compress Init Script|Compress Init Script]]
- [[_COMMUNITY_Caveman-Stats Skill|Caveman-Stats Skill]]
- [[_COMMUNITY_Account Page UI Redesign|Account Page UI Redesign]]
- [[_COMMUNITY_GitHub Actions CICD Setup|GitHub Actions CI/CD Setup]]
- [[_COMMUNITY_Project README|Project README]]
- [[_COMMUNITY_Theme Toggle Icons (SunSwitch)|Theme Toggle Icons (Sun/Switch)]]
- [[_COMMUNITY_Dropdown Arrow Icons|Dropdown Arrow Icons]]
- [[_COMMUNITY_DateTime Icons|Date/Time Icons]]
- [[_COMMUNITY_Dismiss Icons (CloseRemove)|Dismiss Icons (Close/Remove)]]
- [[_COMMUNITY_Conversation 2026-05-25|Conversation 2026-05-25]]
- [[_COMMUNITY_Globe Icon|Globe Icon]]
- [[_COMMUNITY_Puzzle Icon|Puzzle Icon]]
- [[_COMMUNITY_Search Icon|Search Icon]]
- [[_COMMUNITY_Server Icon|Server Icon]]
- [[_COMMUNITY_Settings Gear Icon|Settings Gear Icon]]
- [[_COMMUNITY_Stop Icon|Stop Icon]]
- [[_COMMUNITY_User Icon|User Icon]]
- [[_COMMUNITY_App Logo (voxelruler.png)|App Logo (voxelruler.png)]]
- [[_COMMUNITY_Window Icon|Window Icon]]
- [[_COMMUNITY_Back Arrow Icon|Back Arrow Icon]]
- [[_COMMUNITY_Check Icon|Check Icon]]
- [[_COMMUNITY_Edit Icon|Edit Icon]]
- [[_COMMUNITY_Keyboard Icon|Keyboard Icon]]
- [[_COMMUNITY_Menu Icon|Menu Icon]]

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
- `Resource/World/Shader Path Picker + Launch-Time Redirection Implementation Plan` --implements--> `sync_custom_dirs()`  [EXTRACTED]
  docs/superpowers/plans/2026-07-07-resource-path-picker.md → src/instance_assets.rs
- `World/Resource/Shader Path Picker — Design Spec` --rationale_for--> `sync_custom_dirs()`  [EXTRACTED]
  docs/superpowers/specs/2026-07-07-resource-path-picker-design.md → src/instance_assets.rs
- `Resource/World/Shader Path Picker + Launch-Time Redirection Implementation Plan` --implements--> `sync_one()`  [EXTRACTED]
  docs/superpowers/plans/2026-07-07-resource-path-picker.md → src/instance_assets.rs
- `2026-07-02 Conversation Log — Graphics Crash Diagnosis & Vulkan Research` --rationale_for--> `diagnose_graphics_crash()`  [EXTRACTED]
  .claude/conversations/2026-07-02.md → src/mc_compat.rs
- `Create Instance Dialog Implementation Plan` --implements--> `InstanceConfig`  [EXTRACTED]
  docs/superpowers/plans/2026-05-27-create-instance-dialog.md → src/mc_instance.rs

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Cavecrew subagent delegation system** — agents_skills_cavecrew_readme, agents_skills_cavecrew_skill, cavecrew_investigator, cavecrew_builder, cavecrew_reviewer [EXTRACTED 1.00]
- **Instance Sort Settings feature (AppSettings + InstanceStore + SettingsLogic + refresh_instance_list)** — docs_superpowers_plans_2026_07_06_instance_sort_settings, src_settings_appsettings, src_mc_instance_instancestore, ui_global_settingslogic, src_view_mod_refresh_instance_list [EXTRACTED 1.00]
- **Spec-driven-development task pipeline (plan -> per-task brief -> per-task report -> progress ledger)** — docs_superpowers_plans_2026_07_08_instance_detail_modloader_edit, superpowers_sdd_progress, superpowers_sdd_task_1_brief, superpowers_sdd_task_1_report [INFERRED 0.85]
- **Create Instance Dialog: 4-Task Agent Batch Execution** — claude_agent_tasks_task_01_infra, claude_agent_tasks_task_02_model, claude_agent_tasks_task_03_ui, claude_agent_tasks_task_04_view, concept_create_instance_dialog, claude_conversations_2026_05_27 [EXTRACTED 1.00]
- **macOS Apple Silicon LWJGL/GLFW Compatibility Debugging Saga** — claude_conversations_2026_06_11, claude_conversations_2026_07_02, claude_conversations_2026_07_03, concept_apple_silicon_lwjgl_natives_compat, concept_glfw_icon_boot_crash_fix [INFERRED 0.85]
- **Launcher Window Lifecycle: Close/Minimize/Auto-Minimize Feature Family** — docs_superpowers_specs_2026_07_06_session_tracking_and_close_to_minimize_design, docs_superpowers_specs_2026_07_07_auto_minimize_on_launch_design, claude_conversations_2026_07_08, src_view_mod_on_close_requested [EXTRACTED 1.00]
- **M1 Account Milestone: Spec + Assignment + Known Bugs Blocking Acceptance** — claude_docs_feature_specs, claude_docs_task_assignment, docs_bug_review_cancel_login_port_leak, docs_bug_review_logout_cache_leak [INFERRED 0.85]
- **Mirrored CI/CD Pipelines: Forgejo + GitHub** — forgejo_workflows_ci, forgejo_workflows_release, github_workflows_ci, github_workflows_release [INFERRED 0.95]
- **Caveman Skill Toolkit Family** — agents_skills_caveman_skill, agents_skills_caveman_compress_skill, agents_skills_caveman_help_skill, agents_skills_caveman_review_skill, agents_skills_caveman_stats_skill [EXTRACTED 1.00]
- **macOS Graphics Crash Diagnosis & LWJGL Override Pipeline** — _claude_rule_todo_mc_compat_diagnose_graphics_crash, _claude_rule_todo_macos_override_for, _claude_rule_todo_lwjgl3_x64_override, _claude_rule_todo_macos_tahoe_lwjgl_fix, _claude_rule_todo_mmachina_patched_glfw [INFERRED 0.85]
- **World/Resource/Shader Pack Custom Directory Selection Feature (PR #13)** — _claude_rule_todo_pr13_file_choose, _claude_rule_todo_rfd_crate, _claude_rule_todo_junction_crate, _claude_rule_todo_instance_assets_sync_custom_dirs [EXTRACTED 1.00]
- **Minecraft Launch Pipeline (M3 Core)** — _claude_rule_todo_mc_api_rs, _claude_rule_todo_mc_install_rs, _claude_rule_todo_mc_parser_rs, _claude_rule_todo_do_launch, _claude_rule_todo_set_install_state [EXTRACTED 1.00]
- **VoxelRuler Sidebar Navigation Icons** — ui_assets_icons_home, ui_assets_icons_folder, ui_assets_icons_download [EXTRACTED 1.00]
- **Instances Page Branding and Action Icons** — ui_assets_icons_cube, ui_assets_icons_play, ui_assets_icons_plus [EXTRACTED 1.00]
- **Candidate Instance-Detail Content Tab Icons (Log/Note/Image/Camera/Book)** — ui_assets_icons_log, ui_assets_icons_note, ui_assets_icons_image, ui_assets_icons_camera, ui_assets_icons_book [INFERRED 0.70]
- **Material Navigation/Dropdown Arrow Icon Set** — ui_libs_material_1_0_ui_icons_arrow_back, ui_libs_material_1_0_ui_icons_arrow_drop_down, ui_libs_material_1_0_ui_icons_arrow_drop_up [INFERRED 0.85]
- **Material Design Icon Library (chunk 7)** — ui_libs_material_1_0_ui_icons_arrow_right, ui_libs_material_1_0_ui_icons_calendar_today, ui_libs_material_1_0_ui_icons_check, ui_libs_material_1_0_ui_icons_chevron_backward, ui_libs_material_1_0_ui_icons_chevron_forward, ui_libs_material_1_0_ui_icons_close, ui_libs_material_1_0_ui_icons_edit, ui_libs_material_1_0_ui_icons_keyboard, ui_libs_material_1_0_ui_icons_menu, ui_libs_material_1_0_ui_icons_remove, ui_libs_material_1_0_ui_icons_schedule [EXTRACTED 1.00]
- **Directional Navigation Icons** — ui_libs_material_1_0_ui_icons_chevron_backward, ui_libs_material_1_0_ui_icons_chevron_forward, ui_libs_material_1_0_ui_icons_arrow_right [INFERRED 0.80]
- **UI Action Icons (Confirm/Cancel/Edit)** — ui_libs_material_1_0_ui_icons_check, ui_libs_material_1_0_ui_icons_close, ui_libs_material_1_0_ui_icons_remove, ui_libs_material_1_0_ui_icons_edit [INFERRED 0.75]

## Communities (56 total, 24 thin omitted)

### Community 0 - "Instance Launch & Lifecycle"
Cohesion: 0.05
Nodes (64): AtomicBool, 2026-07-08 Conversation Log — Auto-Minimize & Mod Loader Edit, Auto-Minimize Main Window on Instance Launch, Instance Detail: Editable Mod Loader (Version Tab), Create Instance Dialog Implementation Plan, Play-Session Tracking + Close-to-Minimize Implementation Plan, Auto-Minimize Main Window on Instance Launch Implementation Plan, Resource/World/Shader Path Picker + Launch-Time Redirection Implementation Plan (+56 more)

### Community 1 - "Mod Loader Installation (Fabric/Forge/NeoForge)"
Cohesion: 0.08
Nodes (36): MetadataCache, build_zip(), create_fake_profile(), FabricLoaderEntry, FabricLoaderInfo, http(), LoaderAvailability, LoaderFetchResult (+28 more)

### Community 2 - "OAuth Session & Project Documentation"
Cohesion: 0.06
Nodes (56): AuthorizationCode, VoxelRuler CLAUDE.md (project instructions), 開發規範 (Development Standards), No-Comments-by-Default Policy, No Direct .unwrap() Policy, 功能規格書 (Feature Specifications), FS-03 啟動 Minecraft (Launch), FS-06 多平台打包與發布 (+48 more)

### Community 3 - "Instance Data Store & Sorting"
Cohesion: 0.08
Nodes (50): Agent 協調中心 — Create Instance Dialog, Agent 執行狀態, Task 01 — Infrastructure (Cargo deps, module scaffold), Task 02 — Data Layer (mc_instance.rs), Task 03 — UI Layer (Slint dialog), Task 04 — View Layer Refactor (view.rs), 2026-05-27 Conversation Log — Create Instance Dialog Implementation, 2026-06-08 Conversation Log — Skin Management Page (+42 more)

### Community 4 - "Compatibility Diagnostics & Game Install"
Cohesion: 0.08
Nodes (53): I, Send, arm64_override_for(), CompatArtifact, CrashSignature, diagnose_graphics_crash(), diagnose_with_os(), load_version() (+45 more)

### Community 5 - "Instance Asset Management (mods/worlds/logs)"
Cohesion: 0.10
Nodes (56): Read, add_file(), build_servers_dat(), checked_len(), config_with_paths(), copy_dir_recursive(), create_dir_link(), delete_entry() (+48 more)

### Community 6 - "Caveman-Compress Skill Scripts"
Cohesion: 0.07
Nodes (49): benchmark_pair(), count_tokens(), main(), print_table(), Path, main(), print_usage(), backup_dir_for() (+41 more)

### Community 7 - "Launch Command Parser"
Cohesion: 0.09
Nodes (45): Command, K, Regex, cmd_args(), collect_args(), dedup_jvm_args(), detect_java_archs(), detect_java_major_version() (+37 more)

### Community 8 - "Minecraft API Client"
Cohesion: 0.09
Nodes (28): McJavaAll, PhantomData, Response, S, Authenticated, McAction, McAction<Authenticated>, McAction<Unauthenticated> (+20 more)

### Community 9 - "Minecraft Version Type Models"
Cohesion: 0.10
Nodes (47): feature_rule_matches(), bare_version(), lib(), McArgumentItem, McArguments, McArgumentValue, McArtifactInfo, McAssetIndex (+39 more)

### Community 10 - "Skin & Appearance UI"
Cohesion: 0.09
Nodes (44): AppearanceLogic, AppearanceWindow, SkinData, auto_detect_variant(), fetch_cape_bytes_cached(), fetch_skin_bytes(), handle_browse_skin_file(), handle_select_cape() (+36 more)

### Community 11 - "Mod Loader Edit Feature (SDD Tasks)"
Cohesion: 0.08
Nodes (38): Instance Detail — Editable Mod Loader Implementation Plan, ModLoaderApi::fetch_loader_state, empty_string_model(), on_loader_changed handler (src/view/create.rs), Arc, Child, HashMap, MainApp (+30 more)

### Community 12 - "Instance Detail Window & Log Viewer"
Cohesion: 0.13
Nodes (38): InstanceFileEntry, R, append_log_line(), detail_category_dir(), detail_category_tab(), file_entries_model(), load_detail_tab(), log_line() (+30 more)

### Community 13 - "Minecraft Path Resolution"
Cohesion: 0.16
Nodes (19): McPaths, paths_in(), PathBuf, Result, Self, TempDir, test_asset_indexes_is_under_assets(), test_assets_dir_created() (+11 more)

### Community 14 - "Project TODO / Milestones"
Cohesion: 0.06
Nodes (34): VoxelRuler 專案進度追蹤 (TODO.md), account.slint, AddInstance UI 串接, 啟動即自動最小化主視窗 (feat/view/auto-minimize-on-launch), cargo-packer 打包設定, directories::ProjectDirs, do_launch(), GitHub Actions workflow（三平台編譯） (+26 more)

### Community 15 - "Skin 3D Renderer"
Cohesion: 0.17
Nodes (14): Rgba8Pixel, RgbaImage, SharedPixelBuffer, DynamicImage, Fn, Option, Self, Vec (+6 more)

### Community 16 - "App Settings Persistence"
Cohesion: 0.15
Nodes (8): AppSettings, Path, PathBuf, Result, Self, String, test_settings_file_io(), on_save_settings handler (src/view/launch.rs)

### Community 17 - "macOS Apple Silicon Compatibility Saga"
Cohesion: 0.12
Nodes (19): 2026-05-20 Conversation Log — Java 17/21 JVM Compat Args, 2026-06-11 Conversation Log — Native Libraries & Java Version Fixes, 2026-07-02 Conversation Log — Graphics Crash Diagnosis & Vulkan Research, 2026-07-03 Conversation Log — UI Fixes, Code Review, Performance, 2026-07-04 Conversation Log — App Hang, Offline Launch, Scroll Perf, 2026-07-07 Conversation Log — Resource Path Picker, App Hangs on Close While Game Running — Fix, Apple Silicon (arm64) LWJGL/Java Native Compatibility Layer (+11 more)

### Community 18 - "Skin History Store"
Cohesion: 0.23
Nodes (13): make_entry(), Path, Result, Self, String, Vec, SkinEntry, SkinHistory (+5 more)

### Community 19 - "App Entry & Deep Link Auth"
Cohesion: 0.22
Nodes (12): AuthArgs, DeepLinkAction, main(), Option, Result, Self, String, setup_logging() (+4 more)

### Community 20 - "Legacy Forge FML Installer"
Cohesion: 0.26
Nodes (11): copy_fmllibs_to_game_dir(), get_fmllib_filenames(), install_fmllibs(), Path, Result, test_1_3_x_returns_three_libs(), test_1_4_x_returns_four_libs(), test_1_5_1_returns_correct_deobf_zip() (+3 more)

### Community 21 - "Java Runtime Scanner"
Cohesion: 0.31
Nodes (12): home_dir(), java_exe_name(), push_if_java(), Option, Path, PathBuf, String, Vec (+4 more)

### Community 22 - "Single-Instance IPC"
Cohesion: 0.30
Nodes (11): cleanup(), dispatch_ipc_bytes(), first_deeplink_arg(), Option, PathBuf, String, setup_ipc(), sock_path() (+3 more)

### Community 23 - "Material UI Library Docs"
Cohesion: 0.20
Nodes (11): Material 1.0 LICENSE (MIT), SixtyFPS GmbH (copyright holder), Material 1.0 README (Material Design 3 component set for Slint), Android APK demo (slint_material.apk), material-cpp-template, Material Design 3 guidelines (m3.material.io), material-nodejs-template, material-python-template (+3 more)

### Community 24 - "Caveman Skill Docs"
Cohesion: 0.22
Nodes (9): caveman-compress README, caveman-compress SECURITY, caveman-compress SKILL, caveman-help README, caveman-help SKILL, caveman README, caveman-review README, caveman-review SKILL (+1 more)

### Community 25 - "Instances Page Icons"
Cohesion: 0.29
Nodes (7): VoxelRuler Instances Page Screenshot, Cube Icon (Minecraft-style, green), Download Icon, Folder Icon, Home Icon, Play Icon, Plus Icon

### Community 26 - "Cavecrew Skill"
Cohesion: 0.70
Nodes (5): cavecrew skill README, cavecrew SKILL definition, cavecrew-builder subagent, cavecrew-investigator subagent, cavecrew-reviewer subagent

### Community 27 - "URL Scheme Handler"
Cohesion: 0.40
Nodes (3): register(), String, UnboundedReceiver

### Community 28 - "Caveman-Commit Skill"
Cohesion: 1.00
Nodes (3): caveman-commit Skill README, caveman-commit SKILL.md, caveman-commit: Terse Conventional Commits Skill

### Community 29 - "Detail Tab Icons (Book/Log/Note)"
Cohesion: 0.67
Nodes (3): Book Icon, Log Icon, Note Icon

### Community 30 - "Chevron/Arrow Icons"
Cohesion: 0.67
Nodes (3): Arrow Right Icon, Chevron Backward Icon, Chevron Forward Icon

## Ambiguous Edges - Review These
- `Sun Icon` → `Switch (Swap Arrows) Icon`  [AMBIGUOUS]
  ui/assets/icons/Switch.svg · relation: conceptually_related_to

## Knowledge Gaps
- **80 isolated node(s):** `CreateInstanceDialog component`, `InstanceStore::set_sort`, `SettingsLogic (ui/global.slint)`, `junction crate (Windows directory-link dependency)`, `rfd::AsyncFileDialog folder-picker handlers (src/view/create.rs)` (+75 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **24 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **What is the exact relationship between `Sun Icon` and `Switch (Swap Arrows) Icon`?**
  _Edge tagged AMBIGUOUS (relation: conceptually_related_to) - confidence is low._
- **Why does `InstanceConfig` connect `Instance Data Store & Sorting` to `Instance Launch & Lifecycle`, `Mod Loader Edit Feature (SDD Tasks)`, `Instance Detail Window & Log Viewer`, `Instance Asset Management (mods/worlds/logs)`?**
  _High betweenness centrality (0.261) - this node is a cross-community bridge._
- **Why does `McSpecificVersionDetail` connect `Compatibility Diagnostics & Game Install` to `Minecraft API Client`, `Minecraft Version Type Models`, `Instance Launch & Lifecycle`, `Launch Command Parser`?**
  _High betweenness centrality (0.115) - this node is a cross-community bridge._
- **Why does `ModLoaderApi` connect `Mod Loader Installation (Fabric/Forge/NeoForge)` to `Mod Loader Edit Feature (SDD Tasks)`?**
  _High betweenness centrality (0.111) - this node is a cross-community bridge._
- **Are the 3 inferred relationships involving `do_launch()` (e.g. with `spawn_log_reader()` and `resolve_java_source()`) actually correct?**
  _`do_launch()` has 3 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Caveman compress scripts.  This package provides tools to compress natural langu`, `Split YAML frontmatter from body. Returns (frontmatter, body).      Memory files`, `Resolve the out-of-tree backup directory for a given source file.      Backups m` to the rest of the system?**
  _109 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Instance Launch & Lifecycle` be split into smaller, more focused modules?**
  _Cohesion score 0.05150905432595573 - nodes in this community are weakly interconnected._