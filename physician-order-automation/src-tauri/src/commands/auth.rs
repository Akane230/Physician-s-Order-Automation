use orders_core::auth::{AuthService, AuthStatus, AuthenticatedUser};
use tauri::State;

#[tauri::command]
pub fn auth_status(auth: State<'_, AuthService>) -> Result<AuthStatus, String> {
    auth.status().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn setup_initial_admin(
    auth: State<'_, AuthService>,
    username: String,
    display_name: String,
    password: String,
) -> Result<AuthenticatedUser, String> {
    auth.setup_initial_admin(&username, &display_name, password)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn login(
    auth: State<'_, AuthService>,
    username: String,
    password: String,
) -> Result<AuthenticatedUser, String> {
    auth.login(&username, password)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn logout(auth: State<'_, AuthService>) -> Result<(), String> {
    auth.logout().map_err(|error| error.to_string())
}
