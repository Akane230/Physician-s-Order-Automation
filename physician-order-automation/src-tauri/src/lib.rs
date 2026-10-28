// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use orders_core::db::{Database, DatabaseStatus};
use tauri::{Manager, State};

#[tauri::command]
fn ping() -> String {
    orders_core::ping()
}

#[tauri::command]
fn database_status(database: State<'_, Database>) -> Result<DatabaseStatus, String> {
    database
        .status()
        .map_err(|_| "Database is unavailable or could not be opened.".to_string())
}

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
            app.manage(database);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![ping, database_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
