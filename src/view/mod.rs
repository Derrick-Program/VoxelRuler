slint::include_modules!();
pub mod appearance;
pub mod create;
pub mod instance_detail;
pub mod launch;
pub mod login;
pub mod skin;

use instance_detail::*;
use launch::*;
use skin::*;

use crate::{
    mc_install,
    mc_instance::{InstanceConfig, InstanceStore},
    mc_parser::LaunchContext,
    mc_paths::McPaths,
    mc_token::{self, SessionData},
    mc_types::McSpecificVersionDetail,
    settings::AppSettings,
};
use anyhow::Context as _;
use slint::{Model, ModelRc, VecModel};
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    process::Child,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tracing::{debug, error, info, warn};

const JAVA_MODE_LABEL_GLOBAL: &str = "Use Global Settings";
const JAVA_MODE_LABEL_MINECRAFT: &str = "Use Minecraft Runtime";
const JAVA_MODE_LABEL_CUSTOM: &str = "Use Custom Java";

const JAVA_MODE_GLOBAL: &str = "global";
const JAVA_MODE_MINECRAFT: &str = "minecraft";
const JAVA_MODE_CUSTOM: &str = "custom";

fn java_mode_to_label(mode: &str, is_instance: bool) -> &'static str {
    match mode {
        JAVA_MODE_CUSTOM => JAVA_MODE_LABEL_CUSTOM,
        JAVA_MODE_MINECRAFT => JAVA_MODE_LABEL_MINECRAFT,
        _ if is_instance => JAVA_MODE_LABEL_GLOBAL,
        _ => JAVA_MODE_LABEL_MINECRAFT,
    }
}

fn java_label_to_mode(label: &str) -> &'static str {
    match label {
        JAVA_MODE_LABEL_CUSTOM => JAVA_MODE_CUSTOM,
        JAVA_MODE_LABEL_MINECRAFT => JAVA_MODE_MINECRAFT,
        _ => JAVA_MODE_GLOBAL,
    }
}

const SORT_LABEL_NAME: &str = "Name (A-Z)";
const SORT_LABEL_VERSION: &str = "Version";
const SORT_LABEL_CREATED_AT: &str = "Created Time";
const SORT_LABEL_LAST_PLAYED: &str = "Last Played";

fn sort_mode_to_label(mode: crate::settings::SortMode) -> &'static str {
    use crate::settings::SortMode;
    match mode {
        SortMode::Name => SORT_LABEL_NAME,
        SortMode::Version => SORT_LABEL_VERSION,
        SortMode::CreatedAt => SORT_LABEL_CREATED_AT,
        SortMode::LastPlayed => SORT_LABEL_LAST_PLAYED,
    }
}

fn label_to_sort_mode(label: &str) -> crate::settings::SortMode {
    use crate::settings::SortMode;
    match label {
        SORT_LABEL_NAME => SortMode::Name,
        SORT_LABEL_VERSION => SortMode::Version,
        SORT_LABEL_LAST_PLAYED => SortMode::LastPlayed,
        _ => SortMode::CreatedAt,
    }
}

#[derive(Debug)]
enum JavaSource {
    CustomPath(PathBuf),
    VersionDefault,
}

fn resolve_java_source(instance: &InstanceConfig, settings: &AppSettings) -> JavaSource {
    match instance.java_mode.as_str() {
        JAVA_MODE_CUSTOM if !instance.java_path.trim().is_empty() => {
            return JavaSource::CustomPath(PathBuf::from(instance.java_path.trim()));
        }
        JAVA_MODE_MINECRAFT => return JavaSource::VersionDefault,
        _ => {}
    }
    match settings.java_mode.as_str() {
        JAVA_MODE_CUSTOM if !settings.java_path.trim().is_empty() => {
            JavaSource::CustomPath(PathBuf::from(settings.java_path.trim()))
        }
        _ => JavaSource::VersionDefault,
    }
}

fn config_to_ui_data(config: &InstanceConfig) -> InstanceData {
    let play_time = if config.play_time_secs == 0 {
        String::new()
    } else {
        let h = config.play_time_secs / 3600;
        let m = (config.play_time_secs % 3600) / 60;
        format!("{}h {}m", h, m)
    };
    InstanceData {
        id: config.id.as_str().into(),
        name: config.name.as_str().into(),
        version: config.version.as_str().into(),
        mod_loader: config.mod_loader.as_str().into(),
        last_played: config.last_played.as_str().into(),
        play_time: play_time.into(),
        image: slint::Image::default(),
        status: "ready".into(),
    }
}

fn refresh_instance_list(
    logic: &InstanceLogic,
    configs: &[InstanceConfig],
    running_ids: &std::collections::HashSet<String>,
) {
    let search_text = logic.get_search_text().to_string().to_lowercase();
    let mut ui_items: Vec<InstanceData> = configs
        .iter()
        .filter(|c| search_text.is_empty() || c.name.to_lowercase().contains(&search_text))
        .map(|c| {
            let mut item = config_to_ui_data(c);
            if running_ids.contains(&c.id) {
                item.status = "running".into();
            }
            item
        })
        .collect();
    if ui_items.is_empty() {
        ui_items.push(InstanceData {
            id: "".into(),
            name: "".into(),
            version: "".into(),
            mod_loader: "".into(),
            last_played: "".into(),
            play_time: "".into(),
            image: Default::default(),
            status: "".into(),
        });
    }
    logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(ui_items))));
}

#[allow(unused)]
pub async fn open_view() -> anyhow::Result<()> {
    let ui = MainApp::new()?;
    let logic = ui.global::<InstanceLogic>();

    let store = Arc::new(Mutex::new(InstanceStore::new(
        McPaths::new()?.instances_base_dir(),
    )));
    {
        let boot_settings = AppSettings::load();
        store
            .lock()
            .unwrap()
            .set_sort(boot_settings.sort_mode, boot_settings.sort_ascending);
    }
    let master_configs: Arc<Mutex<Vec<InstanceConfig>>> = {
        let loaded = store.lock().unwrap().load().unwrap_or_default();
        Arc::new(Mutex::new(loaded))
    };
    {
        let configs = master_configs.lock().unwrap();
        let ui_items: Vec<InstanceData> = configs.iter().map(config_to_ui_data).collect();
        logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(ui_items))));
    }

    let running_procs: Arc<Mutex<HashMap<String, Child>>> = Arc::new(Mutex::new(HashMap::new()));
    let launching_procs: Arc<Mutex<std::collections::HashSet<String>>> =
        Arc::new(Mutex::new(std::collections::HashSet::new()));
    let instance_logs: Arc<Mutex<HashMap<String, VecDeque<crate::view::LogLine>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let (_debouncer, rx) = store.lock().unwrap().watch_changes()?;
    let ui_weak_for_watch = ui.as_weak();
    let store_for_watch = Arc::clone(&store);
    let master_for_watch = Arc::clone(&master_configs);
    let running_procs_for_watch = Arc::clone(&running_procs);
    tokio::spawn(async move {
        while rx.recv().is_ok() {
            info!("Detected instance.toml change, syncing to UI list...");
            let latest_configs = match store_for_watch.lock() {
                Ok(s) => s.load().unwrap_or_default(),
                Err(_) => continue,
            };
            if let Ok(mut master) = master_for_watch.lock() {
                *master = latest_configs.clone();
            }
            let running_ids: std::collections::HashSet<String> = running_procs_for_watch
                .lock()
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            let ui_weak = ui_weak_for_watch.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui_handle) = ui_weak.upgrade() {
                    let logic = ui_handle.global::<InstanceLogic>();
                    refresh_instance_list(&logic, &latest_configs, &running_ids);
                    info!("UI list securely synced with disk");
                }
            });
        }
    });

    let mc_versions_cache = Arc::new(std::sync::Mutex::new(
        Vec::<crate::mc_types::McVersion>::new(),
    ));

    let ui_weak_for_versions = ui.as_weak();
    {
        if let Some(ui) = ui_weak_for_versions.upgrade() {
            ui.global::<InstanceCreateLogic>().set_is_loading(true);
        }
    }

    let cache_for_fetch = Arc::clone(&mc_versions_cache);
    let ui_weak_for_fetch = ui.as_weak();
    tokio::spawn(async move {
        let api = crate::mc_api::McAction::new();
        match api.get_mc_manifest().await {
            Ok(manifest) => {
                let latest_release = manifest.latest.release.clone();
                *cache_for_fetch.lock().unwrap() = manifest.versions;
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_for_fetch.upgrade() {
                        let create = ui.global::<InstanceCreateLogic>();
                        create.set_is_loading(false);
                        create.set_version_load_error("".into());
                        create.set_latest_release_id(latest_release.into());
                        create.invoke_filter_versions();
                    }
                })
                .ok();
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to fetch MC versions");
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_for_fetch.upgrade() {
                        let create = ui.global::<InstanceCreateLogic>();
                        create.set_is_loading(false);
                        create.set_version_load_error(
                            "Failed to load Minecraft versions. Please check your network and restart."
                                .into(),
                        );
                    }
                })
                .ok();
            }
        }
    });

    let cache_for_filter = Arc::clone(&mc_versions_cache);
    let ui_weak_for_filter = ui.as_weak();
    ui.global::<InstanceCreateLogic>()
        .on_filter_versions(move || {
            let Some(ui) = ui_weak_for_filter.upgrade() else {
                return;
            };
            let logic = ui.global::<InstanceCreateLogic>();

            let versions = cache_for_filter.lock().unwrap();
            let search_text = logic.get_version_search_text().to_string().to_lowercase();
            let show_release = logic.get_show_release();
            let show_snapshot = logic.get_show_snapshot();
            let show_beta = logic.get_show_beta();
            let show_alpha = logic.get_show_alpha();
            let show_experimental = logic.get_show_experimental();

            let mut filtered: Vec<&crate::mc_types::McVersion> = versions
                .iter()
                .filter(|v| {
                    if !search_text.is_empty() {
                        let id = v.id.to_lowercase();
                        let is_prefix_match =
                            id == search_text || id.starts_with(&format!("{search_text}."));
                        if !is_prefix_match {
                            return false;
                        }
                    }

                    match v.r#type.as_str() {
                        "release" => show_release,
                        "snapshot" => show_snapshot,
                        "old_beta" => show_beta,
                        "old_alpha" => show_alpha,
                        "experimental" | "pending" => show_experimental,
                        _ => show_experimental,
                    }
                })
                .collect();

            filtered.sort_by(|a, b| b.release_time.cmp(&a.release_time));

            let filtered: Vec<slint::SharedString> =
                filtered.into_iter().map(|v| v.id.clone().into()).collect();

            let current_selected = logic.get_selected_version().to_string();
            let found = !current_selected.is_empty()
                && filtered.iter().any(|f| f.as_str() == current_selected);

            if !found {
                let priority: &[(&str, bool)] = &[
                    ("release", show_release),
                    ("snapshot", show_snapshot),
                    ("old_beta", show_beta),
                    ("old_alpha", show_alpha),
                    ("experimental", show_experimental),
                    ("pending", show_experimental),
                ];
                let best = priority
                    .iter()
                    .filter(|(_, enabled)| *enabled)
                    .find_map(|(type_str, _)| {
                        versions
                            .iter()
                            .find(|v| v.r#type.as_str() == *type_str)
                            .and_then(|v| {
                                filtered
                                    .iter()
                                    .find(|f| f.as_str() == v.id.as_str())
                                    .cloned()
                            })
                    })
                    .or_else(|| filtered.first().cloned())
                    .unwrap_or_default();
                logic.set_selected_version(best);
            }

            let selected_now = logic.get_selected_version();
            let selected_idx = filtered
                .iter()
                .position(|f| f.as_str() == selected_now.as_str())
                .map(|i| i as i32)
                .unwrap_or(-1);
            logic.set_selected_version_index(selected_idx);

            logic.set_version_list(ModelRc::from(Rc::new(VecModel::from(filtered))));
            logic.invoke_loader_changed();
        });

    let ui_weak_for_net = ui.as_weak();
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("Failed to create network detection client");
        loop {
            let online = client
                .head("https://launchermeta.mojang.com/mc/game/version_manifest_v2.json")
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false);
            let ui_weak = ui_weak_for_net.clone();
            if slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    let app = ui.global::<AppData>();
                    app.set_network_checking(false);
                    app.set_is_online(online);
                }
            })
            .is_err()
            {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
        }
    });

    {
        let edit_items: Vec<slint::SharedString> = vec![
            JAVA_MODE_LABEL_GLOBAL.into(),
            JAVA_MODE_LABEL_MINECRAFT.into(),
            JAVA_MODE_LABEL_CUSTOM.into(),
        ];
        ui.global::<InstanceEditLogic>()
            .set_java_mode_list(ModelRc::from(Rc::new(VecModel::from(edit_items))));

        let settings_items: Vec<slint::SharedString> = vec![
            JAVA_MODE_LABEL_MINECRAFT.into(),
            JAVA_MODE_LABEL_CUSTOM.into(),
        ];
        let app_settings = AppSettings::load();
        let sl = ui.global::<SettingsLogic>();
        sl.set_java_mode_list(ModelRc::from(Rc::new(VecModel::from(settings_items))));
        sl.set_selected_java_mode(java_mode_to_label(&app_settings.java_mode, false).into());
        sl.set_java_path(app_settings.java_path.as_str().into());

        let sort_items: Vec<slint::SharedString> = vec![
            SORT_LABEL_NAME.into(),
            SORT_LABEL_VERSION.into(),
            SORT_LABEL_CREATED_AT.into(),
            SORT_LABEL_LAST_PLAYED.into(),
        ];
        sl.set_sort_mode_list(ModelRc::from(Rc::new(VecModel::from(sort_items))));
        sl.set_selected_sort_mode(sort_mode_to_label(app_settings.sort_mode).into());
        sl.set_sort_ascending(app_settings.sort_ascending);
    }

    let store_for_sort = Arc::clone(&store);
    let master_for_sort = Arc::clone(&master_configs);
    let running_for_sort = Arc::clone(&running_procs);
    let ui_weak_for_sort = ui.as_weak();
    ui.global::<SettingsLogic>().on_sort_changed(move || {
        let Some(ui) = ui_weak_for_sort.upgrade() else {
            return;
        };
        let sl = ui.global::<SettingsLogic>();
        let mode = label_to_sort_mode(sl.get_selected_sort_mode().as_str());
        let ascending = sl.get_sort_ascending();

        let mut settings = AppSettings::load();
        settings.sort_mode = mode;
        settings.sort_ascending = ascending;
        if let Err(e) = settings.save() {
            warn!(error = %e, "Failed to save sort settings");
        }

        let reloaded = {
            let mut store = store_for_sort.lock().unwrap();
            store.set_sort(mode, ascending);
            store.load().unwrap_or_default()
        };
        *master_for_sort.lock().unwrap() = reloaded.clone();

        let logic = ui.global::<InstanceLogic>();
        let running_ids: std::collections::HashSet<String> =
            running_for_sort.lock().unwrap().keys().cloned().collect();
        refresh_instance_list(&logic, &reloaded, &running_ids);
    });

    let ui_weak_for_scan = ui.as_weak();
    tokio::spawn(async move {
        let javas = tokio::task::spawn_blocking(crate::java_scan::scan_system_javas)
            .await
            .unwrap_or_default();
        info!(count = javas.len(), "System Java scan completed");
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = ui_weak_for_scan.upgrade() else {
                return;
            };
            let items: Vec<slint::SharedString> = javas.iter().map(|p| p.as_str().into()).collect();
            ui.global::<InstanceEditLogic>()
                .set_detected_java_list(ModelRc::from(Rc::new(VecModel::from(items.clone()))));
            ui.global::<SettingsLogic>()
                .set_detected_java_list(ModelRc::from(Rc::new(VecModel::from(items))));
        });
    });

    // rfd 在 Linux 用 xdg-portal 後端（免 GTK 依賴，AppImage 友善）僅提供 async API，故用 spawn_local 等待
    let ui_weak_for_edit_browse = ui.as_weak();
    ui.global::<InstanceEditLogic>().on_browse_java(move || {
        let ui_weak = ui_weak_for_edit_browse.clone();
        let _ = slint::spawn_local(async move {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .set_title("Select Java Executable")
                .pick_file()
                .await
                && let Some(ui) = ui_weak.upgrade()
            {
                ui.global::<InstanceEditLogic>()
                    .set_java_path(file.path().display().to_string().into());
            }
        });
    });

    let ui_weak_for_settings_browse = ui.as_weak();
    ui.global::<SettingsLogic>().on_browse_java(move || {
        let ui_weak = ui_weak_for_settings_browse.clone();
        let _ = slint::spawn_local(async move {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .set_title("Select Java Executable")
                .pick_file()
                .await
                && let Some(ui) = ui_weak.upgrade()
            {
                ui.global::<SettingsLogic>()
                    .set_java_path(file.path().display().to_string().into());
            }
        });
    });

    launch::setup_launch_logic(
        &ui,
        Arc::clone(&store),
        Arc::clone(&master_configs),
        Arc::clone(&running_procs),
        Arc::clone(&launching_procs),
        Arc::clone(&instance_logs),
    );

    instance_detail::setup_instance_detail_logic(
        &ui,
        Arc::clone(&store),
        Arc::clone(&master_configs),
        Arc::clone(&running_procs),
        Arc::clone(&instance_logs),
    );

    create::setup_create_logic(
        &ui,
        Arc::clone(&store),
        Arc::clone(&master_configs),
        Arc::clone(&running_procs),
    );

    let mod_logic = ui.global::<ModLogic>();
    let raw_mods: Vec<ModData> = mod_logic.get_mod_list().iter().collect();
    let mod_logic_weak = ui.as_weak();
    mod_logic.on_search_changed(move |text| {
        let ui = mod_logic_weak.unwrap();
        let logic = ui.global::<ModLogic>();
        let filtered: Vec<ModData> = raw_mods
            .iter()
            .filter(|m| {
                text.is_empty() || m.name.to_lowercase().contains(text.to_lowercase().as_str())
            })
            .cloned()
            .collect();
        logic.set_selected_index(0);
        logic.set_mod_list(ModelRc::from(Rc::new(VecModel::from(filtered))));
    });

    let applogic = ui.global::<AppLogic>();
    applogic.on_sidebar_change(|id| {
        debug!(tab = ?id, "sidebar switch");
    });

    login::setup_login_logic(&ui);

    appearance::setup_appearance_window(&ui);

    slint::select_bundled_translation("en_US").unwrap();
    ui.run()?;
    Ok(())
}

fn set_install_state(
    ui_weak: &slint::Weak<MainApp>,
    installing: bool,
    progress: f32,
    status: &str,
    is_error: bool,
) {
    let status = status.to_string();
    let ui_weak = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        let Some(ui) = ui_weak.upgrade() else { return };
        let logic = ui.global::<InstanceLogic>();
        logic.set_is_installing(installing);
        logic.set_install_progress(progress);
        logic.set_install_status(status.into());
        logic.set_install_is_error(is_error);
    });
}

fn set_instance_status(ui_weak: &slint::Weak<MainApp>, instance_id: &str, status: &str) {
    let id = instance_id.to_string();
    let status = status.to_string();
    let ui_weak = ui_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        let Some(ui) = ui_weak.upgrade() else { return };
        let detail = ui.global::<InstanceDetailLogic>();
        if detail.get_instance_id().as_str() == id {
            detail.set_instance_running(status == "running");
        }
        let list = ui.global::<InstanceLogic>().get_instance_list();
        for i in 0..list.row_count() {
            if i >= list.row_count() {
                break;
            }
            if let Some(mut item) = list.row_data(i)
                && item.id.as_str() == id
            {
                item.status = status.into();
                if i < list.row_count() {
                    list.set_row_data(i, item);
                }
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_java_mode_to_label() {
        assert_eq!(
            java_mode_to_label(JAVA_MODE_CUSTOM, true),
            JAVA_MODE_LABEL_CUSTOM
        );
        assert_eq!(
            java_mode_to_label(JAVA_MODE_MINECRAFT, true),
            JAVA_MODE_LABEL_MINECRAFT
        );
        assert_eq!(
            java_mode_to_label(JAVA_MODE_GLOBAL, true),
            JAVA_MODE_LABEL_GLOBAL
        );
        assert_eq!(java_mode_to_label("", true), JAVA_MODE_LABEL_GLOBAL);

        assert_eq!(
            java_mode_to_label(JAVA_MODE_CUSTOM, false),
            JAVA_MODE_LABEL_CUSTOM
        );
        assert_eq!(
            java_mode_to_label(JAVA_MODE_MINECRAFT, false),
            JAVA_MODE_LABEL_MINECRAFT
        );
        assert_eq!(
            java_mode_to_label(JAVA_MODE_GLOBAL, false),
            JAVA_MODE_LABEL_MINECRAFT
        );
        assert_eq!(java_mode_to_label("", false), JAVA_MODE_LABEL_MINECRAFT);
    }

    #[test]
    fn test_java_label_to_mode() {
        assert_eq!(java_label_to_mode(JAVA_MODE_LABEL_CUSTOM), JAVA_MODE_CUSTOM);
        assert_eq!(
            java_label_to_mode(JAVA_MODE_LABEL_MINECRAFT),
            JAVA_MODE_MINECRAFT
        );
        assert_eq!(java_label_to_mode(JAVA_MODE_LABEL_GLOBAL), JAVA_MODE_GLOBAL);
        assert_eq!(java_label_to_mode("unknown"), JAVA_MODE_GLOBAL);
    }
}
