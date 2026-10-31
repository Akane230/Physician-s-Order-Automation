use orders_core::patients::{PatientDraft, PatientRecord, PatientRepository, SavePatientResult};
use tauri::State;

#[tauri::command]
pub fn search_patients(
    patients: State<'_, PatientRepository>,
    query: String,
) -> Result<Vec<PatientRecord>, String> {
    patients.search(&query).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_patient(
    patients: State<'_, PatientRepository>,
    patient_id: String,
) -> Result<Option<PatientRecord>, String> {
    patients.get(&patient_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_patient(
    patients: State<'_, PatientRepository>,
    patient: PatientDraft,
) -> Result<SavePatientResult, String> {
    patients.save(patient).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_patient(
    patients: State<'_, PatientRepository>,
    patient_id: String,
) -> Result<bool, String> {
    patients
        .delete(&patient_id)
        .map_err(|error| error.to_string())
}
