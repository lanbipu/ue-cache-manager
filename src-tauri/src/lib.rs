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
            app.manage(commands::ddc_pak::UeJobRegistry::default());
            tracing::info!("UECM started, database at {}", db_path.display());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::machines::list_machines,
            commands::machines::add_machine,
            commands::machines::delete_machine,
            commands::machines::rename_machine,
            commands::machines::get_machine_detail,
            commands::discovery::scan_network,
            commands::discovery::add_discovered_machine,
            commands::discovery::refresh_machine,
            commands::credentials::list_credentials,
            commands::credentials::save_credential,
            commands::credentials::delete_credential,
            commands::env_vars::set_machine_env_var,
            commands::env_vars::get_machine_env_var,
            commands::env_vars::set_machine_env_var_with_credential,
            commands::env_vars::get_machine_env_var_with_credential,
            commands::ini_editor::read_ini_section,
            commands::ini_editor::set_ini_key,
            commands::ini_editor::read_ini_section_with_credential,
            commands::ini_editor::set_ini_key_with_credential,
            commands::batch::batch_set_env_var,
            commands::batch::batch_set_ini_key,
            commands::shares::create_share,
            commands::shares::inject_share_credential_to_clients,
            commands::shares::list_shares,
            commands::shares::delete_share,
            commands::projects::list_projects,
            commands::projects::list_project_locations,
            commands::projects::discover_projects,
            commands::projects::set_project_location,
            commands::projects::delete_project,
            commands::projects::delete_project_location,
            commands::projects::create_project_manual,
            commands::ddc_pak::generate_ddc_pak,
            commands::ddc_pak::cancel_ue_job,
            commands::ddc_pak::verify_pak_output,
            commands::ddc_pak::distribute_ddc_pak,
            commands::system::test_powershell_bridge,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
