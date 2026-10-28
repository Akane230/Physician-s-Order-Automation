#[tauri::command]
pub fn ping() -> String {
    orders_core::ping()
}
