use super::*;

use slint::{ModelRc, VecModel};
use std::rc::Rc;
pub fn setup_create_logic(
    ui: &MainApp,
    store: std::sync::Arc<std::sync::Mutex<crate::mc_instance::InstanceStore>>,
    master_configs: std::sync::Arc<std::sync::Mutex<Vec<crate::mc_instance::InstanceConfig>>>,
) {
    let ui_weak_for_new = ui.as_weak();
    ui.global::<InstanceLogic>().on_new_instance(move || {
        let Some(ui) = ui_weak_for_new.upgrade() else {
            return;
        };
        let create = ui.global::<InstanceCreateLogic>();
        create.set_name("".into());
        create.set_mod_loader("None".into());
        create.set_mod_loader_versions(ModelRc::from(Rc::new(VecModel::from(Vec::<
            slint::SharedString,
        >::new()))));
        create.set_selected_mod_loader_version("".into());
        create.set_xmx("2G".into());
        create.set_xms("512M".into());
        create.set_logs_enabled(true);
        create.set_world_path("".into());
        create.set_resource_pack("".into());
        create.set_shader_pack("".into());
        create.set_error_msg("".into());
        create.set_active_tab(0);
        // Reset version filter flags and selection so dialog always opens at latest
        create.set_show_release(true);
        create.set_show_snapshot(false);
        create.set_show_beta(false);
        create.set_show_alpha(false);
        create.set_show_experimental(false);
        create.set_selected_version("".into());
        create.invoke_filter_versions();
        create.set_show_dialog(true);
    });

    let create_logic = ui.global::<InstanceCreateLogic>();

    let ui_weak_for_cancel = ui.as_weak();
    create_logic.on_cancel_create(move || {
        if let Some(ui) = ui_weak_for_cancel.upgrade() {
            ui.global::<InstanceCreateLogic>().set_show_dialog(false);
        }
    });

    let ui_weak_for_loader = ui.as_weak();
    create_logic.on_loader_changed(move || {
        let ui_handle_async = ui_weak_for_loader.clone();

        // Debounce to allow Mac AccessKit to finish combobox interaction before UI heavily updates
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = ui_handle_async.upgrade() else {
                    return;
                };
                let logic = ui.global::<InstanceCreateLogic>();

                let mc_version = logic.get_selected_version().to_string();
                let mod_loader_str = logic.get_mod_loader().to_string();

                if mod_loader_str == "None" || mc_version.is_empty() {
                    logic.set_mod_loader_versions(ModelRc::from(Rc::new(VecModel::from(vec![]))));
                    logic.set_selected_mod_loader_version("".into());
                    return;
                }

                let loader_type = match mod_loader_str.as_str() {
                    "Fabric" => crate::mc_modloader::ModLoaderType::Fabric,
                    "Forge" => crate::mc_modloader::ModLoaderType::Forge,
                    "NeoForge" => crate::mc_modloader::ModLoaderType::NeoForge,
                    _ => return,
                };

                logic.set_is_loading(true);
                let ui_handle_inner = ui.as_weak();

                tokio::spawn(async move {
                    match crate::mc_modloader::ModLoaderApi::get_loader_versions(
                        loader_type,
                        &mc_version,
                    )
                    .await
                    {
                        Ok(versions) => {
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_handle_inner.upgrade() {
                                    let logic = ui.global::<InstanceCreateLogic>();
                                    let slint_versions: Vec<slint::SharedString> = versions
                                        .into_iter()
                                        .map(slint::SharedString::from)
                                        .collect();
                                    let model = ModelRc::from(Rc::new(VecModel::from(
                                        slint_versions.clone(),
                                    )));
                                    logic.set_mod_loader_versions(model);

                                    if let Some(first) = slint_versions.first() {
                                        logic.set_selected_mod_loader_version(first.clone());
                                        logic.set_selected_loader_index(0);
                                    } else {
                                        logic.set_selected_mod_loader_version(
                                            "No available versions".into(),
                                        );
                                    }
                                    logic.set_is_loading(false);
                                }
                            });
                        }
                        Err(e) => {
                            println!("Failed to fetch Mod Loader versions: {}", e);
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = ui_handle_inner.upgrade() {
                                    let logic = ui.global::<InstanceCreateLogic>();
                                    logic.set_mod_loader_versions(ModelRc::from(Rc::new(
                                        VecModel::from(vec![]),
                                    )));
                                    logic.set_selected_mod_loader_version("Read failed".into());
                                    logic.set_is_loading(false);
                                }
                            });
                        }
                    }
                });
            });
        });
    });

    let store_for_create = Arc::clone(&store);
    let master_for_create = Arc::clone(&master_configs);
    let ui_weak_for_confirm = ui.as_weak();
    create_logic.on_confirm_create(move || {
        let Some(ui) = ui_weak_for_confirm.upgrade() else {
            return;
        };
        let create = ui.global::<InstanceCreateLogic>();

        let name = create.get_name().to_string();
        let version = create.get_selected_version().to_string();

        if name.trim().is_empty() {
            create.set_error_msg("Instance name cannot be empty".into());
            return;
        }
        if version.is_empty() {
            create.set_error_msg("Please select Minecraft version".into());
            return;
        }

        let mod_loader = create.get_mod_loader().to_string();
        if mod_loader != "None" && !mod_loader.is_empty() {
            let loader_ver = create.get_selected_mod_loader_version().to_string();
            match loader_ver.as_str() {
                "" => {
                    create.set_error_msg(
                        format!("Please select a {} version", mod_loader).into(),
                    );
                    return;
                }
                "No available versions" => {
                    create.set_error_msg(
                        format!("{} has no available versions for Minecraft {}", mod_loader, version).into(),
                    );
                    return;
                }
                "Read failed" => {
                    create.set_error_msg(
                        "Failed to load mod loader versions. Please check your network and try again.".into(),
                    );
                    return;
                }
                _ => {}
            }
        }

        let config = InstanceConfig {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.trim().to_string(),
            version,
            mod_loader: create.get_mod_loader().to_string(),
            mod_loader_version: create.get_selected_mod_loader_version().to_string(),
            xmx: create.get_xmx().to_string(),
            xms: create.get_xms().to_string(),
            logs_enabled: create.get_logs_enabled(),
            world_path: create.get_world_path().to_string(),
            resource_pack: create.get_resource_pack().to_string(),
            shader_pack: create.get_shader_pack().to_string(),
            ..Default::default()
        };

        match store_for_create.lock().unwrap().append(config) {
            Ok(updated) => {
                *master_for_create.lock().unwrap() = updated.clone();
                let logic = ui.global::<InstanceLogic>();
                let new_items: Vec<InstanceData> = updated.iter().map(config_to_ui_data).collect();
                logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(new_items))));
                create.set_show_dialog(false);
            }
            Err(e) => {
                create.set_error_msg(format!("Creation failed: {e}").into());
            }
        }
    });
}
