use orders_core::parsing::diagnoses::Diagnosis;

#[tauri::command]
pub fn parse_diagnoses(text: String) -> Result<Vec<Diagnosis>, String> {
    orders_core::parsing::diagnoses::parse_diagnoses(&text).map_err(|error| error.to_string())
}
