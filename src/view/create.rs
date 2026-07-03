use super::*;

use slint::{ModelRc, VecModel};
use std::rc::Rc;

/// 驗證建立實例的輸入，回傳第一個錯誤訊息；全部通過則回傳 Ok
pub(crate) fn validate_create_input(
    name: &str,
    version: &str,
    mod_loader: &str,
    loader_version: &str,
) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Instance name cannot be empty".to_string());
    }
    if version.is_empty() {
        return Err("Please select Minecraft version".to_string());
    }
    // 選了 Loader 但沒有可用版本（載入失敗 / 無版本時 selected 會保持空字串）
    if mod_loader != "None" && !mod_loader.is_empty() && loader_version.is_empty() {
        return Err(format!("Please select a {} version", mod_loader));
    }
    Ok(())
}

/// 空的字串模型（清空下拉選單用）
fn empty_string_model() -> ModelRc<slint::SharedString> {
    ModelRc::from(Rc::new(VecModel::from(Vec::<slint::SharedString>::new())))
}

pub fn setup_create_logic(
    ui: &MainApp,
    store: std::sync::Arc<std::sync::Mutex<crate::mc_instance::InstanceStore>>,
    master_configs: std::sync::Arc<std::sync::Mutex<Vec<crate::mc_instance::InstanceConfig>>>,
    running_procs: std::sync::Arc<
        std::sync::Mutex<std::collections::HashMap<String, std::process::Child>>,
    >,
) {
    let ui_weak_for_new = ui.as_weak();
    ui.global::<InstanceLogic>().on_new_instance(move || {
        let Some(ui) = ui_weak_for_new.upgrade() else {
            return;
        };
        let create = ui.global::<InstanceCreateLogic>();
        create.set_name("".into());
        create.set_mod_loader("None".into());
        create.set_mod_loader_versions(empty_string_model());
        create.set_selected_mod_loader_version("".into());
        create.set_selected_loader_index(-1);
        create.set_is_loader_loading(false);
        create.set_loader_load_error("".into());
        create.set_fabric_available(true);
        create.set_forge_available(true);
        create.set_neoforge_available(true);
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
        create.set_selected_version_index(-1);
        create.set_version_search_text("".into());
        // filter-versions 會選出預設版本，其結尾會 invoke loader-changed
        // 查詢各 Loader 對該版本的可用性，此處不需重複觸發
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
    // 世代計數器：快速連續切換 Loader / MC 版本時，讓過期的抓取結果自動作廢，
    // 避免較慢的舊回應覆蓋較新的選擇（競態防護）
    let loader_fetch_gen = Arc::new(std::sync::atomic::AtomicU64::new(0));
    create_logic.on_loader_changed(move || {
        use std::sync::atomic::Ordering;

        let my_gen = loader_fetch_gen.fetch_add(1, Ordering::SeqCst) + 1;
        let gen_handle = Arc::clone(&loader_fetch_gen);
        let ui_handle_async = ui_weak_for_loader.clone();

        // Debounce：讓 Mac AccessKit 完成互動後再大量更新 UI；
        // 期間若使用者再次切換，世代已前進，本次直接放棄
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if gen_handle.load(Ordering::SeqCst) != my_gen {
                return;
            }

            let gen_for_ui = Arc::clone(&gen_handle);
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = ui_handle_async.upgrade() else {
                    return;
                };
                let logic = ui.global::<InstanceCreateLogic>();

                let mc_version = logic.get_selected_version().to_string();
                let mod_loader_str = logic.get_mod_loader().to_string();

                // 切換 Loader / MC 版本時一律先清空舊清單與錯誤提示
                logic.set_mod_loader_versions(empty_string_model());
                logic.set_selected_mod_loader_version("".into());
                logic.set_selected_loader_index(-1);
                logic.set_loader_load_error("".into());

                if mc_version.is_empty() {
                    logic.set_is_loader_loading(false);
                    return;
                }

                // None 時 loader_type 為空，仍要查可用性以更新單選鈕的禁用狀態
                let loader_type = crate::mc_modloader::ModLoaderType::from_name(&mod_loader_str);
                logic.set_is_loader_loading(loader_type.is_some());

                let ui_handle_inner = ui.as_weak();
                let gen_for_fetch = Arc::clone(&gen_for_ui);

                tokio::spawn(async move {
                    // 查詢三種 Loader 對此 MC 版本的支援度
                    //（Forge / NeoForge 使用快取的 maven metadata，通常無需網路）
                    let avail =
                        crate::mc_modloader::ModLoaderApi::check_availability(&mc_version).await;
                    let selected_available =
                        loader_type.is_some_and(|lt| avail.supports(lt));

                    // 只有選中的 Loader 可用時才抓它的版本清單
                    let versions_result = match loader_type {
                        Some(lt) if selected_available => Some(
                            crate::mc_modloader::ModLoaderApi::get_loader_versions(
                                lt,
                                &mc_version,
                            )
                            .await,
                        ),
                        _ => None,
                    };

                    let _ = slint::invoke_from_event_loop(move || {
                        // 回應抵達時已有更新的請求 → 丟棄本次結果
                        if gen_for_fetch.load(Ordering::SeqCst) != my_gen {
                            return;
                        }
                        let Some(ui) = ui_handle_inner.upgrade() else {
                            return;
                        };
                        let logic = ui.global::<InstanceCreateLogic>();
                        logic.set_is_loader_loading(false);
                        logic.set_fabric_available(avail.fabric);
                        logic.set_forge_available(avail.forge);
                        logic.set_neoforge_available(avail.neoforge);

                        // 已選的 Loader 不支援新選的 MC 版本 → 自動退回 None
                        if loader_type.is_some() && !selected_available {
                            logic.set_mod_loader("None".into());
                            return;
                        }

                        match versions_result {
                            Some(Ok(versions)) if !versions.is_empty() => {
                                let default_idx =
                                    crate::mc_modloader::ModLoaderApi::default_version_index(
                                        &versions,
                                    );
                                let slint_versions: Vec<slint::SharedString> = versions
                                    .into_iter()
                                    .map(slint::SharedString::from)
                                    .collect();
                                let default_ver = slint_versions[default_idx].clone();
                                logic.set_mod_loader_versions(ModelRc::from(Rc::new(
                                    VecModel::from(slint_versions),
                                )));
                                logic.set_selected_mod_loader_version(default_ver);
                                logic.set_selected_loader_index(default_idx as i32);
                            }
                            Some(Ok(_)) => {
                                logic.set_loader_load_error(
                                    format!(
                                        "{} has no available versions for Minecraft {}",
                                        mod_loader_str, mc_version
                                    )
                                    .into(),
                                );
                            }
                            Some(Err(e)) => {
                                tracing::error!(error = %e, loader = %mod_loader_str, "Failed to fetch mod loader versions");
                                logic.set_loader_load_error(
                                    "Failed to load versions. Please check your network and try again."
                                        .into(),
                                );
                            }
                            // 未選 Loader：僅更新可用性
                            None => {}
                        }
                    });
                });
            });
        });
    });

    let store_for_create = Arc::clone(&store);
    let master_for_create = Arc::clone(&master_configs);
    let running_for_create = Arc::clone(&running_procs);
    let ui_weak_for_confirm = ui.as_weak();
    create_logic.on_confirm_create(move || {
        let Some(ui) = ui_weak_for_confirm.upgrade() else {
            return;
        };
        let create = ui.global::<InstanceCreateLogic>();

        let name = create.get_name().to_string();
        let version = create.get_selected_version().to_string();
        let mod_loader = create.get_mod_loader().to_string();
        let loader_ver = create.get_selected_mod_loader_version().to_string();

        if let Err(msg) = validate_create_input(&name, &version, &mod_loader, &loader_ver) {
            create.set_error_msg(msg.into());
            return;
        }

        let config = InstanceConfig {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.trim().to_string(),
            version,
            mod_loader,
            mod_loader_version: loader_ver,
            xmx: create.get_xmx().to_string(),
            xms: create.get_xms().to_string(),
            logs_enabled: create.get_logs_enabled(),
            world_path: create.get_world_path().to_string(),
            resource_pack: create.get_resource_pack().to_string(),
            shader_pack: create.get_shader_pack().to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
            ..Default::default()
        };

        match store_for_create.lock().unwrap().append(config) {
            Ok(updated) => {
                *master_for_create.lock().unwrap() = updated.clone();
                let logic = ui.global::<InstanceLogic>();
                // 清空搜尋並顯示完整清單：新實例若不符目前搜尋字串會被過濾掉，
                // 使用者會誤以為建立失敗
                logic.set_search_text("".into());

                let running_ids: std::collections::HashSet<String> =
                    running_for_create.lock().unwrap().keys().cloned().collect();

                let new_items: Vec<InstanceData> = updated
                    .iter()
                    .map(|c| {
                        let mut item = config_to_ui_data(c);
                        if running_ids.contains(&c.id) {
                            item.status = "running".into();
                        }
                        item
                    })
                    .collect();
                logic.set_instance_list(ModelRc::from(Rc::new(VecModel::from(new_items))));
                create.set_show_dialog(false);
            }
            Err(e) => {
                create.set_error_msg(format!("Creation failed: {e}").into());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::validate_create_input;

    #[test]
    fn test_empty_name_rejected() {
        let err = validate_create_input("   ", "1.20.4", "None", "").unwrap_err();
        assert!(err.contains("name"));
    }

    #[test]
    fn test_missing_version_rejected() {
        let err = validate_create_input("My Instance", "", "None", "").unwrap_err();
        assert!(err.contains("Minecraft version"));
    }

    #[test]
    fn test_vanilla_without_loader_version_ok() {
        // mod loader 為 None 或空字串時不檢查 loader 版本
        assert!(validate_create_input("My Instance", "1.20.4", "None", "").is_ok());
        assert!(validate_create_input("My Instance", "1.20.4", "", "").is_ok());
    }

    #[test]
    fn test_loader_selected_but_no_version_rejected() {
        let err = validate_create_input("My Instance", "1.20.4", "Forge", "").unwrap_err();
        assert!(err.contains("Forge"));
    }

    #[test]
    fn test_valid_loader_version_ok() {
        assert!(
            validate_create_input("A", "1.20.4", "Forge", "1.20.4-49.0.50 (Recommended)").is_ok()
        );
    }
}
