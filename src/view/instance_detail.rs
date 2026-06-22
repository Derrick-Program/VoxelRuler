use std::path::{Path, PathBuf};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use slint::{ModelRc, SharedString, VecModel, Weak, Model, ComponentHandle};
use crate::view::*;

use crate::view::{MainApp, InstanceFileEntry};
use tracing::error;

pub(crate) fn append_log_line(
    model: &ModelRc<slint::SharedString>,
    line: slint::SharedString,
) -> Option<ModelRc<slint::SharedString>> {
    if let Some(vec_model) = model
        .as_any()
        .downcast_ref::<VecModel<slint::SharedString>>()
    {
        vec_model.push(line);
        None
    } else {
        let mut lines: Vec<slint::SharedString> = (0..model.row_count())
            .filter_map(|i| model.row_data(i))
            .collect();
        lines.push(line);
        Some(ModelRc::from(Rc::new(VecModel::from(lines))))
    }
}

/// 詳細視窗 category key → 實際資料夾
pub(crate) fn detail_category_dir(paths: &McPaths, instance_id: &str, category: &str) -> PathBuf {
    let root = paths.instance_dir(instance_id);
    match category {
        "root" => root,
        "worlds" => root.join("saves"),
        other => root.join(other),
    }
}

/// category key → 所屬分頁索引（操作後重新整理用）
pub(crate) fn detail_category_tab(category: &str) -> i32 {
    match category {
        "mods" => 2,
        "resourcepacks" => 3,
        "shaderpacks" => 4,
        "saves" | "worlds" => 6,
        "screenshots" => 8,
        _ => -1,
    }
}

pub(crate) fn shared_model(items: Vec<slint::SharedString>) -> ModelRc<slint::SharedString> {
    ModelRc::from(Rc::new(VecModel::from(items)))
}

pub(crate) fn file_entries_model(dir: &Path, exts: &[&str], allow_dirs: bool) -> ModelRc<InstanceFileEntry> {
    let items: Vec<InstanceFileEntry> = crate::instance_assets::list_entries(dir, exts, allow_dirs)
        .into_iter()
        .map(|e| InstanceFileEntry {
            file_name: e.file_name.as_str().into(),
            info: e.info.as_str().into(),
            enabled: e.enabled,
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(items)))
}

/// 載入詳細視窗指定分頁的資料（必須在 UI 執行緒呼叫）
pub(crate) fn load_detail_tab(
    ui: &MainApp,
    instance_id: &str,
    tab: i32,
    instance_logs: &Arc<Mutex<HashMap<String, VecDeque<String>>>>,
) {
    let Ok(paths) = McPaths::new() else { return };
    let dir = paths.instance_dir(instance_id);
    let detail = ui.global::<InstanceDetailLogic>();
    detail.set_pending_delete_key("".into());
    match tab {
        // Minecraft 紀錄檔（live，先帶入目前 buffer，後續由 log reader 增量 push）
        0 => {
            let lines: Vec<slint::SharedString> = instance_logs
                .lock()
                .unwrap()
                .get(instance_id)
                .map(|d| d.iter().map(|s| s.as_str().into()).collect())
                .unwrap_or_default();
            detail.set_live_log_lines(shared_model(lines));
        }
        2 => detail.set_mods(file_entries_model(&dir.join("mods"), &[".jar"], false)),
        3 => detail.set_resource_packs(file_entries_model(
            &dir.join("resourcepacks"),
            &[".zip"],
            true,
        )),
        4 => detail.set_shader_packs(file_entries_model(
            &dir.join("shaderpacks"),
            &[".zip"],
            true,
        )),
        5 => {
            detail.set_notes(crate::instance_assets::read_notes(&dir).as_str().into());
            detail.set_notes_status("".into());
        }
        6 => {
            let rows: Vec<WorldRow> = crate::instance_assets::list_worlds(&dir.join("saves"))
                .into_iter()
                .map(|w| WorldRow {
                    dir_name: w.dir_name.as_str().into(),
                    level_name: w.level_name.as_str().into(),
                    info: w.info.as_str().into(),
                })
                .collect();
            detail.set_worlds(ModelRc::from(Rc::new(VecModel::from(rows))));
        }
        7 => {
            let rows: Vec<ServerRow> =
                crate::instance_assets::read_servers(&dir.join("servers.dat"))
                    .unwrap_or_default()
                    .into_iter()
                    .map(|s| ServerRow {
                        name: s.name.as_str().into(),
                        ip: s.ip.as_str().into(),
                    })
                    .collect();
            detail.set_servers(ModelRc::from(Rc::new(VecModel::from(rows))));
        }
        8 => {
            let rows: Vec<ScreenshotRow> =
                crate::instance_assets::list_screenshots(&dir.join("screenshots"), 24)
                    .into_iter()
                    .map(|p| ScreenshotRow {
                        file_name: p
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default()
                            .as_str()
                            .into(),
                        thumb: slint::Image::load_from_path(&p).unwrap_or_default(),
                    })
                    .collect();
            detail.set_screenshots(ModelRc::from(Rc::new(VecModel::from(rows))));
        }
        10 => {
            let names: Vec<slint::SharedString> =
                crate::instance_assets::list_log_files(&dir.join("logs"))
                    .iter()
                    .map(|s| s.as_str().into())
                    .collect();
            detail.set_other_logs(shared_model(names));
            detail.set_viewing_log_name("".into());
            detail.set_other_log_content(shared_model(Vec::new()));
        }
        _ => {}
    }
}

/// 開啟詳細視窗：填基本資料 + 設定分頁（InstanceEditLogic）+ 載入指定分頁
pub(crate) fn open_instance_detail(
    ui: &MainApp,
    master_configs: &Arc<Mutex<Vec<InstanceConfig>>>,
    running_procs: &Arc<Mutex<HashMap<String, Child>>>,
    instance_logs: &Arc<Mutex<HashMap<String, VecDeque<String>>>>,
    id: &str,
    tab: i32,
) {
    let config = {
        let configs = master_configs.lock().unwrap();
        configs.iter().find(|c| c.id == id).cloned()
    };
    let Some(c) = config else { return };

    // 設定分頁沿用 InstanceEditLogic（記憶體 / Java）
    let edit = ui.global::<InstanceEditLogic>();
    edit.set_instance_id(c.id.as_str().into());
    edit.set_instance_name(c.name.as_str().into());
    edit.set_xmx(c.xmx.as_str().into());
    edit.set_xms(c.xms.as_str().into());
    edit.set_java_path(c.java_path.as_str().into());
    edit.set_selected_java_mode(java_mode_to_label(&c.java_mode, true).into());
    edit.set_error_msg("".into());

    let detail = ui.global::<InstanceDetailLogic>();
    detail.set_instance_id(c.id.as_str().into());
    detail.set_instance_name(c.name.as_str().into());
    detail.set_version(c.version.as_str().into());
    detail.set_mod_loader(c.mod_loader.as_str().into());
    detail.set_selected_version(c.version.as_str().into());
    // 版本清單與「建立實例」對話框共用（啟動時已從 Mojang 取得）
    detail.set_version_list(ui.global::<InstanceCreateLogic>().get_version_list());
    detail.set_instance_running(running_procs.lock().unwrap().contains_key(id));
    detail.set_status_msg("".into());
    detail.set_active_tab(tab);
    load_detail_tab(ui, id, tab, instance_logs);
    detail.set_show_dialog(true);
}

pub(crate) fn spawn_log_reader<R: std::io::Read + Send + 'static>(
    reader: R,
    instance_id: String,
    instance_logs: Arc<Mutex<HashMap<String, VecDeque<String>>>>,
    ui_weak: slint::Weak<MainApp>,
) {
    tokio::task::spawn_blocking(move || {
        use std::io::BufRead;
        let buf = std::io::BufReader::new(reader);
        for line in buf.lines().map_while(Result::ok) {
            debug!(instance = %instance_id, "[Java] {}", line);
            {
                let mut logs = instance_logs.lock().unwrap();
                if let Some(deque) = logs.get_mut(&instance_id) {
                    if deque.len() >= 500 {
                        deque.pop_front();
                    }
                    deque.push_back(line.clone());
                }
            }
            let id = instance_id.clone();
            let line_shared: slint::SharedString = line.into();
            let ui = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui_handle) = ui.upgrade() else {
                    return;
                };
                let logic = ui_handle.global::<InstanceLogic>();
                if logic.get_show_log()
                    && logic.get_log_instance_id().as_str() == id
                    && let Some(new_model) =
                        append_log_line(&logic.get_log_lines(), line_shared.clone())
                {
                    logic.set_log_lines(new_model);
                }
                // 詳細視窗的「Minecraft 紀錄檔」分頁（live）
                let detail = ui_handle.global::<InstanceDetailLogic>();
                if detail.get_show_dialog()
                    && detail.get_active_tab() == 0
                    && detail.get_instance_id().as_str() == id
                    && let Some(new_model) =
                        append_log_line(&detail.get_live_log_lines(), line_shared)
                {
                    detail.set_live_log_lines(new_model);
                }
            });
        }
    });
}
