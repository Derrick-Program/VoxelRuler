use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, VecDeque};
use slint::Weak;
use anyhow::Context;
use tracing::{info, warn, error};
use crate::mc_paths::McPaths;
use crate::mc_install;
use crate::mc_api;
use crate::mc_types::McSpecificVersionDetail;
use crate::mc_parser::LaunchContext;
use crate::mc_instance::InstanceConfig;
use crate::settings::AppSettings;
use crate::view::MainApp;
use crate::view::{set_install_state, JavaSource, resolve_java_source};
use crate::view::*;

pub(crate) async fn install_java_runtime(
    api: &crate::mc_api::McAction<crate::mc_api::Unauthenticated>,
    paths: &McPaths,
    component: &str,
    os_arch: &str,
    ui_weak: &slint::Weak<MainApp>,
) -> anyhow::Result<(PathBuf, String)> {
    let native_arch = crate::mc_parser::get_mojang_os_arch();
    let mut os_arch = os_arch.to_string();
    let mut manifest = api
        .get_java_runtime_manifest_for_platform(component, &os_arch)
        .await;

    // 官方目錄缺漏保險：Apple Silicon 目錄沒有該 component（如 java-runtime-beta
    // 只有 x64 版）→ 自動改抓 x64 經 Rosetta 執行
    if manifest.is_err() && os_arch == "mac-os-arm64" {
        warn!(
            component,
            "No official arm64 version, automatically falling back to x86_64 (Rosetta)"
        );
        os_arch = "mac-os".to_string();
        manifest = api
            .get_java_runtime_manifest_for_platform(component, &os_arch)
            .await;
    }
    let manifest = manifest
        .with_context(|| format!("Failed to get Java runtime '{component}' ({os_arch}) info"))?;

    let dir_name = if os_arch == native_arch {
        component.to_string()
    } else {
        format!("{component}-{os_arch}")
    };
    info!(java_dir = ?paths.java_dir(&dir_name), component, %os_arch, "Starting Java installation");
    mc_install::install_java(&manifest, &paths.java_dir(&dir_name), {
        let ui_weak = ui_weak.clone();
        move |p| {
            let status = format!("Downloading Java runtime... {:.0}%", p * 100.0);
            set_install_state(&ui_weak, true, 0.1 + p * 0.3, &status, false);
        }
    })
    .await
    .context("Failed to install Java")?;
    Ok((paths.java_bin(&dir_name), os_arch))
}

pub(crate) async fn do_launch(
    config: InstanceConfig,
    ui_weak: slint::Weak<MainApp>,
    instance_logs: Arc<Mutex<HashMap<String, VecDeque<crate::view::LogLine>>>>,
    active_account_authenticator: String,
    active_account_username: String,
) -> anyhow::Result<Child> {
    let mut version_id = config.version.clone();
    let instance_id = config.id.clone();

    set_install_state(&ui_weak, true, 0.0, "Fetching version data...", false);

    let api = crate::mc_api::McAction::new();
    let mut version = api.get_specific_mc_version_detail(&version_id).await?;
    let paths = McPaths::new()?;

    // Java 解析：instance（path > runtime）→ 全域（path > runtime）→ 版本預設
    let app_settings = AppSettings::load();
    let java_source = resolve_java_source(&config, &app_settings);
    info!(?java_source, instance = %config.name, "Java source parsing result");

    let required_java_major = version.java_version.as_ref().map(|j| j.major_version);
    let mut actual_java_major: Option<i32> = None;

    // Apple Silicon：1.19 之前的版本只有 x86_64 natives。
    // 若實際使用 arm64 Java，改用 Prism 式函式庫替換（compat）原生執行；
    // 若使用 x86_64 Java（Rosetta），維持原版函式庫。
    let is_arm_mac = cfg!(target_os = "macos") && std::env::consts::ARCH == "aarch64";
    let supports_arm64 = crate::mc_parser::version_supports_macos_arm64(&version);
    let mut compat: Option<&'static crate::mc_compat::MacosArm64Override> = None;

    let java_path: PathBuf = match java_source {
        JavaSource::CustomPath(p) => {
            if !p.is_file() {
                anyhow::bail!("Custom Java path does not exist or is not a file: {}", p.display());
            }
            set_install_state(&ui_weak, true, 0.4, "Using custom Java...", false);

            #[cfg(target_os = "macos")]
            if is_arm_mac && !supports_arm64 {
                let probe = p.clone();
                let archs = tokio::task::spawn_blocking(move || {
                    crate::mc_parser::detect_java_archs(&probe)
                })
                .await
                .unwrap_or_default();
                info!(?archs, "Custom Java architecture detection");
                let java_is_arm64 = archs.is_empty() || archs.iter().any(|a| a == "arm64");
                if java_is_arm64 {
                    compat = crate::mc_compat::arm64_override_for(&version);
                    match compat {
                        Some(ov) => {
                            info!(name = ov.name, "Enabling Apple Silicon native mode (library replacement)")
                        }
                        None if !archs.iter().any(|a| a == "x86_64") => {
                            anyhow::bail!(
                                "This Minecraft version lacks Apple Silicon native libraries and has no replacements.\nRequires x86_64 Java via Rosetta, but selected Java architecture is {}.",
                                archs.join("/")
                            );
                        }
                        None => {}
                    }
                }
            }

            // 偵測實際 Java 版本：compat flags 依此決定，過舊則提前給明確錯誤
            let probe = p.clone();
            actual_java_major = tokio::task::spawn_blocking(move || {
                crate::mc_parser::detect_java_major_version(&probe)
            })
            .await
            .ok()
            .flatten();
            info!(
                ?actual_java_major,
                ?required_java_major,
                "Custom Java version detection"
            );

            if let (Some(actual), Some(required)) = (actual_java_major, required_java_major)
                && actual < required
            {
                anyhow::bail!(
                    "This Minecraft version requires Java {required} or above, but selected Java is {actual} ({}).\nPlease change Java in instance settings or global settings.",
                    p.display()
                );
            }
            p
        }
        JavaSource::VersionDefault => {
            let component = version
                .java_version
                .as_ref()
                .map(|j| j.component.clone())
                .unwrap_or_else(|| "jre-legacy".into());

            if is_arm_mac && !supports_arm64 {
                compat = crate::mc_compat::arm64_override_for(&version);
            }

            // Mojang 只有 Java 17+（gamma/delta）的 arm64 版；
            // 版本需求 >= 16（1.17–1.18）才能走原生模式，否則退回 Rosetta
            if compat.is_some() && required_java_major.is_some_and(|m| m >= 16) {
                info!(
                    name = compat.map(|o| o.name),
                    "Apple Silicon native mode: using arm64 java-runtime-gamma"
                );
                actual_java_major = Some(17);
                let requested_arch = crate::mc_parser::get_mojang_os_arch();
                let (path, used_arch) = install_java_runtime(
                    &api,
                    &paths,
                    "java-runtime-gamma",
                    requested_arch,
                    &ui_weak,
                )
                .await?;
                // 官方 arm64 目錄缺貨而 fallback 至 x64 時，
                // 必須同步取消替換（x64 Java 配 arm64 natives 會炸）→ 改走 Rosetta + 原版函式庫
                if used_arch != requested_arch {
                    warn!(
                        "arm64 Java unavailable, fell back to x86_64, cancelled library replacement (Rosetta mode)"
                    );
                    compat = None;
                }
                path
            } else {
                let os_arch = if is_arm_mac && !supports_arm64 {
                    // 舊版需 Java 8，Mojang 無 arm64 版 → Rosetta + 原版函式庫
                    compat = None;
                    info!("This version lacks arm64 natives and arm64 Java, fetching x86_64 Java instead (Rosetta)");
                    "mac-os"
                } else {
                    crate::mc_parser::get_mojang_os_arch()
                };
                actual_java_major = required_java_major;
                let (path, _) =
                    install_java_runtime(&api, &paths, &component, os_arch, &ui_weak).await?;
                path
            }
        }
    };

    if config.mod_loader != "None" && !config.mod_loader.is_empty() && !config.mod_loader_version.is_empty() {
        set_install_state(&ui_weak, true, 0.45, "Installing Mod Loader...", false);
        let loader_type = match config.mod_loader.as_str() {
            "Fabric" => crate::mc_modloader::ModLoaderType::Fabric,
            "Forge" => crate::mc_modloader::ModLoaderType::Forge,
            "NeoForge" => crate::mc_modloader::ModLoaderType::NeoForge,
            _ => anyhow::bail!("Unknown Mod Loader type: {}", config.mod_loader),
        };
        let profile_id = crate::mc_modloader::ModLoaderApi::install_modloader(
            loader_type,
            &version_id,
            &config.mod_loader_version,
            &java_path,
            &paths.base_dir(),
        ).await?;
        
        info!(profile_id, "Mod Loader installation complete, reading new profile");
        let profile_json_path = paths.versions_dir().join(&profile_id).join(format!("{}.json", profile_id));
        let profile_json_str = tokio::fs::read_to_string(&profile_json_path)
            .await
            .with_context(|| format!("Failed to read Mod Loader profile: {}", profile_json_path.display()))?;
        let modded_version: crate::mc_types::McSpecificVersionDetail = serde_json::from_str(&profile_json_str)?;
        
        version = version.merge(modded_version);
        version_id = profile_id;
    }

    info!(versions_dir = ?paths.versions_dir(), "Starting Minecraft client installation");
    mc_install::install_client(&version, &paths.versions_dir(), {
        let ui_weak = ui_weak.clone();
        move |p| {
            let status = format!("Downloading Minecraft client... {:.0}%", p * 100.0);
            set_install_state(&ui_weak, true, 0.4 + p * 0.2, &status, false);
        }
    })
    .await
    .context("Failed to install Minecraft client")?;

    // Forge (launchwrapper) 需剝除 Minecraft JAR 的 code signing，
    // 否則 ASM bytecode 轉換時 JVM 會因 package seal 驗證拋出 SecurityException。
    // 原版 JAR 保持原大小（避免 download_and_verify 重複下載）；classpath 改用 nosig copy。
    if config.mod_loader == "Forge" {
        let version_jar = paths.versions_dir()
            .join(&version_id)
            .join(format!("{}.jar", version_id));
        let nosig_jar = paths.versions_dir()
            .join(&version_id)
            .join(format!("{}-nosig.jar", version_id));
        mc_install::create_nosig_jar(&version_jar, &nosig_jar)
            .await
            .context("Failed to strip Forge JAR signature")?;
    }

    // 舊版 Forge（≤1.6.4）需要額外的 FML 依賴（deobfuscation data、bcprov、asm 等），
    // 這些依賴不在版本 JSON 中，由 FML bootstrap 在執行時動態載入。
    if config.mod_loader == "Forge" {
        set_install_state(&ui_weak, true, 0.55, "Downloading legacy FML dependencies...", false);
        crate::mc_legacy_fml::install_fmllibs(&version_id, &paths.libraries_dir())
            .await
            .context("Failed to download legacy FML dependencies")?;
    }

    info!(libraries_dir = ?paths.libraries_dir(), "Starting library installation");
    mc_install::install_libraries(&version, &paths.libraries_dir(), compat, {
        let ui_weak = ui_weak.clone();
        move |p| {
            let status = format!("Downloading libraries... {:.0}%", p * 100.0);
            set_install_state(&ui_weak, true, 0.6 + p * 0.2, &status, false);
        }
    })
    .await
    .context("Failed to install libraries")?;

    // Copy downloaded legacy fmllibs to the game_dir/lib folder before launch
    if config.mod_loader == "Forge" {
        crate::mc_legacy_fml::copy_fmllibs_to_game_dir(
            &version_id,
            &paths.libraries_dir(),
            &paths.instance_dir(&instance_id),
        )
        .await
        .context("Failed to copy legacy Forge dependencies")?;
    }

    info!(natives_dir = ?paths.natives_dir(&version_id), "Extracting native libraries");
    set_install_state(&ui_weak, true, 0.8, "Extracting native libraries...", false);
    mc_install::extract_natives(
        &version,
        &paths.libraries_dir(),
        &paths.natives_dir(&version_id),
        compat,
    )
    .await
    .context("Failed to extract native libraries")?;

    info!(assets_dir = ?paths.assets_dir(), "Starting game assets installation");
    mc_install::install_assets(&version, &paths.assets_dir(), {
        let ui_weak = ui_weak.clone();
        move |p| {
            let status = format!("Downloading game assets... {:.0}%", p * 100.0);
            set_install_state(&ui_weak, true, 0.8 + p * 0.2, &status, false);
        }
    })
    .await
    .context("Failed to install game assets")?;



    set_install_state(&ui_weak, true, 1.0, "Starting game...", false);

    let token = crate::GLOBAL_CACHE
        .get("mc_ac_key")
        .map(|v| v.clone())
        .unwrap_or_default();

    let (player_name, player_uuid) = if active_account_authenticator == "Offline" {
        (
            active_account_username.clone(),
            "00000000-0000-0000-0000-000000000000".into(),
        )
    } else if !token.is_empty() {
        match crate::mc_api::McAction::new()
            .authenticate(&token)
            .get_user_profile()
            .await
        {
            Ok(profile) => (profile.name, profile.id),
            Err(_) => (
                active_account_username.clone(),
                "00000000-0000-0000-0000-000000000000".into(),
            ),
        }
    } else {
        (
            active_account_username.clone(),
            "00000000-0000-0000-0000-000000000000".into(),
        )
    };

    let ctx = LaunchContext {
        version,
        java_path,
        game_dir: paths.instance_dir(&instance_id),
        libraries_dir: paths.libraries_dir(),
        assets_dir: paths.assets_dir(),
        natives_dir: paths.natives_dir(&version_id),
        versions_dir: paths.versions_dir(),
        auth_player_name: player_name,
        auth_uuid: player_uuid,
        auth_access_token: token,
        client_id: String::new(),
        xuid: String::new(),
        xmx: config.xmx.clone(),
        xms: config.xms.clone(),
        java_major_version: actual_java_major,
        compat_override: compat,
    };
    // 啟動前缺檔檢查：避免 Java 端丟出難排查的 ClassNotFound / UnsatisfiedLinkError
    let missing = ctx.missing_classpath_files();
    if !missing.is_empty() {
        let list = missing
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        error!(count = missing.len(), "classpath missing library files:\n{list}");
        anyhow::bail!(
            "Pre-launch check failed, missing {} library files (see log). Please retry to download.",
            missing.len()
        );
    }

    let mut cmd = ctx.build_command();
    debug!(cmd = ?cmd, java = ?ctx.java_path, game_dir = ?ctx.game_dir, "Launch command");
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().with_context(|| {
        format!(
            "Spawn failed, java={:?} game_dir={:?}",
            ctx.java_path, ctx.game_dir
        )
    })?;
    set_install_state(&ui_weak, false, 0.0, "", false);

    instance_logs
        .lock()
        .unwrap()
        .insert(instance_id.clone(), VecDeque::with_capacity(500));

    if let Some(stdout) = child.stdout.take() {
        spawn_log_reader(
            stdout,
            instance_id.clone(),
            Arc::clone(&instance_logs),
            ui_weak.clone(),
        );
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_reader(
            stderr,
            instance_id.clone(),
            Arc::clone(&instance_logs),
            ui_weak.clone(),
        );
    }

    Ok(child)
}





pub fn setup_launch_logic(
    ui: &MainApp,
    store: std::sync::Arc<std::sync::Mutex<crate::mc_instance::InstanceStore>>,
    master_configs: std::sync::Arc<std::sync::Mutex<Vec<crate::mc_instance::InstanceConfig>>>,
    running_procs: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, std::process::Child>>>,
    launching_procs: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    instance_logs: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, std::collections::VecDeque<crate::view::LogLine>>>>,
) {
    let logic = ui.global::<InstanceLogic>();
        let master_for_search = Arc::clone(&master_configs);
        let ui_weak_for_search = ui.as_weak();
        logic.on_search_changed(move |text| {
            let Some(ui) = ui_weak_for_search.upgrade() else {
                return;
            };
            let logic = ui.global::<InstanceLogic>();
            let configs = master_for_search.lock().unwrap();
            let filtered: Vec<InstanceData> = configs
                .iter()
                .filter(|c| text.is_empty() || c.name.to_lowercase().contains(&text.to_lowercase()))
                .map(config_to_ui_data)
                .collect();
            logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(filtered))));
        });
    
    
        let master_for_launch = Arc::clone(&master_configs);
        let running_procs_for_launch = Arc::clone(&running_procs);
        let launching_procs_for_launch = Arc::clone(&launching_procs);
        let instance_logs_for_launch = Arc::clone(&instance_logs);
        let ui_weak_for_launch = ui.as_weak();
        logic.on_launch_instance(move |id| {
            let (active_account_authenticator, active_account_username) = {
                let Some(ui) = ui_weak_for_launch.upgrade() else { return; };
                let acc = ui.global::<PageAccountLogic>().get_active_account();
                (acc.authenticator.to_string(), acc.username.to_string())
            };
            if active_account_authenticator == "No Account" {
                set_install_state(&ui_weak_for_launch, true, 0.0, "Please login to an account first", true);
                return;
            }
    
            let config = {
                let configs = master_for_launch.lock().unwrap();
                configs
                    .iter()
                    .find(|c| c.id == id.as_str())
                    .cloned()
                    .unwrap_or_else(|| InstanceConfig {
                        id: id.to_string(),
                        name: id.to_string(),
                        version: id.to_string(),
                        ..Default::default()
                    })
            };
            let instance_id = config.id.clone();
            let running_procs = Arc::clone(&running_procs_for_launch);
            let launching_procs = Arc::clone(&launching_procs_for_launch);
            let ui_weak = ui_weak_for_launch.clone();
            let logs = Arc::clone(&instance_logs_for_launch);
            if !running_procs.lock().unwrap().is_empty() {
                set_install_state(&ui_weak_for_launch, true, 0.0, "Please close the currently running game first", true);
                return;
            }
            let mut launching_lock = launching_procs.lock().unwrap();
            if !launching_lock.is_empty() {
                return; // Already launching some instance
            }
            launching_lock.insert(instance_id.clone());
            drop(launching_lock);
            tokio::spawn(async move {
                let res = do_launch(config, ui_weak.clone(), logs, active_account_authenticator, active_account_username).await;
                launching_procs.lock().unwrap().remove(&instance_id);
                match res {
                    Ok(child) => {
                        running_procs
                            .lock()
                            .unwrap()
                            .insert(instance_id.clone(), child);
                        set_instance_status(&ui_weak, &instance_id, "running");
                        let running_procs_watch = Arc::clone(&running_procs);
                        let ui_weak_watch = ui_weak.clone();
                        let id_watch = instance_id.clone();
                        tokio::spawn(async move {
                            loop {
                                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                                let mut map = running_procs_watch.lock().unwrap();
                                let Some(child) = map.get_mut(&id_watch) else {
                                    break;
                                };
                                match child.try_wait() {
                                    Ok(Some(_)) | Err(_) => {
                                        map.remove(&id_watch);
                                        drop(map);
                                        set_instance_status(&ui_weak_watch, &id_watch, "ready");
                                        break;
                                    }
                                    Ok(None) => {}
                                }
                            }
                        });
                    }
                    Err(e) => {
                        error!("Launch failed: {e:#}");
                        set_install_state(&ui_weak, true, 0.0, &format!("Launch failed: {e:#}"), true);
                    }
                }
            });
        });
    
        let running_procs_for_kill = Arc::clone(&running_procs);
        let ui_weak_for_kill = ui.as_weak();
        logic.on_kill_instance(move |id| {
            let mut map = running_procs_for_kill.lock().unwrap();
            if let Some(mut child) = map.remove(id.as_str()) {
                let _ = child.kill();
                drop(map);
                set_instance_status(&ui_weak_for_kill, id.as_str(), "ready");
            }
        });
    
        // 「編輯實例」→ 開啟詳細視窗的「設定」分頁（tab 9）
        let master_for_edit_open = Arc::clone(&master_configs);
        let running_for_edit_open = Arc::clone(&running_procs);
        let logs_for_edit_open = Arc::clone(&instance_logs);
        let ui_weak_for_edit_open = ui.as_weak();
        logic.on_open_instance_settings(move |id| {
            let Some(ui) = ui_weak_for_edit_open.upgrade() else {
                return;
            };
            open_instance_detail(
                &ui,
                &master_for_edit_open,
                &running_for_edit_open,
                &logs_for_edit_open,
                id.as_str(),
                9,
            );
        });
    
        let edit_logic = ui.global::<InstanceEditLogic>();
    
        let ui_weak_for_edit_cancel = ui.as_weak();
        edit_logic.on_cancel_edit(move || {
            if let Some(ui) = ui_weak_for_edit_cancel.upgrade() {
                ui.global::<InstanceEditLogic>().set_show_dialog(false);
            }
        });
    
        let store_for_edit = Arc::clone(&store);
        let master_for_edit = Arc::clone(&master_configs);
        let ui_weak_for_edit_confirm = ui.as_weak();
        edit_logic.on_confirm_edit(move || {
            let Some(ui) = ui_weak_for_edit_confirm.upgrade() else {
                return;
            };
            let edit = ui.global::<InstanceEditLogic>();
            let id = edit.get_instance_id().to_string();
            let xmx = edit.get_xmx().trim().to_string();
            let xms = edit.get_xms().trim().to_string();
            let java_path = edit.get_java_path().trim().to_string();
            let java_mode = java_label_to_mode(edit.get_selected_java_mode().as_str());
    
            if java_mode == JAVA_MODE_CUSTOM {
                if java_path.is_empty() {
                    edit.set_error_msg("Path is required when selecting custom Java".into());
                    return;
                }
                if !Path::new(&java_path).is_file() {
                    edit.set_error_msg("Custom Java path does not exist or is not a file".into());
                    return;
                }
            }
    
            let updated_config = {
                let mut master = master_for_edit.lock().unwrap();
                let Some(c) = master.iter_mut().find(|c| c.id == id) else {
                    edit.set_error_msg("Instance not found".into());
                    return;
                };
                c.xmx = if xmx.is_empty() { "2G".into() } else { xmx };
                c.xms = if xms.is_empty() { "512M".into() } else { xms };
                c.java_mode = java_mode.to_string();
                // 路徑文字保留，切換模式時不清掉使用者輸入
                c.java_path = java_path;
                c.clone()
            };
    
            // 寫入 instance.toml；watcher 會自動同步 UI 列表
            match store_for_edit.lock().unwrap().save_one(&updated_config) {
                Ok(()) => {
                    edit.set_error_msg("".into());
                    ui.global::<InstanceDetailLogic>()
                        .set_status_msg("✓ Saved".into());
                }
                Err(e) => edit.set_error_msg(format!("Save failed: {e}").into()),
            }
        });
    
        let ui_weak_for_settings = ui.as_weak();
        ui.global::<SettingsLogic>().on_save_settings(move || {
            let Some(ui) = ui_weak_for_settings.upgrade() else {
                return;
            };
            let sl = ui.global::<SettingsLogic>();
            let java_path = sl.get_java_path().trim().to_string();
            let java_mode = java_label_to_mode(sl.get_selected_java_mode().as_str());
    
            if java_mode == JAVA_MODE_CUSTOM {
                if java_path.is_empty() {
                    sl.set_status_msg("⚠ Path is required when selecting custom Java".into());
                    return;
                }
                if !Path::new(&java_path).is_file() {
                    sl.set_status_msg("⚠ Java path does not exist or is not a file".into());
                    return;
                }
            }
    
            let new_settings = AppSettings {
                java_mode: java_mode.to_string(),
                java_path,
            };
            match new_settings.save() {
                Ok(()) => sl.set_status_msg("✓ Saved".into()),
                Err(e) => sl.set_status_msg(format!("Save failed: {e}").into()),
            }
        });
    
        let ui_weak_for_dismiss = ui.as_weak();
        logic.on_dismiss_install_dialog(move || {
            if let Some(ui) = ui_weak_for_dismiss.upgrade() {
                let logic = ui.global::<InstanceLogic>();
                logic.set_is_installing(false);
                logic.set_install_is_error(false);
                logic.set_install_status("".into());
            }
        });
    
        let instance_logs_for_open = Arc::clone(&instance_logs);
        let ui_weak_for_open = ui.as_weak();
        logic.on_open_log(move |id| {
            let id = id.to_string();
            let lines: Vec<crate::view::LogLine> = {
                let logs = instance_logs_for_open.lock().unwrap();
                logs.get(&id)
                    .map(|deque| deque.iter().cloned().collect())
                    .unwrap_or_default()
            };
            if let Some(ui) = ui_weak_for_open.upgrade() {
                let logic = ui.global::<InstanceLogic>();
                logic.set_log_instance_id(id.into());
                logic.set_log_lines(ModelRc::from(Rc::new(VecModel::from(lines))));
                logic.set_show_log(true);
            }
        });
    
        let ui_weak_for_close_log = ui.as_weak();
        logic.on_close_log(move || {
            if let Some(ui) = ui_weak_for_close_log.upgrade() {
                ui.global::<InstanceLogic>().set_show_log(false);
            }
        });
    
        // ── 右鍵選單：開啟資料夾 / 複製 / 重新命名 / 刪除 ─────────────────────
        logic.on_open_instance_folder(move |id| {
            if let Ok(paths) = McPaths::new() {
                let _ = open::that(paths.instance_dir(id.as_str()));
            }
        });
    
        let store_for_dup = Arc::clone(&store);
        let master_for_dup = Arc::clone(&master_configs);
        let ui_weak_for_dup = ui.as_weak();
        logic.on_duplicate_instance(move |id| {
            let config = {
                let configs = master_for_dup.lock().unwrap();
                configs.iter().find(|c| c.id == id.as_str()).cloned()
            };
            let Some(config) = config else { return };
            let store = Arc::clone(&store_for_dup);
            let ui_weak = ui_weak_for_dup.clone();
            // 實例資料夾可能很大（worlds / mods），放 blocking thread 複製
            tokio::task::spawn_blocking(move || {
                let result = (|| -> anyhow::Result<()> {
                    let paths = McPaths::new()?;
                    let mut new_config = config.clone();
                    new_config.id = uuid::Uuid::new_v4().to_string();
                    new_config.name = format!("{} (copy)", config.name);
                    new_config.last_played = String::new();
                    new_config.play_time_secs = 0;
                    let src = paths.instance_dir(&config.id);
                    let dst = paths.instance_dir(&new_config.id);
                    crate::instance_assets::copy_dir_recursive(&src, &dst)?;
                    // 覆寫複製來的 instance.toml（換 id / 名稱）；watcher 會同步列表
                    store.lock().unwrap().save_one(&new_config)?;
                    Ok(())
                })();
                if let Err(e) = result {
                    error!("Failed to duplicate instance: {e:#}");
                    set_install_state(&ui_weak, true, 0.0, &format!("Failed to duplicate instance: {e:#}"), true);
                }
            });
        });
    
        let store_for_rename = Arc::clone(&store);
        let master_for_rename = Arc::clone(&master_configs);
        let ui_weak_for_rename = ui.as_weak();
        logic.on_confirm_rename(move || {
            let Some(ui) = ui_weak_for_rename.upgrade() else {
                return;
            };
            let logic = ui.global::<InstanceLogic>();
            let id = logic.get_rename_id().to_string();
            let new_name = logic.get_rename_text().trim().to_string();
            if new_name.is_empty() {
                logic.set_rename_error("Name cannot be empty".into());
                return;
            }
            let updated = {
                let mut master = master_for_rename.lock().unwrap();
                let Some(c) = master.iter_mut().find(|c| c.id == id) else {
                    logic.set_rename_error("Instance not found".into());
                    return;
                };
                c.name = new_name;
                c.clone()
            };
            match store_for_rename.lock().unwrap().save_one(&updated) {
                Ok(()) => logic.set_show_rename(false),
                Err(e) => logic.set_rename_error(format!("Save failed: {e}").into()),
            }
        });
    
        let store_for_del = Arc::clone(&store);
        let running_for_del = Arc::clone(&running_procs);
        let ui_weak_for_del = ui.as_weak();
        logic.on_confirm_delete(move || {
            let Some(ui) = ui_weak_for_del.upgrade() else {
                return;
            };
            let logic = ui.global::<InstanceLogic>();
            let id = logic.get_delete_id().to_string();
            // 執行中先停止
            if let Some(mut child) = running_for_del.lock().unwrap().remove(&id) {
                let _ = child.kill();
            }
            if let Err(e) = store_for_del.lock().unwrap().delete_one(&id) {
                error!("Failed to delete instance: {e:#}");
            }
            logic.set_show_delete_confirm(false);
            // 詳細視窗若開著同一實例，順手關閉
            let detail = ui.global::<InstanceDetailLogic>();
            if detail.get_instance_id().as_str() == id {
                detail.set_show_dialog(false);
            }
        });
    
}
