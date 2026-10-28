use orders_core::db::{Database, DatabaseStatus};
use tauri::State;

#[tauri::command]
pub fn database_status(database: State<'_, Database>) -> Result<DatabaseStatus, String> {
    database
        .status()
        .map_err(|_| "Database is unavailable or could not be opened.".to_string())
}
