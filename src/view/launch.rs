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




