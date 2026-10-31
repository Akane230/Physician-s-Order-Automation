use orders_core::parsing::demographics::ParsedDemographics;

#[tauri::command]
pub fn parse_demographics(text: String) -> Result<ParsedDemographics, String> {
    orders_core::parsing::demographics::parse_demographics(&text).map_err(|error| error.to_string())
}
