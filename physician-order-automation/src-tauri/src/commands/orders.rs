use orders_core::{
    auth::AuthService,
    orders::{self, OrderRequest},
    reference_data::ReferenceDataRepository,
};
use std::fs;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub fn export_order(
    app: AppHandle,
    auth: State<'_, AuthService>,
    reference_data: State<'_, ReferenceDataRepository>,
    request: OrderRequest,
) -> Result<Option<String>, String> {
    auth.require_authenticated()
        .map_err(|error| error.to_string())?;
    let physician = reference_data
        .get_active_physician(&request.physician_id)
        .map_err(|error| error.to_string())?;
    let order_type = reference_data
        .get_active_order_type(&request.order_type_id)
        .map_err(|error| error.to_string())?;
    let pdf = orders::render_order_pdf(&request, &physician, &order_type)
        .map_err(|error| error.to_string())?;
    let file_name = orders::export_filename(&request.patient.full_name, &request.order_date)
        .map_err(|error| error.to_string())?;

    let Some(file_path) = app
        .dialog()
        .file()
        .add_filter("PDF document", &["pdf"])
        .set_file_name(&file_name)
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = file_path
        .into_path()
        .map_err(|error| format!("Could not access the selected file: {error}"))?;
    fs::write(&path, pdf).map_err(|error| format!("Could not save the PDF: {error}"))?;
    Ok(Some(file_name))
}
