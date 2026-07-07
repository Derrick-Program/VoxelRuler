use crate::view::*;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel, Weak};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::view::{InstanceFileEntry, MainApp};
use tracing::error;

pub(crate) fn append_log_line(
    model: &ModelRc<crate::view::LogLine>,
    line: crate::view::LogLine,
) -> Option<ModelRc<crate::view::LogLine>> {
    if let Some(vec_model) = model
        .as_any()
        .downcast_ref::<VecModel<crate::view::LogLine>>()
    {
        vec_model.push(line);
        None
    } else {
        let mut lines: Vec<crate::view::LogLine> = (0..model.row_count())
            .filter_map(|i| model.row_data(i))
            .collect();
        lines.push(line);
        Some(ModelRc::from(Rc::new(VecModel::from(lines))))
    }
}

use std::sync::OnceLock;
pub(crate) fn parse_ansi_log_line(line: &str) -> crate::view::LogLine {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"\x1B\[([0-9;]*)[mK]").unwrap());

    let mut color = slint::Color::from_rgb_u8(197, 200, 198);
    let mut has_ansi = false;

    if let Some(caps) = re.captures(line) {
        has_ansi = true;
        let codes = caps.get(1).map_or("", |m| m.as_str());
        for code in codes.split(';') {
            match code {
                "31" | "91" => color = slint::Color::from_rgb_u8(231, 76, 60),
                "33" | "93" => color = slint::Color::from_rgb_u8(241, 196, 15),
                "32" | "92" => color = slint::Color::from_rgb_u8(46, 204, 113),
                "36" | "96" => color = slint::Color::from_rgb_u8(26, 188, 156),
                "35" | "95" => color = slint::Color::from_rgb_u8(155, 89, 182),
                "34" | "94" => color = slint::Color::from_rgb_u8(52, 152, 219),
                _ => {}
            }
        }
    }

    if !has_ansi {
        if line.contains("/ERROR]")
            || line.contains(" ERROR ")
            || line.starts_with("Exception")
            || line.contains("Exception:")
            || line.starts_with("\tat ")
        {
            color = slint::Color::from_rgb_u8(231, 76, 60);
        } else if line.contains("/WARN]") || line.contains(" WARN ") {
            color = slint::Color::from_rgb_u8(241, 196, 15);
        } else if line.contains("/DEBUG]") || line.contains(" DEBUG ") {
            color = slint::Color::from_rgb_u8(127, 140, 141);
        } else if line.contains("/FATAL]") || line.contains(" FATAL ") {
            color = slint::Color::from_rgb_u8(192, 57, 43);
        }
    }

    let text = re.replace_all(line, "").to_string();
    crate::view::LogLine {
        text: text.into(),
        color,
    }
}

pub(crate) fn detail_category_dir(paths: &McPaths, instance_id: &str, category: &str) -> PathBuf {
    let root = paths.instance_dir(instance_id);
    match category {
        "root" => root,
        "worlds" => root.join("saves"),
        other => root.join(other),
    }
}

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

pub(crate) fn shared_log_model(items: Vec<crate::view::LogLine>) -> ModelRc<crate::view::LogLine> {
    ModelRc::from(Rc::new(VecModel::from(items)))
}

pub(crate) fn file_entries_model(
    dir: &Path,
    exts: &[&str],
    allow_dirs: bool,
) -> ModelRc<InstanceFileEntry> {
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

pub(crate) fn load_detail_tab(
    ui: &MainApp,
    instance_id: &str,
    tab: i32,
    instance_logs: &Arc<Mutex<HashMap<String, VecDeque<crate::view::LogLine>>>>,
) {
    let Ok(paths) = McPaths::new() else { return };
    let dir = paths.instance_dir(instance_id);
    let detail = ui.global::<InstanceDetailLogic>();
    detail.set_pending_delete_key("".into());
    match tab {
        0 => {
            let lines: Vec<crate::view::LogLine> = instance_logs
                .lock()
                .unwrap()
                .get(instance_id)
                .map(|d| d.iter().cloned().collect())
                .unwrap_or_default();
            detail.set_live_log_lines(shared_log_model(lines));
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

pub(crate) fn open_instance_detail(
    ui: &MainApp,
    master_configs: &Arc<Mutex<Vec<InstanceConfig>>>,
    running_procs: &Arc<Mutex<HashMap<String, Child>>>,
    instance_logs: &Arc<Mutex<HashMap<String, VecDeque<crate::view::LogLine>>>>,
    id: &str,
    tab: i32,
) {
    let config = {
        let configs = master_configs.lock().unwrap();
        configs.iter().find(|c| c.id == id).cloned()
    };
    let Some(c) = config else { return };

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
    instance_logs: Arc<Mutex<HashMap<String, VecDeque<crate::view::LogLine>>>>,
    ui_weak: slint::Weak<MainApp>,
) {
    // must be a plain OS thread, not tokio::spawn_blocking: this read blocks until the
    // game process exits, and a blocking-pool task would stall runtime shutdown while the game keeps running
    std::thread::spawn(move || {
        use std::io::BufRead;
        let buf = std::io::BufReader::new(reader);
        for line in buf.lines().map_while(Result::ok) {
            debug!(instance = %instance_id, "[Java] {}", line);
            let parsed_line = parse_ansi_log_line(&line);
            {
                let mut logs = instance_logs.lock().unwrap();
                if let Some(deque) = logs.get_mut(&instance_id) {
                    if deque.len() >= 500 {
                        deque.pop_front();
                    }
                    deque.push_back(parsed_line.clone());
                }
            }
            let id = instance_id.clone();
            let ui = ui_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui_handle) = ui.upgrade() else {
                    return;
                };
                let logic = ui_handle.global::<InstanceLogic>();
                if logic.get_show_log()
                    && logic.get_log_instance_id().as_str() == id
                    && let Some(new_model) =
                        append_log_line(&logic.get_log_lines(), parsed_line.clone())
                {
                    logic.set_log_lines(new_model);
                }
                let detail = ui_handle.global::<InstanceDetailLogic>();
                if detail.get_show_dialog()
                    && detail.get_active_tab() == 0
                    && detail.get_instance_id().as_str() == id
                    && let Some(new_model) =
                        append_log_line(&detail.get_live_log_lines(), parsed_line)
                {
                    detail.set_live_log_lines(new_model);
                }
            });
        }
    });
}

pub fn setup_instance_detail_logic(
    ui: &MainApp,
    store: std::sync::Arc<std::sync::Mutex<crate::mc_instance::InstanceStore>>,
    master_configs: std::sync::Arc<std::sync::Mutex<Vec<crate::mc_instance::InstanceConfig>>>,
    running_procs: std::sync::Arc<
        std::sync::Mutex<std::collections::HashMap<String, std::process::Child>>,
    >,
    instance_logs: std::sync::Arc<
        std::sync::Mutex<
            std::collections::HashMap<String, std::collections::VecDeque<crate::view::LogLine>>,
        >,
    >,
) {
    let detail_logic = ui.global::<InstanceDetailLogic>();

    let master_for_detail_open = Arc::clone(&master_configs);
    let running_for_detail_open = Arc::clone(&running_procs);
    let logs_for_detail_open = Arc::clone(&instance_logs);
    let ui_weak_for_detail_open = ui.as_weak();
    detail_logic.on_open_detail(move |id, tab| {
        let Some(ui) = ui_weak_for_detail_open.upgrade() else {
            return;
        };
        open_instance_detail(
            &ui,
            &master_for_detail_open,
            &running_for_detail_open,
            &logs_for_detail_open,
            id.as_str(),
            tab,
        );
    });

    let ui_weak_for_detail_close = ui.as_weak();
    detail_logic.on_close_detail(move || {
        if let Some(ui) = ui_weak_for_detail_close.upgrade() {
            ui.global::<InstanceDetailLogic>().set_show_dialog(false);
        }
    });

    let logs_for_tab = Arc::clone(&instance_logs);
    let ui_weak_for_tab = ui.as_weak();
    detail_logic.on_tab_changed(move |tab| {
        let Some(ui) = ui_weak_for_tab.upgrade() else {
            return;
        };
        let id = ui
            .global::<InstanceDetailLogic>()
            .get_instance_id()
            .to_string();
        load_detail_tab(&ui, &id, tab, &logs_for_tab);
    });

    let ui_weak_for_subfolder = ui.as_weak();
    detail_logic.on_open_subfolder(move |category| {
        let Some(ui) = ui_weak_for_subfolder.upgrade() else {
            return;
        };
        let id = ui
            .global::<InstanceDetailLogic>()
            .get_instance_id()
            .to_string();
        let Ok(paths) = McPaths::new() else { return };
        let dir = detail_category_dir(&paths, &id, category.as_str());
        let _ = std::fs::create_dir_all(&dir);
        let _ = open::that(dir);
    });

    let logs_for_toggle = Arc::clone(&instance_logs);
    let ui_weak_for_toggle = ui.as_weak();
    detail_logic.on_toggle_entry(move |category, file_name| {
        let Some(ui) = ui_weak_for_toggle.upgrade() else {
            return;
        };
        let detail = ui.global::<InstanceDetailLogic>();
        let id = detail.get_instance_id().to_string();
        let Ok(paths) = McPaths::new() else { return };
        let dir = detail_category_dir(&paths, &id, category.as_str());
        match crate::instance_assets::toggle_disabled(&dir, file_name.as_str()) {
            Ok(_) => detail.set_status_msg("".into()),
            Err(e) => detail.set_status_msg(format!("{e}").into()),
        }
        load_detail_tab(
            &ui,
            &id,
            detail_category_tab(category.as_str()),
            &logs_for_toggle,
        );
    });

    let logs_for_delete_entry = Arc::clone(&instance_logs);
    let ui_weak_for_delete_entry = ui.as_weak();
    detail_logic.on_delete_entry(move |category, file_name| {
        let Some(ui) = ui_weak_for_delete_entry.upgrade() else {
            return;
        };
        let detail = ui.global::<InstanceDetailLogic>();
        let id = detail.get_instance_id().to_string();
        let Ok(paths) = McPaths::new() else { return };
        let dir = detail_category_dir(&paths, &id, category.as_str());
        match crate::instance_assets::delete_entry(&dir, file_name.as_str()) {
            Ok(()) => detail.set_status_msg("".into()),
            Err(e) => detail.set_status_msg(format!("{e}").into()),
        }
        load_detail_tab(
            &ui,
            &id,
            detail_category_tab(category.as_str()),
            &logs_for_delete_entry,
        );
    });

    let logs_for_add = Arc::clone(&instance_logs);
    let ui_weak_for_add = ui.as_weak();
    detail_logic.on_add_entry(move |category| {
        let ui_weak = ui_weak_for_add.clone();
        let logs = Arc::clone(&logs_for_add);
        let category = category.to_string();
        let _ = slint::spawn_local(async move {
            let mut dialog = rfd::AsyncFileDialog::new().set_title("Select file to add");
            dialog = match category.as_str() {
                "mods" => dialog.add_filter("Minecraft Mod", &["jar"]),
                "resourcepacks" | "shaderpacks" => dialog.add_filter("Pack", &["zip"]),
                _ => dialog,
            };
            let Some(files) = dialog.pick_files().await else {
                return;
            };
            let Some(ui) = ui_weak.upgrade() else { return };
            let detail = ui.global::<InstanceDetailLogic>();
            let id = detail.get_instance_id().to_string();
            let Ok(paths) = McPaths::new() else { return };
            let dir = detail_category_dir(&paths, &id, &category);
            for f in files {
                if let Err(e) = crate::instance_assets::add_file(&dir, f.path()) {
                    detail.set_status_msg(format!("{e}").into());
                }
            }
            load_detail_tab(&ui, &id, detail_category_tab(&category), &logs);
        });
    });

    let ui_weak_for_notes = ui.as_weak();
    detail_logic.on_save_notes(move || {
        let Some(ui) = ui_weak_for_notes.upgrade() else {
            return;
        };
        let detail = ui.global::<InstanceDetailLogic>();
        let id = detail.get_instance_id().to_string();
        let Ok(paths) = McPaths::new() else { return };
        match crate::instance_assets::save_notes(
            &paths.instance_dir(&id),
            detail.get_notes().as_str(),
        ) {
            Ok(()) => detail.set_notes_status("✓ Saved".into()),
            Err(e) => detail.set_notes_status(format!("{e}").into()),
        }
    });

    let ui_weak_for_view_log = ui.as_weak();
    detail_logic.on_view_log_file(move |name| {
        let Some(ui) = ui_weak_for_view_log.upgrade() else {
            return;
        };
        let detail = ui.global::<InstanceDetailLogic>();
        let id = detail.get_instance_id().to_string();
        let Ok(paths) = McPaths::new() else { return };
        let logs_dir = paths.instance_dir(&id).join("logs");
        detail.set_viewing_log_name(name.clone());
        match crate::instance_assets::read_log_lines(&logs_dir, name.as_str(), 2000) {
            Ok(lines) => {
                let shared: Vec<slint::SharedString> =
                    lines.iter().map(|s| s.as_str().into()).collect();
                detail.set_other_log_content(ModelRc::from(Rc::new(VecModel::from(shared))));
            }
            Err(e) => {
                let msg: Vec<slint::SharedString> = vec![format!("Read failed: {e}").into()];
                detail.set_other_log_content(ModelRc::from(Rc::new(VecModel::from(msg))));
            }
        }
    });

    let store_for_version = Arc::clone(&store);
    let master_for_version = Arc::clone(&master_configs);
    let ui_weak_for_version_save = ui.as_weak();
    detail_logic.on_save_version(move || {
        let Some(ui) = ui_weak_for_version_save.upgrade() else {
            return;
        };
        let detail = ui.global::<InstanceDetailLogic>();
        let id = detail.get_instance_id().to_string();
        let new_version = detail.get_selected_version().to_string();
        if new_version.is_empty() {
            return;
        }
        let updated = {
            let mut master = master_for_version.lock().unwrap();
            let Some(c) = master.iter_mut().find(|c| c.id == id) else {
                return;
            };
            c.version = new_version.clone();
            c.clone()
        };
        match store_for_version.lock().unwrap().save_one(&updated) {
            Ok(()) => {
                detail.set_version(new_version.as_str().into());
                detail.set_status_msg("✓ Saved".into());
            }
            Err(e) => detail.set_status_msg(format!("Save failed: {e}").into()),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ansi_log_line() {
        let line = parse_ansi_log_line("\x1b[31mError message\x1b[0m");
        assert_eq!(line.text.as_str(), "Error message");
        assert_eq!(line.color.red(), 231);
        assert_eq!(line.color.green(), 76);
        assert_eq!(line.color.blue(), 60);

        let line_clean = parse_ansi_log_line("Just info");
        assert_eq!(line_clean.text.as_str(), "Just info");
        assert_eq!(line_clean.color.red(), 197);
    }

    #[test]
    fn test_detail_category_tab() {
        assert_eq!(detail_category_tab("mods"), 2);
        assert_eq!(detail_category_tab("resourcepacks"), 3);
        assert_eq!(detail_category_tab("shaderpacks"), 4);
        assert_eq!(detail_category_tab("saves"), 6);
        assert_eq!(detail_category_tab("worlds"), 6);
        assert_eq!(detail_category_tab("screenshots"), 8);
        assert_eq!(detail_category_tab("settings"), -1);
    }

    #[test]
    fn test_parse_ansi_log_line_keyword_fallback() {
        let err = parse_ansi_log_line("[12:00:00] [main/ERROR]: something broke");
        assert_eq!((err.color.red(), err.color.green()), (231, 76));

        let warn = parse_ansi_log_line("[12:00:00] [main/WARN]: heads up");
        assert_eq!((warn.color.red(), warn.color.green()), (241, 196));

        let stack = parse_ansi_log_line("\tat net.minecraft.client.main(Main.java:1)");
        assert_eq!(stack.color.red(), 231);
    }

    #[test]
    fn test_detail_category_dir_mapping() {
        let paths = McPaths::new().unwrap();
        let root = paths.instance_dir("abc");
        assert_eq!(detail_category_dir(&paths, "abc", "root"), root);
        assert_eq!(
            detail_category_dir(&paths, "abc", "worlds"),
            root.join("saves")
        );
        assert_eq!(
            detail_category_dir(&paths, "abc", "mods"),
            root.join("mods")
        );
    }

    fn log_line(text: &str) -> crate::view::LogLine {
        crate::view::LogLine {
            text: text.into(),
            color: slint::Color::from_rgb_u8(197, 200, 198),
        }
    }

    #[test]
    fn test_append_log_line_vecmodel_pushes_in_place() {
        let model: ModelRc<crate::view::LogLine> =
            ModelRc::from(Rc::new(VecModel::from(vec![log_line("first")])));
        let replaced = append_log_line(&model, log_line("second"));
        assert!(replaced.is_none());
        assert_eq!(model.row_count(), 2);
        assert_eq!(model.row_data(1).unwrap().text.as_str(), "second");
    }

    #[test]
    fn test_append_log_line_non_vecmodel_returns_new_model() {
        let inner = Rc::new(VecModel::from(vec![log_line("first")]));
        let filtered: ModelRc<crate::view::LogLine> =
            ModelRc::from(Rc::new(slint::FilterModel::new(inner, |_| true)));
        let replaced =
            append_log_line(&filtered, log_line("second")).expect("should return new model");
        assert_eq!(filtered.row_count(), 1);
        assert_eq!(replaced.row_count(), 2);
        assert_eq!(replaced.row_data(1).unwrap().text.as_str(), "second");
    }

    #[tokio::test]
    async fn test_spawn_log_reader_parses_and_stores_lines() {
        let logs: Arc<Mutex<HashMap<String, VecDeque<crate::view::LogLine>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        logs.lock()
            .unwrap()
            .insert("inst".to_string(), VecDeque::new());

        let data = "plain line\n\x1b[31mred error\x1b[0m\n";
        spawn_log_reader(
            std::io::Cursor::new(data.as_bytes().to_vec()),
            "inst".to_string(),
            Arc::clone(&logs),
            slint::Weak::default(),
        );

        for _ in 0..200 {
            if logs.lock().unwrap().get("inst").map(|d| d.len()) == Some(2) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let logs_lock = logs.lock().unwrap();
        let deque = logs_lock.get("inst").unwrap();
        assert_eq!(deque.len(), 2);
        assert_eq!(deque[0].text.as_str(), "plain line");
        assert_eq!(deque[1].text.as_str(), "red error");
        assert_eq!(deque[1].color.red(), 231);
    }

    #[tokio::test]
    async fn test_spawn_log_reader_caps_at_500_lines() {
        let logs: Arc<Mutex<HashMap<String, VecDeque<crate::view::LogLine>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        {
            let mut deque = VecDeque::new();
            for i in 0..500 {
                deque.push_back(log_line(&format!("old-{i}")));
            }
            logs.lock().unwrap().insert("inst".to_string(), deque);
        }

        spawn_log_reader(
            std::io::Cursor::new(b"new line\n".to_vec()),
            "inst".to_string(),
            Arc::clone(&logs),
            slint::Weak::default(),
        );

        for _ in 0..200 {
            let done = logs
                .lock()
                .unwrap()
                .get("inst")
                .is_some_and(|d| d.back().is_some_and(|l| l.text.as_str() == "new line"));
            if done {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let logs_lock = logs.lock().unwrap();
        let deque = logs_lock.get("inst").unwrap();
        assert_eq!(deque.len(), 500);
        assert_eq!(deque.front().unwrap().text.as_str(), "old-1");
        assert_eq!(deque.back().unwrap().text.as_str(), "new line");
    }
}
