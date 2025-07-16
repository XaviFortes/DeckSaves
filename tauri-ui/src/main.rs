use tauri::{CustomMenuItem, SystemTray, SystemTrayMenu, SystemTrayEvent, Manager, AppHandle};
use tracing::info;

mod commands;
use commands::AppState;

fn create_system_tray() -> SystemTray {
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let show = CustomMenuItem::new("show".to_string(), "Show");
    let sync_all = CustomMenuItem::new("sync_all".to_string(), "Sync All Games");
    
    let tray_menu = SystemTrayMenu::new()
        .add_item(show)
        .add_native_item(tauri::SystemTrayMenuItem::Separator)
        .add_item(sync_all)
        .add_native_item(tauri::SystemTrayMenuItem::Separator)
        .add_item(quit);

    SystemTray::new().with_menu(tray_menu)
}

fn handle_system_tray_event(app: &AppHandle, event: SystemTrayEvent) {
    match event {
        SystemTrayEvent::LeftClick {
            position: _,
            size: _,
            ..
        } => {
            let window = app.get_window("main").unwrap();
            window.show().unwrap();
            window.set_focus().unwrap();
        }
        SystemTrayEvent::MenuItemClick { id, .. } => {
            match id.as_str() {
                "quit" => {
                    std::process::exit(0);
                }
                "show" => {
                    let window = app.get_window("main").unwrap();
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
                "sync_all" => {
                    // Trigger sync all via event
                    app.emit_all("sync-all-requested", {}).unwrap();
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
        .system_tray(create_system_tray())
        .on_system_tray_event(handle_system_tray_event)
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
            // Setup notifications
            let app_handle = app.handle();
            
            // Register app event listeners
            let _app_handle = app_handle.clone();
            app.listen_global("sync-all-requested", move |_event| {
                // Emit to frontend to trigger sync all
                _app_handle.emit_all("sync-all-trigger", {}).unwrap();
            });

            Ok(())
        })
        .on_window_event(|event| match event.event() {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // Hide window instead of closing when user clicks X
                event.window().hide().unwrap();
                api.prevent_close();
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
