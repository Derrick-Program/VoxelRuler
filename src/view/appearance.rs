use super::*;

pub fn setup_appearance_window(ui: &MainApp) {
    let appearance_win_rc: std::rc::Rc<std::cell::RefCell<Option<AppearanceWindow>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    let ap_rc_manage = appearance_win_rc.clone();
    let skin_timer_rc: std::rc::Rc<std::cell::RefCell<Option<slint::Timer>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    let skin_timer_manage = skin_timer_rc.clone();
    let active_renderer: std::sync::Arc<
        std::sync::Mutex<Option<crate::skin_renderer::SkinRenderer>>,
    > = std::sync::Arc::new(std::sync::Mutex::new(None));
    let active_renderer_manage = active_renderer.clone();

    let main_ui_weak_for_appearance = ui.as_weak();
    ui.global::<PageAccountLogic>()
            .on_manage_appearance(move || {
                let mut ap_ref = ap_rc_manage.borrow_mut();
                if ap_ref.is_none()
                    && let Ok(ap) = AppearanceWindow::new() {
                        let ap_weak = ap.as_weak();
                        ap.window().on_close_requested(move || {
                            if let Some(ap) = ap_weak.upgrade() {
                                let _ = ap.hide();
                            }
                            slint::CloseRequestResponse::KeepWindowShown
                        });

                        let ap_weak_drag = ap.as_weak();
                        ap.global::<AppearanceLogic>().on_preview_drag_started(move || {
                            let Some(ap) = ap_weak_drag.upgrade() else { return };
                            let apl = ap.global::<AppearanceLogic>();
                            apl.set_base_yaw(apl.get_preview_yaw());
                            apl.set_base_pitch(apl.get_preview_pitch());

                            #[cfg(target_os = "macos")]
                            if crate::GLOBAL_CACHE.get("mac_natural_scroll").is_none() {
                                let val = std::process::Command::new("defaults")
                                    .args(["read", "-g", "com.apple.swipescrolldirection"])
                                    .output()
                                    .ok()
                                    .and_then(|o| String::from_utf8(o.stdout).ok())
                                    .map(|s| if s.trim() == "0" { "0" } else { "1" })
                                    .unwrap_or("1");
                                crate::GLOBAL_CACHE.insert("mac_natural_scroll".to_string(), val.to_string());
                            }
                        });

                        let ap_weak_drag2 = ap.as_weak();
                        ap.global::<AppearanceLogic>().on_preview_dragged(move |dx, dy| {
                            if let Some(ap) = ap_weak_drag2.upgrade() {
                                let apl = ap.global::<AppearanceLogic>();
                                let mut multiplier = 1.0;

                                #[cfg(target_os = "macos")]
                                {
                                    if let Some(val) = crate::GLOBAL_CACHE.get("mac_natural_scroll") {
                                        if val.value() == "1" {
                                            multiplier = -1.0;
                                        }
                                    } else {
                                        multiplier = -1.0;
                                    }
                                }

                                let new_yaw = apl.get_base_yaw() - dx * multiplier;
                                let new_pitch = (apl.get_base_pitch() - dy * multiplier).clamp(-90.0, 90.0);
                                apl.set_preview_yaw(new_yaw);
                                apl.set_preview_pitch(new_pitch);
                            }
                        });

                        let ap_timer = ap.as_weak();
                        let active_renderer_timer = active_renderer_manage.clone();
                        let timer = slint::Timer::default();
                        timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(33), move || {
                            let Some(ap) = ap_timer.upgrade() else { return };
                            let apl = ap.global::<AppearanceLogic>();
                            let time = apl.get_preview_time() + 0.033;
                            apl.set_preview_time(time);

                            if let Ok(guard) = active_renderer_timer.try_lock()
                                && let Some(renderer) = &*guard {
                                    let yaw = apl.get_preview_yaw();
                                    let pitch = apl.get_preview_pitch();
                                    let slim = apl.get_skin_variant() == "slim";
                                    let buffer = renderer.render(240, 360, yaw.to_radians(), pitch.to_radians(), slim, time);
                                    apl.set_preview_image(slint::Image::from_rgba8(buffer));
                                    apl.set_has_preview(true);
                                }
                        });
                        *skin_timer_manage.borrow_mut() = Some(timer);

                        let ap_weak_close = ap.as_weak();
                        ap.global::<AppearanceLogic>().on_close_appearance(move || {
                            if let Some(ap) = ap_weak_close.upgrade() {
                                let _ = ap.hide();
                            }
                        });
                        let ap_weak_apply = ap.as_weak();
                        let main_ui_weak_apply = main_ui_weak_for_appearance.clone();
                        ap.global::<AppearanceLogic>().on_apply_appearance(move || {
                            let ap_weak = ap_weak_apply.clone();
                            let token = crate::GLOBAL_CACHE.get("mc_ac_key").map(|r| r.value().clone()).unwrap_or_default();
                            if token.is_empty() { return; }

                            let mut skin_url_to_apply = String::new();
                            let mut variant_to_apply = String::new();
                            let mut cape_id_to_apply = String::new();

                            let mut skin_changed = false;
                            let mut cape_changed = false;

                            if let Some(ap) = ap_weak.upgrade() {
                                let apl = ap.global::<AppearanceLogic>();
                                let selected_skin_url = apl.get_selected_library_skin_url().to_string();
                                let active_skin_url = apl.get_active_skin_url().to_string();
                                let selected_cape_id = apl.get_selected_cape_id().to_string();
                                let active_cape_id = apl.get_active_cape_id().to_string();

                                skin_url_to_apply = selected_skin_url.clone();
                                variant_to_apply = apl.get_skin_variant().to_string();
                                cape_id_to_apply = selected_cape_id.clone();

                                skin_changed = !selected_skin_url.is_empty() && selected_skin_url != active_skin_url;
                                cape_changed = selected_cape_id != active_cape_id;

                                if !skin_changed && !cape_changed { return; }

                                apl.set_is_uploading(true);
                                apl.set_upload_status("Applying changes...".into());
                            }

                            let main_ui_weak = main_ui_weak_apply.clone();
                            let username = crate::mc_token::SessionData::load_session()
                                .ok()
                                .flatten()
                                .map(|s| s.mc_username().clone())
                                .unwrap_or_default();

                            tokio::spawn(async move {
                                let api = crate::mc_api::McAction::new().authenticate(&token);

                                let mut has_error = false;
                                let mut err_msg = String::new();
                                let mut skin_success = false;
                                let mut new_profile = None;

                                if skin_changed {
                                    let is_file = skin_url_to_apply.starts_with("file://");
                                    let file_path = is_file
                                        .then(|| url::Url::parse(&skin_url_to_apply).ok().and_then(|u| u.to_file_path().ok()))
                                        .flatten()
                                        .unwrap_or_default();
                                    let auto_variant = auto_detect_variant(&skin_url_to_apply).await.unwrap_or(variant_to_apply);

                                    match if is_file {
                                        api.upload_skin_from_file(&file_path, &auto_variant).await
                                    } else {
                                        api.upload_skin_from_url(&skin_url_to_apply, &auto_variant).await
                                    } {
                                        Ok(()) => skin_success = true,
                                        Err(e) => {
                                            has_error = true;
                                            err_msg.push_str(&format!("Failed to apply skin: {}\n", e));
                                        }
                                    }
                                }

                                if cape_changed {
                                    let res = if cape_id_to_apply.is_empty() {
                                        api.hide_cape().await.map(|_| None)
                                    } else {
                                        api.set_active_cape(&cape_id_to_apply).await.map(Some)
                                    };

                                    match res {
                                        Ok(Some(p)) => new_profile = Some(p),
                                        Ok(None) => {},
                                        Err(e) => {
                                            has_error = true;
                                            err_msg.push_str(&format!("Failed to apply cape: {}\n", e));
                                        }
                                    }
                                }

                                let fetch_result = if skin_success && !username.is_empty() {
                                    fetch_avatar_from_mojang(&token, &username, false).await
                                } else {
                                    None
                                };
                                let avatar_path_opt = fetch_result.as_ref().map(|(p, _)| p.clone());
                                let new_active_url = fetch_result.map(|(_, u)| u);

                                if cape_changed && !has_error {
                                    update_cape_cache(new_profile.as_ref(), &cape_id_to_apply);
                                }

                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ap) = ap_weak.upgrade() {
                                        let apl = ap.global::<AppearanceLogic>();
                                        apl.set_is_uploading(false);

                                        if has_error {
                                            apl.set_upload_is_error(true);
                                            apl.set_upload_status(err_msg.trim().into());
                                        } else {
                                            apl.set_upload_is_error(false);
                                            apl.set_upload_status("Appearance applied successfully!\n(May require relogging in-game)".into());
                                            if skin_changed {
                                                apl.set_active_skin_url(skin_url_to_apply.clone().into());
                                            }
                                            if cape_changed {
                                                apl.set_active_cape_id(cape_id_to_apply.clone().into());
                                            }
                                        }
                                        apl.set_show_result_dialog(true);
                                    }

                                    update_avatar_in_ui(&main_ui_weak, avatar_path_opt.as_deref(), &username);
                                });
                            });
                        });

                        let ap_weak_select_cape = ap.as_weak();
                        let active_renderer_select = active_renderer_manage.clone();
                        ap.global::<AppearanceLogic>().on_select_cape(move |cape_id| {
                            handle_select_cape(cape_id, ap_weak_select_cape.clone(), active_renderer_select.clone());
                        });


                        let ap_weak_delete = ap.as_weak();
                        ap.global::<AppearanceLogic>().on_delete_skin(move |id| {
                            if let Ok(paths) = crate::mc_paths::McPaths::new() {
                                let history_file = paths.skins_history_file();
                                let mut history = crate::skin_history::SkinHistory::load(&history_file);
                                history.skins.retain(|s| s.name != id.as_str());
                                let _ = history.save(&history_file);

                                if let Some(ap) = ap_weak_delete.upgrade() {
                                    let ui_skins = get_ui_skins(&paths, &history);
                                    ap.global::<AppearanceLogic>().set_skin_history(
                                        slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(ui_skins)))
                                    );
                                }
                            }
                        });

                        let ap_weak_browse = ap.as_weak();
                        let active_renderer_browse = active_renderer_manage.clone();
                        ap.global::<AppearanceLogic>().on_browse_skin_file(move || {
                            handle_browse_skin_file(ap_weak_browse.clone(), active_renderer_browse.clone());
                        });

                        let ap_weak_save = ap.as_weak();
                        ap.global::<AppearanceLogic>().on_save_to_library(move || {
                            let Some(ap) = ap_weak_save.upgrade() else { return };
                            let apl = ap.global::<AppearanceLogic>();
                            let name = apl.get_new_skin_name().to_string();
                            let path_str = apl.get_selected_skin_path().to_string();
                            let url_str = apl.get_skin_url().to_string();
                            let variant = apl.get_skin_variant().to_string();

                            let name = if name.is_empty() { "Unnamed Appearance".to_string() } else { name };

                            apl.set_upload_status("Adding to skin library...".into());
                            apl.set_upload_is_error(false);
                            apl.set_is_uploading(true);

                            let ap_weak_async = ap.as_weak();
                            tokio::spawn(async move {
                                let mut skin_bytes = Vec::new();
                                let mut is_error = false;
                                let mut status = "Added successfully!".to_string();

                                match fetch_skin_bytes(&path_str, &url_str).await {
                                    Ok(bytes) => skin_bytes = bytes,
                                    Err(e) => {
                                        is_error = true;
                                        status = e;
                                    }
                                }

                                if !is_error && !skin_bytes.is_empty()
                                    && let Ok(paths) = crate::mc_paths::McPaths::new() {
                                        let history_file = paths.skins_history_file();
                                        let mut history = crate::skin_history::SkinHistory::load(&history_file);

                                        use sha1::Digest;
                                        let mut hash = String::new();
                                        if let Ok(img) = image::load_from_memory(&skin_bytes) {
                                            let mut hasher = sha1::Sha1::new();
                                            hasher.update(img.to_rgba8().into_raw());
                                            hash = hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();
                                        } else {
                                            let mut hasher = sha1::Sha1::new();
                                            hasher.update(&skin_bytes);
                                            hash = hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();
                                        }

                                        let target_path = paths.skins_dir().join(format!("{}.png", hash));
                                        let render_path = paths.skins_dir().join(format!("{}_render.png", hash));

                                        let _ = std::fs::write(&target_path, &skin_bytes);
                                        if let Ok(img) = image::load_from_memory(&skin_bytes) {
                                            let is_slim = variant == "slim";
                                            let render_img = crate::view::generate_2d_front(&img, is_slim);
                                            let _ = render_img.save(&render_path);
                                        }

                                        let final_url = format!("file://{}", target_path.display());

                                        history.skins.retain(|s| {
                                            let s_hash = s.url.split('/').next_back().unwrap_or("").trim_end_matches(".png");
                                            s_hash != hash
                                        });

                                        history.add_skin(crate::skin_history::SkinEntry {
                                            cape_id: "".to_string(),
                                            model: variant.clone(),
                                            name: name.clone(),
                                            url: if url_str.is_empty() { final_url } else { url_str.clone() },
                                        });
                                        let _ = history.save(&history_file);
                                    }

                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ap) = ap_weak_async.upgrade() {
                                        let apl = ap.global::<AppearanceLogic>();
                                        apl.set_upload_status(status.into());
                                        apl.set_upload_is_error(is_error);
                                        apl.set_is_uploading(false);

                                        if !is_error {
                                            reload_skin_history_ui(&apl);
                                        } else {
                                            apl.set_show_result_dialog(true);
                                        }
                                    }
                                });
                            });
                        });

                        let active_renderer_preview = active_renderer_manage.clone();
                        let ap_weak_preview = ap.as_weak();
                        ap.global::<AppearanceLogic>().on_preview_library_skin(move |url| {
                            let url_str = url.to_string();
                            let active_renderer = active_renderer_preview.clone();
                            let ap_weak = ap_weak_preview.clone();
                            tokio::spawn(async move {
                                let skin_bytes: Option<Vec<u8>> = if url_str.starts_with("file://") {
                                    url::Url::parse(&url_str).ok()
                                        .and_then(|u| u.to_file_path().ok())
                                        .and_then(|p| std::fs::read(&p).ok())
                                } else if !url_str.is_empty() {
                                    match reqwest::get(&url_str).await {
                                        Ok(resp) => resp.bytes().await.ok().map(|b| b.to_vec()),
                                        Err(_) => None,
                                    }
                                } else {
                                    None
                                };

                                let Some(bytes) = skin_bytes else { return };
                                let Ok(img) = image::load_from_memory(&bytes) else { return };
                                if let Ok(mut guard) = active_renderer.lock() {
                                    let old_cape = guard.as_ref().and_then(|r| r.get_cape());
                                    let mut new_renderer = crate::skin_renderer::SkinRenderer::new(img);
                                    new_renderer.set_cape_rgba(old_cape);
                                    *guard = Some(new_renderer);
                                }
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ap) = ap_weak.upgrade() {
                                        ap.global::<AppearanceLogic>().set_has_preview(true);
                                    }
                                });
                            });
                        });

                        let main_ui_weak_inner = main_ui_weak_for_appearance.clone();
                        let ap_weak_upload = ap.as_weak();
                        let main_ui_weak_upload = main_ui_weak_inner.clone();
                        ap.global::<AppearanceLogic>().on_upload_from_file(move || {
                            let Some(ap) = ap_weak_upload.upgrade() else { return };
                            let apl = ap.global::<AppearanceLogic>();
                            let path_str = apl.get_selected_skin_path().to_string();
                            let variant = apl.get_skin_variant().to_string();
                            let token = crate::GLOBAL_CACHE.get("mc_ac_key").map(|v| v.clone()).unwrap_or_default();

                            if path_str.is_empty() {
                                apl.set_upload_status("Please select skin file first".into());
                                apl.set_upload_is_error(true);
                                return;
                            }
                            if token.is_empty() {
                                apl.set_upload_status("Please login to Microsoft account first".into());
                                apl.set_upload_is_error(true);
                                return;
                            }

                            apl.set_upload_status("Uploading...".into());
                            apl.set_upload_is_error(false);
                            apl.set_is_uploading(true);

                            let ap_weak_async = ap.as_weak();
                            let main_ui_weak = main_ui_weak_upload.clone();
                            let username = crate::mc_token::SessionData::load_session()
                                .ok()
                                .flatten()
                                .map(|s| s.mc_username().clone())
                                .unwrap_or_default();

                            tokio::spawn(async move {
                                let api = crate::mc_api::McAction::new().authenticate(&token);
                                let result = api.upload_skin_from_file(std::path::Path::new(&path_str), &variant).await;
                                let (status, is_error) = match result {
                                    Ok(()) => ("Upload successful!".to_string(), false),
                                    Err(e) => (format!("Upload failed: {e}"), true),
                                };

                                let fetch_result: Option<(std::path::PathBuf, String)> = if !is_error && !username.is_empty() {
                                    fetch_avatar_from_mojang(&token, &username, false).await
                                } else {
                                    None
                                };
                                let avatar_path_opt = fetch_result.as_ref().map(|(p, _)| p.clone());
                                let new_active_url = fetch_result.map(|(_, u)| u);

                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ap) = ap_weak_async.upgrade() {
                                        let apl = ap.global::<AppearanceLogic>();
                                        apl.set_upload_status(status.into());
                                        apl.set_upload_is_error(is_error);
                                        apl.set_is_uploading(false);

                                        if let Some(url) = new_active_url {
                                            apl.set_active_skin_url(url.into());
                                        }
                                        if !is_error {
                                            reload_skin_history_ui(&apl);
                                        }
                                    }
                                    update_avatar_in_ui(&main_ui_weak, avatar_path_opt.as_deref(), &username);
                                });
                            });
                        });

                        let ap_weak_url = ap.as_weak();
                        let main_ui_weak_url = main_ui_weak_inner.clone();
                        ap.global::<AppearanceLogic>().on_upload_from_url(move |url| {
                            let Some(ap) = ap_weak_url.upgrade() else { return };
                            let apl = ap.global::<AppearanceLogic>();
                            let url_str = url.to_string();
                            let variant = apl.get_skin_variant().to_string();
                            let token = crate::GLOBAL_CACHE.get("mc_ac_key").map(|v| v.clone()).unwrap_or_default();

                            if url_str.is_empty() {
                                apl.set_upload_status("Please enter skin URL".into());
                                apl.set_upload_is_error(true);
                                return;
                            }
                            if token.is_empty() {
                                apl.set_upload_status("Please login to Microsoft account first".into());
                                apl.set_upload_is_error(true);
                                return;
                            }

                            apl.set_upload_status("Applying...".into());
                            apl.set_upload_is_error(false);
                            apl.set_is_uploading(true);

                            let ap_weak_async = ap.as_weak();
                            let main_ui_weak = main_ui_weak_url.clone();
                            let username = crate::mc_token::SessionData::load_session()
                                .ok()
                                .flatten()
                                .map(|s| s.mc_username().clone())
                                .unwrap_or_default();

                            tokio::spawn(async move {
                                let is_file = url_str.starts_with("file://");
                                let file_path = is_file
                                    .then(|| url::Url::parse(&url_str).ok().and_then(|u| u.to_file_path().ok()))
                                    .flatten()
                                    .unwrap_or_default();
                                let auto_variant = auto_detect_variant(&url_str).await.unwrap_or(variant);

                                let api = crate::mc_api::McAction::new().authenticate(&token);
                                let result = if is_file {
                                    api.upload_skin_from_file(&file_path, &auto_variant).await
                                } else {
                                    api.upload_skin_from_url(&url_str, &auto_variant).await
                                };
                                let (status, is_error) = match result {
                                    Ok(()) => ("Applied successfully!".to_string(), false),
                                    Err(e) => (format!("Apply failed: {e}"), true),
                                };

                                let fetch_result: Option<(std::path::PathBuf, String)> = if !is_error && !username.is_empty() {
                                    fetch_avatar_from_mojang(&token, &username, false).await
                                } else {
                                    None
                                };
                                let avatar_path_opt = fetch_result.as_ref().map(|(p, _)| p.clone());
                                let new_active_url = fetch_result.map(|(_, u)| u);

                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ap) = ap_weak_async.upgrade() {
                                        let apl = ap.global::<AppearanceLogic>();
                                        apl.set_skin_variant(auto_variant.into());
                                        apl.set_upload_status(status.into());
                                        apl.set_upload_is_error(is_error);
                                        apl.set_is_uploading(false);
                                        apl.set_show_result_dialog(true);
                                        if let Some(url) = new_active_url {
                                            apl.set_active_skin_url(url.into());
                                        }
                                        if !is_error {
                                            reload_skin_history_ui(&apl);
                                        }
                                    }
                                    update_avatar_in_ui(&main_ui_weak, avatar_path_opt.as_deref(), &username);
                                });
                            });
                        });

                        *ap_ref = Some(ap);
                    }

                if let Some(ap) = ap_ref.as_ref() {
                    let apl = ap.global::<AppearanceLogic>();
                    apl.set_upload_status("".into());
                    apl.set_upload_is_error(false);
                    apl.set_skin_url("".into());
                    apl.set_selected_skin_path("".into());
                    apl.set_skin_variant("classic".into());
                    apl.set_is_uploading(false);
                    apl.set_has_preview(false);

                    reload_skin_history_ui(&apl);

                    let token = crate::GLOBAL_CACHE.get("mc_ac_key").map(|v| v.clone()).unwrap_or_default();
                    if !token.is_empty() {
                        let active_renderer_spawn = active_renderer_manage.clone();
                        let ap_weak = ap.as_weak();
                        tokio::spawn(async move {
                            let api = crate::mc_api::McAction::new().authenticate(&token);

                            let cache_file = crate::mc_paths::McPaths::new().ok()
                                .map(|p| p.capes_dir().join("profile_cache.json"));
                            let cached_profile = cache_file.as_ref()
                                .and_then(|f| std::fs::read_to_string(f).ok())
                                .and_then(|s| serde_json::from_str::<crate::mc_types::McProfile>(&s).ok());

                            let profile_opt = match api.get_user_profile().await.ok() {
                                Some(p) => {
                                    if let Some(f) = &cache_file {
                                        let _ = std::fs::write(f, serde_json::to_string_pretty(&p).unwrap_or_default());
                                    }
                                    Some(p)
                                }
                                None => cached_profile,
                            };

                            if let Some(profile) = profile_opt {
                                let active_skin_url = profile.skins.iter()
                                    .find(|s| s.state == crate::mc_types::McState::Active)
                                    .map(|s| s.url.clone())
                                    .unwrap_or_default();

                                struct TempCape {
                                    id: String,
                                    alias: String,
                                    state: crate::mc_types::McState,
                                    url: String,
                                    buffer: Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>>,
                                }

                                use futures_util::stream::{self, StreamExt};
                                let capes_temp: Vec<TempCape> = stream::iter(&profile.capes)
                                    .then(|cape| async move {
                                        let buffer = fetch_cape_bytes_cached(&cape.id, &cape.url).await
                                            .and_then(|b| image::load_from_memory(&b).ok())
                                            .map(|img| {
                                                let (raw, w, h) = create_cape_preview_raw(&img);
                                                slint::SharedPixelBuffer::clone_from_slice(&raw, w, h)
                                            });
                                        TempCape {
                                            id: cape.id.clone(),
                                            alias: cape.alias.clone(),
                                            state: cape.state,
                                            url: cape.url.clone(),
                                            buffer,
                                        }
                                    })
                                    .collect()
                                    .await;

                                let has_capes = !capes_temp.is_empty();
                                let active_cape_id = profile.capes.iter()
                                    .find(|c| c.state == crate::mc_types::McState::Active)
                                    .map(|c| c.id.clone())
                                    .unwrap_or_default();

                                let active_url_clone = active_skin_url.clone();
                                let ap_weak_1 = ap_weak.clone();
                                let active_cape_id_clone = active_cape_id.clone();
                                let _ = slint::invoke_from_event_loop(move || {
                                    let Some(ap) = ap_weak_1.upgrade() else { return };
                                    let apl = ap.global::<AppearanceLogic>();
                                    apl.set_active_skin_url(active_url_clone.into());

                                    let (capes_ui, active_cape_name, active_cape_preview) = capes_temp
                                        .into_iter()
                                        .fold(
                                            (Vec::new(), String::from("No Cape"), slint::Image::default()),
                                            |(mut vec, mut name, mut preview), t| {
                                                let img = t.buffer.as_ref()
                                                    .map_or_else(slint::Image::default, |b| slint::Image::from_rgba8(b.clone()));
                                                if t.id == active_cape_id_clone {
                                                    name = if !t.alias.is_empty() { t.alias.clone() } else { t.id.clone() };
                                                    preview = img.clone();
                                                }
                                                vec.push(CapeData {
                                                    id: t.id.into(),
                                                    alias: t.alias.into(),
                                                    state: if t.state == crate::mc_types::McState::Active { "ACTIVE".into() } else { "INACTIVE".into() },
                                                    url: t.url.into(),
                                                    preview: img,
                                                });
                                                (vec, name, preview)
                                            },
                                        );

                                    apl.set_capes(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(capes_ui))));
                                    apl.set_has_capes(has_capes);
                                    apl.set_active_cape_id(active_cape_id_clone.clone().into());
                                    apl.set_selected_cape_id(active_cape_id_clone.into());
                                    apl.set_selected_cape_name(active_cape_name.into());
                                    apl.set_selected_cape_preview(active_cape_preview);
                                });

                                let Ok(resp) = reqwest::get(&active_skin_url).await else { return };
                                let Ok(bytes) = resp.bytes().await else { return };
                                let Ok(img) = image::load_from_memory(&bytes) else { return };
                                let is_slim = detect_is_slim(&img);

                                let cape_img = if let Some(cape) = profile.capes.iter().find(|c| c.id == active_cape_id) {
                                    fetch_cape_bytes_cached(&cape.id, &cape.url).await
                                        .and_then(|b| image::load_from_memory(&b).ok())
                                } else {
                                    None
                                };

                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Ok(mut guard) = active_renderer_spawn.lock() {
                                        let mut renderer = crate::skin_renderer::SkinRenderer::new(img);
                                        renderer.set_cape(cape_img);
                                        *guard = Some(renderer);
                                    }
                                    if let Some(ap) = ap_weak.upgrade() {
                                        let apl = ap.global::<AppearanceLogic>();
                                        apl.set_skin_variant(if is_slim { "slim".into() } else { "classic".into() });
                                        apl.set_has_preview(true);
                                    }
                                });
                            }
                        });
                    }

                    let _ = ap.show();
                }
            });
}

async fn auto_detect_variant(skin_url: &str) -> Option<String> {
    let bytes = if skin_url.starts_with("file://") {
        let parsed = url::Url::parse(skin_url).ok()?;
        let path = parsed.to_file_path().ok()?;
        std::fs::read(&path).ok()?
    } else {
        reqwest::get(skin_url)
            .await
            .ok()?
            .bytes()
            .await
            .ok()?
            .to_vec()
    };
    let img = image::load_from_memory(&bytes).ok()?;
    Some(if detect_is_slim(&img) {
        "slim".to_string()
    } else {
        "classic".to_string()
    })
}

fn update_cape_cache(new_profile: Option<&crate::mc_types::McProfile>, cape_id_to_apply: &str) {
    let Ok(paths) = crate::mc_paths::McPaths::new() else {
        return;
    };
    let f = paths.capes_dir().join("profile_cache.json");
    if let Some(profile) = new_profile {
        let _ = std::fs::write(
            &f,
            serde_json::to_string_pretty(profile).unwrap_or_default(),
        );
        return;
    }
    let Ok(s) = std::fs::read_to_string(&f) else {
        return;
    };
    let Ok(mut p) = serde_json::from_str::<crate::mc_types::McProfile>(&s) else {
        return;
    };
    p.capes.iter_mut().for_each(|c| {
        c.state = if c.id == cape_id_to_apply {
            crate::mc_types::McState::Active
        } else {
            crate::mc_types::McState::Inactive
        };
    });
    let _ = std::fs::write(&f, serde_json::to_string_pretty(&p).unwrap_or_default());
}

fn handle_browse_skin_file(
    ap_weak: slint::Weak<AppearanceWindow>,
    active_renderer: std::sync::Arc<std::sync::Mutex<Option<crate::skin_renderer::SkinRenderer>>>,
) {
    let result = rfd::FileDialog::new()
        .add_filter("PNG Image", &["png"])
        .set_title("Select Skin File")
        .pick_file();

    let Some(path) = result else {
        return;
    };
    let path_str = path.to_string_lossy().to_string();
    let Some(ap) = ap_weak.upgrade() else {
        return;
    };
    let apl = ap.global::<AppearanceLogic>();

    let Ok(img) = image::open(&path) else {
        apl.set_upload_status("Failed to read image file".into());
        apl.set_upload_is_error(true);
        apl.set_show_result_dialog(true);
        apl.set_has_preview(false);
        apl.set_selected_skin_path("".into());
        return;
    };

    use image::GenericImageView;
    let (w, h) = img.dimensions();
    if w != 64 || (h != 64 && h != 32) {
        apl.set_upload_status(
            format!("Invalid skin size ({}x{}), must be 64x64 or 64x32", w, h).into(),
        );
        apl.set_upload_is_error(true);
        apl.set_show_result_dialog(true);
        apl.set_has_preview(false);
        apl.set_selected_skin_path("".into());
        return;
    }

    apl.set_selected_skin_path(path_str.into());
    let is_slim = detect_is_slim(&img);
    if let Ok(mut guard) = active_renderer.lock() {
        let old_cape = guard.as_ref().and_then(|r| r.get_cape());
        let mut new_renderer = crate::skin_renderer::SkinRenderer::new(img);
        new_renderer.set_cape_rgba(old_cape);
        *guard = Some(new_renderer);
    }
    apl.set_skin_variant(if is_slim {
        "slim".into()
    } else {
        "classic".into()
    });
    apl.set_has_preview(true);
}

fn handle_select_cape(
    cape_id: slint::SharedString,
    ap_weak: slint::Weak<AppearanceWindow>,
    renderer_lock: std::sync::Arc<std::sync::Mutex<Option<crate::skin_renderer::SkinRenderer>>>,
) {
    let cape_id = cape_id.to_string();
    let _ = slint::invoke_from_event_loop(move || {
        let Some(ap) = ap_weak.upgrade() else {
            return;
        };
        let apl = ap.global::<AppearanceLogic>();
        apl.set_selected_cape_id(cape_id.clone().into());

        if cape_id.is_empty() {
            if let Ok(mut guard) = renderer_lock.lock()
                && let Some(r) = guard.as_mut()
            {
                r.set_cape(None);
            }
            apl.set_selected_cape_name("No Cape".into());
            apl.set_selected_cape_preview(Default::default());
            return;
        }

        let capes = apl.get_capes();
        let (url, cape_name) = (0..capes.row_count())
            .filter_map(|i| capes.row_data(i))
            .find(|c| c.id == cape_id)
            .map(|c| {
                let name = if !c.alias.to_string().is_empty() {
                    c.alias.to_string()
                } else {
                    c.id.to_string()
                };
                (c.url.to_string(), name)
            })
            .unwrap_or_else(|| (String::new(), cape_id.clone()));
        apl.set_selected_cape_name(cape_name.into());

        if url.is_empty() {
            return;
        }

        let renderer_lock2 = renderer_lock.clone();
        let cape_id_clone = cape_id.clone();
        let ap_weak2 = ap_weak.clone();
        tokio::spawn(async move {
            let Some(bytes) = fetch_cape_bytes_cached(&cape_id_clone, &url).await else {
                return;
            };
            let Ok(img) = image::load_from_memory(&bytes) else {
                return;
            };
            let (raw_pixels, w, h) = create_cape_preview_raw(&img);

            let _ = slint::invoke_from_event_loop(move || {
                if let Ok(mut guard) = renderer_lock2.lock()
                    && let Some(r) = guard.as_mut()
                {
                    r.set_cape(Some(img));
                }
                if let Some(ap) = ap_weak2.upgrade() {
                    let slint_img = slint::Image::from_rgba8(
                        slint::SharedPixelBuffer::clone_from_slice(&raw_pixels, w, h),
                    );
                    ap.global::<AppearanceLogic>()
                        .set_selected_cape_preview(slint_img);
                }
            });
        });
    });
}

async fn fetch_cape_bytes_cached(cape_id: &str, cape_url: &str) -> Option<Vec<u8>> {
    let paths = crate::mc_paths::McPaths::new().ok()?;
    let cache_file = paths.capes_dir().join(format!("{}.png", cape_id));
    if cache_file.exists() {
        return std::fs::read(&cache_file).ok();
    }
    let bytes = reqwest::get(cape_url)
        .await
        .ok()?
        .bytes()
        .await
        .ok()?
        .to_vec();
    let _ = std::fs::write(&cache_file, &bytes);
    Some(bytes)
}

fn reload_skin_history_ui(apl: &AppearanceLogic) {
    let Ok(paths) = crate::mc_paths::McPaths::new() else {
        return;
    };
    let history_file = paths.skins_history_file();
    let history = crate::skin_history::SkinHistory::load(&history_file);
    apl.set_skin_history(slint::ModelRc::from(std::rc::Rc::new(
        slint::VecModel::from(get_ui_skins(&paths, &history)),
    )));
}

fn update_avatar_in_ui(
    main_ui_weak: &slint::Weak<MainApp>,
    avatar_path: Option<&std::path::Path>,
    username: &str,
) {
    let Some(path) = avatar_path else { return };
    let Some(main_ui) = main_ui_weak.upgrade() else {
        return;
    };
    let Ok(img) = slint::Image::load_from_path(path) else {
        return;
    };
    let pal = main_ui.global::<PageAccountLogic>();
    let mut active = pal.get_active_account();
    active.avatar = img.clone();
    pal.set_active_account(active);
    let mut accounts: Vec<_> = pal.get_accounts().iter().collect();
    if let Some(row) = accounts.iter_mut().find(|r| r.username == username) {
        row.avatar = img;
    }
    pal.set_accounts(slint::ModelRc::from(std::rc::Rc::new(
        slint::VecModel::from(accounts),
    )));
}

async fn fetch_skin_bytes(path_str: &str, url_str: &str) -> Result<Vec<u8>, String> {
    if !path_str.is_empty() {
        std::fs::read(path_str).map_err(|_| "Failed to read local file".to_string())
    } else if !url_str.is_empty() {
        let resp = reqwest::get(url_str)
            .await
            .map_err(|_| "Failed to connect to URL".to_string())?;
        let bytes = resp
            .bytes()
            .await
            .map_err(|_| "Failed to download skin".to_string())?;
        Ok(bytes.to_vec())
    } else {
        Err("No file or URL selected".to_string())
    }
}
