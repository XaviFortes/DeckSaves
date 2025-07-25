use tauri::{Manager, AppHandle, tray::{TrayIconBuilder, TrayIconEvent}, menu::{Menu, MenuItem, PredefinedMenuItem}, Emitter, Listener};
use tracing::info;

mod commands;
use commands::AppState;

fn create_tray_menu(app: &AppHandle) -> Menu<tauri::Wry> {
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>).unwrap();
    let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>).unwrap();
    let sync_all = MenuItem::with_id(app, "sync_all", "Sync All Games", true, None::<&str>).unwrap();
    
    Menu::with_items(app, &[
        &show,
        &PredefinedMenuItem::separator(app).unwrap(),
        &sync_all,
        &PredefinedMenuItem::separator(app).unwrap(),
        &quit,
    ]).unwrap()
}

fn handle_tray_event(app: &AppHandle, event: TrayIconEvent) {
    match event {
        TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, .. } => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        _ => {}
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting DeckSaves GUI");

    // Initialize app state
    let app_state = AppState::new().expect("Failed to initialize app state");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::enable_local_storage,
            commands::set_aws_credentials,
            commands::set_aws_credentials_and_config,
            commands::get_aws_credentials,
            commands::debug_credentials,
            commands::test_command,
            commands::get_config_path,
            commands::get_games,
            commands::get_games_with_status,
            commands::add_game,
            commands::add_game_with_dialogs,
            commands::update_game,
            commands::remove_game,
            commands::sync_game,
            commands::sync_game_with_feedback,
            commands::test_aws_connection,
            commands::start_watching_game,
            commands::stop_watching_game,
            commands::get_watching_games,
            commands::validate_path,
            commands::install_service,
            commands::get_system_info,
            commands::show_notification,
            commands::detect_steam_games,
            commands::get_steam_save_suggestions,
            commands::add_steam_game_to_config,
            commands::test_steam_detection,
            // Versioned sync commands
            commands::sync_game_with_versioning,
            commands::get_version_history,
            commands::restore_version,
            commands::pin_version,
            commands::cleanup_old_versions,
            commands::delete_version,
            commands::test_s3_connection,
        ])
        .setup(|app| {
            // Setup tray icon
            let tray_menu = create_tray_menu(app.handle());
            let _tray = TrayIconBuilder::with_id("main-tray")
                .menu(&tray_menu)
                .icon(app.default_window_icon().unwrap().clone())
                .on_tray_icon_event(move |tray, event| {
                    handle_tray_event(tray.app_handle(), event);
                })
                .on_menu_event(move |app, event| {
                    match event.id().as_ref() {
                        "quit" => {
                            std::process::exit(0);
                        }
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "sync_all" => {
                            // Trigger sync all via event
                            let _ = app.emit("sync-all-requested", ());
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // Setup notifications
            let app_handle = app.handle().clone();
            
            // Register app event listeners
            let _app_handle = app_handle.clone();
            app.listen("sync-all-requested", move |_event| {
                // Emit to frontend to trigger sync all
                let _ = _app_handle.emit("sync-all-trigger", ());
            });

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // Hide window instead of closing when user clicks X
                let _ = window.hide();
                api.prevent_close();
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
