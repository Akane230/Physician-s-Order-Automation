use orders_core::{
    auth::AuthService,
    patients::{PatientDraft, PatientRecord, PatientRepository, SavePatientResult},
};
use tauri::State;

#[tauri::command]
pub fn search_patients(
    auth: State<'_, AuthService>,
    patients: State<'_, PatientRepository>,
    query: String,
) -> Result<Vec<PatientRecord>, String> {
    auth.require_authenticated()
        .map_err(|error| error.to_string())?;
    patients.search(&query).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_patient(
    auth: State<'_, AuthService>,
    patients: State<'_, PatientRepository>,
    patient_id: String,
) -> Result<Option<PatientRecord>, String> {
    auth.require_authenticated()
        .map_err(|error| error.to_string())?;
    patients.get(&patient_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_patient(
    auth: State<'_, AuthService>,
    patients: State<'_, PatientRepository>,
    patient: PatientDraft,
) -> Result<SavePatientResult, String> {
    let user = auth
        .require_authenticated()
        .map_err(|error| error.to_string())?;
    patients
        .save(patient, &user.id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_patient(
    auth: State<'_, AuthService>,
    patients: State<'_, PatientRepository>,
    patient_id: String,
) -> Result<bool, String> {
    auth.require_admin().map_err(|error| error.to_string())?;
    patients
        .delete(&patient_id)
        .map_err(|error| error.to_string())
}
