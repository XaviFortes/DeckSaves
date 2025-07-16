use tauri::{Manager, AppHandle, tray::{TrayIconBuilder, TrayIconEvent}, menu::{Menu, MenuItem}};
use tracing::info;

mod commands;
use commands::AppState;

fn create_tray_menu() -> Menu<tauri::Wry> {
    let quit = MenuItem::with_id("quit", "Quit", true, None::<&str>);
    let show = MenuItem::with_id("show", "Show", true, None::<&str>);
    let sync_all = MenuItem::with_id("sync_all", "Sync All Games", true, None::<&str>);
    
    Menu::with_items(&[
        &show,
        &MenuItem::separator(),
        &sync_all,
        &MenuItem::separator(),
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
        TrayIconEvent::MenuItemClick { id, .. } => {
            match id.as_ref() {
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
            commands::get_config_path,
            commands::get_games,
            commands::add_game,
            commands::update_game,
            commands::remove_game,
            commands::sync_game,
            commands::start_watching_game,
            commands::stop_watching_game,
            commands::get_watching_games,
            commands::select_folder,
            commands::select_file,
            commands::validate_path,
            commands::install_service,
            commands::get_system_info,
            commands::show_notification,
        ])
        .setup(|app| {
            // Setup tray icon
            let tray_menu = create_tray_menu();
            let _tray = TrayIconBuilder::with_id("main-tray")
                .menu(&tray_menu)
                .icon(app.default_window_icon().unwrap().clone())
                .on_tray_icon_event(move |tray, event| {
                    handle_tray_event(tray.app_handle(), event);
                })
                .build(app)?;

            // Setup notifications
            let app_handle = app.handle();
            
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
