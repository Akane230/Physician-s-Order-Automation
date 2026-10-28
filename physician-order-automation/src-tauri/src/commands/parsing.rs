use orders_core::{auth::AuthService, parsing::demographics::ParsedDemographics};
use tauri::State;

#[tauri::command]
pub fn parse_demographics(
    auth: State<'_, AuthService>,
    text: String,
) -> Result<ParsedDemographics, String> {
    auth.require_authenticated()
        .map_err(|error| error.to_string())?;
    orders_core::parsing::demographics::parse_demographics(&text).map_err(|error| error.to_string())
}
