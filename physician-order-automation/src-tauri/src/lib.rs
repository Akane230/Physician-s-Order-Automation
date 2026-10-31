// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
mod commands;

use orders_core::{
    db::Database, patients::PatientRepository, reference_data::ReferenceDataRepository,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let db_path = app.path().app_data_dir()?.join("physician-orders.db");
            let database = Database::new(db_path);
            // Opening the database creates it, applies pending migrations, and
            // verifies its SQLCipher key before the UI is shown.
            drop(database.open().map_err(|error| {
                std::io::Error::other(format!("database initialization failed: {error}"))
            })?);
            app.manage(PatientRepository::new(database.clone()));
            app.manage(ReferenceDataRepository::new(database.clone()));
            app.manage(database);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::ping,
            commands::database::database_status,
            commands::patients::search_patients,
            commands::patients::get_patient,
            commands::patients::save_patient,
            commands::patients::delete_patient,
            commands::parsing::parse_demographics,
            commands::diagnoses::parse_diagnoses,
            commands::reference_data::list_physicians,
            commands::reference_data::save_physician,
            commands::reference_data::delete_physician,
            commands::reference_data::list_order_types,
            commands::reference_data::save_order_type,
            commands::reference_data::delete_order_type,
            commands::orders::export_order
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
