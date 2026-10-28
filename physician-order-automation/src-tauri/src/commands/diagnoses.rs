use orders_core::{auth::AuthService, parsing::diagnoses::Diagnosis};
use tauri::State;

#[tauri::command]
pub fn parse_diagnoses(
    auth: State<'_, AuthService>,
    text: String,
) -> Result<Vec<Diagnosis>, String> {
    auth.require_authenticated()
        .map_err(|error| error.to_string())?;
    orders_core::parsing::diagnoses::parse_diagnoses(&text).map_err(|error| error.to_string())
}
