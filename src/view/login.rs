use super::*;

pub fn setup_login_logic(ui: &MainApp) {
    if let Ok(Some(session)) = SessionData::load_session()
        && !session.mc_username().is_empty()
    {
        let is_expired = *session.mc_token_expires_at() < chrono::Utc::now().timestamp();
        let username = session.mc_username().clone();
        let token = session.minecraft_access_token().clone();
        let ui_weak_for_init = ui.as_weak();

        tokio::spawn(async move {
            let avatar_path = fetch_avatar_path(&username).await;
            let (authenticator_text, status_text) = if is_expired {
                ("Microsoft".to_string(), "Offline".to_string())
            } else {
                let api = crate::mc_api::McAction::new().authenticate(&token);
                match api.check_game_ownership().await {
                    Ok(true) => ("Microsoft (Premium)".to_string(), "Online".to_string()),
                    Ok(false) => ("Microsoft (Unpaid)".to_string(), "Online".to_string()),
                    Err(_) => ("Microsoft".to_string(), "Offline".to_string()),
                }
            };

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_for_init.upgrade() {
                    let avatar_img = avatar_path
                        .and_then(|p| slint::Image::load_from_path(&p).ok())
                        .unwrap_or_default();
                    let pal = ui.global::<PageAccountLogic>();
                    let row = AccountRow {
                        checked: true,
                        authenticator: authenticator_text.into(),
                        username: username.into(),
                        status: status_text.into(),
                        avatar: avatar_img,
                    };
                    pal.set_active_account(row.clone());
                    pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(vec![row]))));
                }
            });
        });
    }

    let page_account_logic_clone = ui.global::<PageAccountLogic>();
    page_account_logic_clone.on_open_browser_url(|url| {
        let _ = open::that(url.as_str());
    });
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_flag_clone = Arc::clone(&cancel_flag);
    let page_account_logic_clone = ui.global::<PageAccountLogic>();
    page_account_logic_clone.on_cancel_login(move || {
        cancel_flag_clone.store(true, Ordering::SeqCst);
    });
    let ui_weak = ui.as_weak();
    let page_account_logic = ui.global::<PageAccountLogic>();
    page_account_logic.on_login_with_microsoft(move || {
        let ui_weak = ui_weak.clone();
        let cancel_flag = Arc::clone(&cancel_flag);
        cancel_flag.store(false, Ordering::SeqCst);
        if let Some(ui) = ui_weak.upgrade() {
            let pal = ui.global::<PageAccountLogic>();
            pal.set_login_url("".into());
            pal.set_login_status_text("Generating secure login link...".into());
            pal.set_is_error(false);
            pal.set_is_logging_in(true);
        }
        tokio::spawn(async move {
            let ui_weak_for_url = ui_weak.clone();
            let on_url_ready = move |url: String| {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_for_url.upgrade() {
                        let pal = ui.global::<PageAccountLogic>();
                        pal.set_login_url(url.into());
                        pal.set_login_status_text(
                            "Please complete verification in your browser.".into(),
                        );
                    }
                });
            };
            match mc_token::set_token_in_native_store(on_url_ready).await {
                Ok(_new_token) => {
                    if cancel_flag.load(Ordering::SeqCst) {
                        return;
                    }
                    let api = crate::mc_api::McAction::new().authenticate(&_new_token);
                    let is_premium = api.check_game_ownership().await.unwrap_or(false);
                    let username = SessionData::load_session()
                        .ok()
                        .flatten()
                        .map(|s| s.mc_username().clone())
                        .unwrap_or_default();

                    let avatar_path = fetch_avatar_from_mojang(&_new_token, &username, true)
                        .await
                        .map(|(p, _)| p);

                    handle_login_success(ui_weak.clone(), username, is_premium, avatar_path);
                }
                Err(e) => {
                    handle_login_error(ui_weak.clone(), e.to_string());
                }
            }
        });
    });
    let ui_weak_remove = ui.as_weak();
    ui.global::<PageAccountLogic>().on_remove_account(move || {
        let Some(ui) = ui_weak_remove.upgrade() else {
            return;
        };
        let pal = ui.global::<PageAccountLogic>();
        let idx = pal.get_selected_index();
        if idx < 0 {
            return;
        }

        let accounts: Vec<AccountRow> = pal.get_accounts().iter().collect();
        let new_accounts: Vec<AccountRow> = accounts
            .into_iter()
            .enumerate()
            .filter(|(i, _)| *i != idx as usize)
            .map(|(_, r)| r)
            .collect();

        pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(new_accounts.clone()))));
        pal.set_selected_index(-1);

        if let Some(default_acc) = new_accounts.iter().find(|r| r.checked) {
            pal.set_active_account(default_acc.clone());
        } else {
            let mut guest = pal.get_active_account();
            guest.username = "Guest".into();
            guest.authenticator = "No Account".into();
            guest.status = "Offline".into();
            guest.checked = false;
            pal.set_active_account(guest);
        }

        let _ = mc_token::SessionData::delete_session();
    });

    let ui_weak_set_default = ui.as_weak();
    ui.global::<PageAccountLogic>()
        .on_set_default_account(move || {
            let Some(ui) = ui_weak_set_default.upgrade() else {
                return;
            };
            let pal = ui.global::<PageAccountLogic>();
            let idx = pal.get_selected_index();
            if idx < 0 {
                return;
            }

            let accounts: Vec<AccountRow> = pal.get_accounts().iter().collect();
            let new_accounts: Vec<AccountRow> = accounts
                .into_iter()
                .enumerate()
                .map(|(i, mut r)| {
                    r.checked = i == idx as usize;
                    r
                })
                .collect();

            if let Some(default_acc) = new_accounts.iter().find(|r| r.checked) {
                pal.set_active_account(default_acc.clone());
            }

            pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(new_accounts))));
        });

    let ui_weak_unset_default = ui.as_weak();
    ui.global::<PageAccountLogic>()
        .on_unset_default_account(move || {
            let Some(ui) = ui_weak_unset_default.upgrade() else {
                return;
            };
            let pal = ui.global::<PageAccountLogic>();
            let accounts: Vec<AccountRow> = pal.get_accounts().iter().collect();
            let new_accounts: Vec<AccountRow> = accounts
                .into_iter()
                .map(|mut r| {
                    r.checked = false;
                    r
                })
                .collect();

            let mut guest = pal.get_active_account();
            guest.username = "Guest".into();
            guest.authenticator = "No Account".into();
            guest.status = "Offline".into();
            guest.checked = false;
            pal.set_active_account(guest);

            pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(new_accounts))));
        });

    let ui_weak_add_offline = ui.as_weak();
    ui.global::<PageAccountLogic>()
        .on_confirm_add_offline_account(move |username| {
            let Some(ui) = ui_weak_add_offline.upgrade() else {
                return;
            };
            let pal = ui.global::<PageAccountLogic>();
            let username = username.to_string();

            let mut accounts: Vec<AccountRow> = pal.get_accounts().iter().collect();
            let new_row = AccountRow {
                checked: accounts.is_empty(),
                authenticator: "Offline".into(),
                username: username.clone().into(),
                status: "Ready".into(),
                avatar: slint::Image::default(),
            };

            if new_row.checked {
                pal.set_active_account(new_row.clone());
            }

            accounts.push(new_row);
            pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(accounts))));
        });

    let ui_weak_refresh = ui.as_weak();
    ui.global::<PageAccountLogic>().on_refresh_account(move || {
        let Some(ui) = ui_weak_refresh.upgrade() else {
            return;
        };
        let pal = ui.global::<PageAccountLogic>();
        let idx = pal.get_selected_index();
        if idx < 0 {
            return;
        }

        let mut accounts: Vec<AccountRow> = pal.get_accounts().iter().collect();
        let mut row = accounts[idx as usize].clone();

        if row.authenticator == "Offline" {
            row.status = "Ready".into();
            accounts[idx as usize] = row.clone();
            pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(accounts))));
            if row.checked {
                pal.set_active_account(row);
            }
        } else {
            let ui_weak_async = ui_weak_refresh.clone();
            let username = row.username.to_string();
            tokio::spawn(async move {
                let Ok(Some(session)) = SessionData::load_session() else {
                    return;
                };
                let token = session.minecraft_access_token().clone();
                let api = crate::mc_api::McAction::new().authenticate(&token);
                let ownership = api.check_game_ownership().await.unwrap_or(false);
                let status_text = if ownership { "Online" } else { "Offline" };
                let avatar_path = fetch_avatar_from_mojang(&token, &username, false)
                    .await
                    .map(|(p, _)| p);

                handle_refresh_success(ui_weak_async, idx as usize, status_text, avatar_path);
            });
        }
    });
}

fn handle_login_success(
    ui_weak: slint::Weak<MainApp>,
    username: String,
    is_premium: bool,
    avatar_path: Option<std::path::PathBuf>,
) {
    let authenticator_text = if is_premium {
        "Microsoft (Premium)"
    } else {
        "Microsoft (Unpaid)"
    };
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let avatar_img = avatar_path
                .and_then(|p| slint::Image::load_from_path(&p).ok())
                .unwrap_or_default();
            let pal = ui.global::<PageAccountLogic>();
            pal.set_is_logging_in(false);
            if !username.is_empty() {
                let row = AccountRow {
                    checked: true,
                    authenticator: authenticator_text.into(),
                    username: username.into(),
                    status: "Online".into(),
                    avatar: avatar_img,
                };
                pal.set_active_account(row.clone());
                pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(vec![row]))));
            }
        }
    });
}

fn handle_login_error(ui_weak: slint::Weak<MainApp>, e: String) {
    let error_msg = format!("Login failed: {}", e);
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let pal = ui.global::<PageAccountLogic>();
            pal.set_is_logging_in(false);
            pal.set_is_error(true);
            pal.set_login_url("".into());
            pal.set_login_status_text(error_msg.into());
        }
    });
}

fn handle_refresh_success(
    ui_weak: slint::Weak<MainApp>,
    idx: usize,
    status_text: &str,
    avatar_path: Option<std::path::PathBuf>,
) {
    let status_text = status_text.to_string();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let pal = ui.global::<PageAccountLogic>();
            let mut accounts: Vec<AccountRow> = pal.get_accounts().iter().collect();
            if idx < accounts.len() {
                let mut row = accounts[idx].clone();
                row.status = status_text.into();
                if let Some(p) = avatar_path.and_then(|p| slint::Image::load_from_path(&p).ok()) {
                    row.avatar = p;
                }
                accounts[idx] = row.clone();
                pal.set_accounts(ModelRc::from(Rc::new(VecModel::from(accounts))));
                if row.checked {
                    pal.set_active_account(row);
                }
            }
        }
    });
}
