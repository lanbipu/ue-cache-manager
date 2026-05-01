pub mod commands;
pub mod core;
pub mod data;
pub mod error;

use std::path::PathBuf;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let db_path: PathBuf = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app_data_dir")
                .join("uecm.sqlite");

            std::fs::create_dir_all(db_path.parent().unwrap())?;
            let db = data::open(&db_path)?;
            {
                let mut conn = db.lock().unwrap();
                data::schema::migrate(&mut conn)?;
            }
            app.manage(db);
            tracing::info!("UECM started, database at {}", db_path.display());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::machines::list_machines,
            commands::machines::add_machine,
            commands::machines::delete_machine,
            commands::system::test_powershell_bridge,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
