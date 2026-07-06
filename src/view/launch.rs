use crate::mc_api;
use crate::mc_install;
use crate::mc_instance::InstanceConfig;
use crate::mc_parser::LaunchContext;
use crate::mc_paths::McPaths;
use crate::mc_types::McSpecificVersionDetail;
use crate::settings::AppSettings;
use crate::view::MainApp;
use crate::view::*;
use crate::view::{JavaSource, resolve_java_source, set_install_state};
use anyhow::Context;
use slint::Weak;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tracing::{error, info, warn};

const OFFLINE_UUID: &str = "00000000-0000-0000-0000-000000000000";

fn java_runtime_dir_name(component: &str, os_arch: &str, native_arch: &str) -> String {
    if os_arch == native_arch {
        component.to_string()
    } else {
        format!("{component}-{os_arch}")
    }
}

fn should_fetch_online_profile(authenticator: &str, token: &str) -> bool {
    authenticator != "Offline" && !token.is_empty()
}

async fn load_or_fetch_version_detail(
    api: &crate::mc_api::McAction<crate::mc_api::Unauthenticated>,
    json_path: &Path,
    version_id: &str,
) -> anyhow::Result<McSpecificVersionDetail> {
    if let Ok(cached) = tokio::fs::read_to_string(json_path).await {
        match serde_json::from_str(&cached) {
            Ok(v) => {
                info!(version_id, "Using locally cached version JSON");
                return Ok(v);
            }
            Err(e) => {
                warn!(version_id, error = %e, "Local version JSON is corrupt, refetching");
            }
        }
    }
    let version = api
        .get_specific_mc_version_detail(version_id)
        .await
        .with_context(|| {
            format!(
                "Failed to get version data for {version_id}. \
                 If you are offline, launch this version once while online first."
            )
        })?;
    if let Some(parent) = json_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(json_path, serde_json::to_string(&version)?).await?;
    Ok(version)
}

fn resolve_player_identity(
    username: &str,
    online_profile: Option<(String, String)>,
) -> (String, String) {
    match online_profile {
        Some((name, id)) => (name, id),
        None => (username.to_string(), OFFLINE_UUID.to_string()),
    }
}

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

    let dir_name = java_runtime_dir_name(component, &os_arch, native_arch);
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
    let paths = McPaths::new()?;
    let vanilla_version_dir = paths.versions_dir().join(&version_id);
    let vanilla_json_path = vanilla_version_dir.join(format!("{}.json", version_id));
    let mut version = load_or_fetch_version_detail(&api, &vanilla_json_path, &version_id).await?;

    let app_settings = AppSettings::load();
    let java_source = resolve_java_source(&config, &app_settings);
    info!(?java_source, instance = %config.name, "Java source parsing result");

    let required_java_major = version.java_version.as_ref().map(|j| j.major_version);
    let mut actual_java_major: Option<i32> = None;

    let is_arm_mac = cfg!(target_os = "macos") && std::env::consts::ARCH == "aarch64";
    let supports_arm64 = crate::mc_parser::version_supports_macos_arm64(&version);
    let mut compat: Option<&'static crate::mc_compat::MacosArm64Override> = None;

    let java_path: PathBuf = match java_source {
        JavaSource::CustomPath(p) => {
            if !p.is_file() {
                anyhow::bail!(
                    "Custom Java path does not exist or is not a file: {}",
                    p.display()
                );
            }
            set_install_state(&ui_weak, true, 0.4, "Using custom Java...", false);

            #[cfg(target_os = "macos")]
            if !supports_arm64 {
                let probe = p.clone();
                let archs = tokio::task::spawn_blocking(move || {
                    crate::mc_parser::detect_java_archs(&probe)
                })
                .await
                .unwrap_or_default();
                info!(?archs, "Custom Java architecture detection");
                let java_is_arm64 =
                    is_arm_mac && (archs.is_empty() || archs.iter().any(|a| a == "arm64"));
                compat = crate::mc_compat::macos_override_for(&version, java_is_arm64);
                match compat {
                    Some(ov) => {
                        info!(
                            name = ov.name,
                            "macOS compatibility mode (library replacement)"
                        );
                    }
                    None if java_is_arm64 && !archs.iter().any(|a| a == "x86_64") => {
                        anyhow::bail!(
                            "This Minecraft version lacks Apple Silicon native libraries and has no replacements.\nRequires x86_64 Java via Rosetta, but selected Java architecture is {}.",
                            archs.join("/")
                        );
                    }
                    None => {}
                }
            }

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

            let os_arch = if is_arm_mac && !supports_arm64 {
                info!("Version lacks arm64-safe libraries, fetching x86_64 Java instead (Rosetta)");
                "mac-os"
            } else {
                crate::mc_parser::get_mojang_os_arch()
            };
            // 修 macOS 26 的 GLFW 65544 與開機 setIcon 的 65548
            compat = if cfg!(target_os = "macos") && !supports_arm64 {
                crate::mc_compat::macos_override_for(&version, false)
            } else {
                None
            };
            if let Some(ov) = compat {
                info!(
                    name = ov.name,
                    "macOS compatibility mode (library replacement)"
                );
            }
            actual_java_major = required_java_major;
            match install_java_runtime(&api, &paths, &component, os_arch, &ui_weak).await {
                Ok((path, _)) => path,
                Err(e) => {
                    let native_arch = crate::mc_parser::get_mojang_os_arch();
                    let mut candidates =
                        vec![java_runtime_dir_name(&component, os_arch, native_arch)];
                    if os_arch == "mac-os-arm64" {
                        candidates.push(java_runtime_dir_name(&component, "mac-os", native_arch));
                    }
                    match candidates
                        .iter()
                        .map(|d| paths.java_bin(d))
                        .find(|p| p.is_file())
                    {
                        Some(bin) => {
                            warn!(
                                error = %e, java = ?bin,
                                "Java manifest unavailable (offline?), using existing local runtime"
                            );
                            bin
                        }
                        None => return Err(e),
                    }
                }
            }
        }
    };

    tokio::fs::create_dir_all(&vanilla_version_dir).await?;

    info!(versions_dir = ?paths.versions_dir(), "Pre-installing vanilla Minecraft client for Mod Loader");
    mc_install::install_client(&version, &paths.versions_dir(), {
        let ui_weak = ui_weak.clone();
        move |p| {
            let status = format!("Downloading vanilla client... {:.0}%", p * 100.0);
            set_install_state(&ui_weak, true, 0.4 + p * 0.1, &status, false);
        }
    })
    .await?;

    if config.mod_loader != "None"
        && !config.mod_loader.is_empty()
        && !config.mod_loader_version.is_empty()
    {
        set_install_state(&ui_weak, true, 0.55, "Installing Mod Loader...", false);
        let loader_type = crate::mc_modloader::ModLoaderType::from_name(&config.mod_loader)
            .ok_or_else(|| anyhow::anyhow!("Unknown Mod Loader type: {}", config.mod_loader))?;
        let profile_id = crate::mc_modloader::ModLoaderApi::install_modloader(
            loader_type,
            &version_id,
            &config.mod_loader_version,
            &java_path,
            &paths.base_dir(),
        )
        .await?;

        info!(
            profile_id,
            "Mod Loader installation complete, reading new profile"
        );
        let profile_json_path = paths
            .versions_dir()
            .join(&profile_id)
            .join(format!("{}.json", profile_id));
        let profile_json_str = tokio::fs::read_to_string(&profile_json_path)
            .await
            .with_context(|| {
                format!(
                    "Failed to read Mod Loader profile: {}",
                    profile_json_path.display()
                )
            })?;
        let modded_version: crate::mc_types::McSpecificVersionDetail =
            serde_json::from_str(&profile_json_str)?;

        version = version.merge(modded_version);
        version_id = profile_id;

        let vanilla_jar_path = vanilla_version_dir.join(format!("{}.jar", config.version));
        let modded_version_dir = paths.versions_dir().join(&version_id);
        tokio::fs::create_dir_all(&modded_version_dir).await?;
        let modded_jar_path = modded_version_dir.join(format!("{}.jar", version_id));
        if vanilla_jar_path.exists() && !modded_jar_path.exists() {
            tokio::fs::copy(&vanilla_jar_path, &modded_jar_path).await?;
        }
    }

    info!(versions_dir = ?paths.versions_dir(), "Checking Minecraft client installation");
    mc_install::install_client(&version, &paths.versions_dir(), {
        let ui_weak = ui_weak.clone();
        move |p| {
            let status = format!("Downloading Minecraft client... {:.0}%", p * 100.0);
            set_install_state(&ui_weak, true, 0.4 + p * 0.2, &status, false);
        }
    })
    .await
    .context("Failed to install Minecraft client")?;

    // Forge (launchwrapper) 需剝除 JAR code signing，否則 ASM bytecode 轉換時 JVM 會拋出 SecurityException
    if config.mod_loader == "Forge" {
        let version_jar = paths
            .versions_dir()
            .join(&version_id)
            .join(format!("{}.jar", version_id));
        let nosig_jar = paths
            .versions_dir()
            .join(&version_id)
            .join(format!("{}-nosig.jar", version_id));
        mc_install::create_nosig_jar(&version_jar, &nosig_jar)
            .await
            .context("Failed to strip Forge JAR signature")?;
    }

    if config.mod_loader == "Forge" {
        set_install_state(
            &ui_weak,
            true,
            0.55,
            "Downloading legacy FML dependencies...",
            false,
        );
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

    let online_profile = if should_fetch_online_profile(&active_account_authenticator, &token) {
        crate::mc_api::McAction::new()
            .authenticate(&token)
            .get_user_profile()
            .await
            .ok()
            .map(|profile| (profile.name, profile.id))
    } else {
        None
    };
    let (player_name, player_uuid) =
        resolve_player_identity(&active_account_username, online_profile);

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
    let missing = ctx.missing_classpath_files();
    if !missing.is_empty() {
        let list = missing
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        error!(
            count = missing.len(),
            "classpath missing library files:\n{list}"
        );
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
    let logic = ui.global::<InstanceLogic>();
    let master_for_search = Arc::clone(&master_configs);
    let ui_weak_for_search = ui.as_weak();
    let running_procs_for_search = Arc::clone(&running_procs);
    logic.on_search_changed(move |text| {
        let Some(ui) = ui_weak_for_search.upgrade() else {
            return;
        };
        let logic = ui.global::<InstanceLogic>();
        let configs = master_for_search.lock().unwrap();
        let running_ids: std::collections::HashSet<String> = running_procs_for_search
            .lock()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        let needle = text.to_lowercase();
        let filtered: Vec<InstanceData> = configs
            .iter()
            .filter(|c| needle.is_empty() || c.name.to_lowercase().contains(&needle))
            .map(|c| {
                let mut item = config_to_ui_data(c);
                if running_ids.contains(&c.id) {
                    item.status = "running".into();
                }
                item
            })
            .collect();
        logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(filtered))));
    });

    let master_for_launch = Arc::clone(&master_configs);
    let store_for_launch = Arc::clone(&store);
    let running_procs_for_launch = Arc::clone(&running_procs);
    let launching_procs_for_launch = Arc::clone(&launching_procs);
    let instance_logs_for_launch = Arc::clone(&instance_logs);
    let pending_quit_for_launch = Arc::clone(&pending_quit);
    let ui_weak_for_launch = ui.as_weak();
    logic.on_launch_instance(move |id| {
        let (active_account_authenticator, active_account_username) = {
            let Some(ui) = ui_weak_for_launch.upgrade() else {
                return;
            };
            let acc = ui.global::<PageAccountLogic>().get_active_account();
            (acc.authenticator.to_string(), acc.username.to_string())
        };
        if active_account_authenticator == "No Account" {
            set_install_state(
                &ui_weak_for_launch,
                true,
                0.0,
                "Please login to an account first",
                true,
            );
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
        let master_for_session = Arc::clone(&master_for_launch);
        let store_for_session = Arc::clone(&store_for_launch);
        let pending_quit = Arc::clone(&pending_quit_for_launch);
        let mut launching_lock = launching_procs.lock().unwrap();
        if launching_lock.contains(&instance_id) {
            return;
        }
        if !launching_lock.is_empty() {
            set_install_state(
                &ui_weak_for_launch,
                true,
                0.0,
                "Another instance is launching, please wait for it to finish",
                true,
            );
            return;
        }
        launching_lock.insert(instance_id.clone());
        drop(launching_lock);
        tokio::spawn(async move {
            let logs_watch = Arc::clone(&logs);
            let res = do_launch(
                config,
                ui_weak.clone(),
                logs,
                active_account_authenticator,
                active_account_username,
            )
            .await;
            launching_procs.lock().unwrap().remove(&instance_id);
            match res {
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
                    tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            let mut map = running_procs_watch.lock().unwrap();
                            let Some(child) = map.get_mut(&id_watch) else {
                                break;
                            };
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
            c.java_path = java_path;
            c.clone()
        };

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

        let mut new_settings = AppSettings::load();
        new_settings.java_mode = java_mode.to_string();
        new_settings.java_path = java_path;
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
                store.lock().unwrap().save_one(&new_config)?;
                Ok(())
            })();
            if let Err(e) = result {
                error!("Failed to duplicate instance: {e:#}");
                set_install_state(
                    &ui_weak,
                    true,
                    0.0,
                    &format!("Failed to duplicate instance: {e:#}"),
                    true,
                );
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
        if let Some(mut child) = running_for_del.lock().unwrap().remove(&id) {
            let _ = child.kill();
        }
        if let Err(e) = store_for_del.lock().unwrap().delete_one(&id) {
            error!("Failed to delete instance: {e:#}");
        }
        logic.set_show_delete_confirm(false);
        let detail = ui.global::<InstanceDetailLogic>();
        if detail.get_instance_id().as_str() == id {
            detail.set_show_dialog(false);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_java_runtime_dir_name_native_arch() {
        assert_eq!(
            java_runtime_dir_name("java-runtime-gamma", "mac-os-arm64", "mac-os-arm64"),
            "java-runtime-gamma"
        );
    }

    #[test]
    fn test_java_runtime_dir_name_cross_arch_gets_suffix() {
        assert_eq!(
            java_runtime_dir_name("java-runtime-beta", "mac-os", "mac-os-arm64"),
            "java-runtime-beta-mac-os"
        );
    }

    #[test]
    fn test_should_fetch_online_profile() {
        assert!(should_fetch_online_profile("Microsoft", "some-token"));
        assert!(!should_fetch_online_profile("Offline", "some-token"));
        assert!(!should_fetch_online_profile("Microsoft", ""));
    }

    #[test]
    fn test_resolve_player_identity_online() {
        let (name, uuid) = resolve_player_identity(
            "LocalName",
            Some(("OnlineName".to_string(), "abc-123".to_string())),
        );
        assert_eq!(name, "OnlineName");
        assert_eq!(uuid, "abc-123");
    }

    #[test]
    fn test_resolve_player_identity_fallback_to_offline() {
        let (name, uuid) = resolve_player_identity("LocalName", None);
        assert_eq!(name, "LocalName");
        assert_eq!(uuid, OFFLINE_UUID);
    }
}
