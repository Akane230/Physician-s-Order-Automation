use orders_core::{
    auth::AuthService,
    reference_data::{
        OrderType, OrderTypeDraft, Physician, PhysicianDraft, ReferenceDataRepository,
    },
};
use tauri::State;

#[tauri::command]
pub fn list_physicians(
    auth: State<'_, AuthService>,
    reference_data: State<'_, ReferenceDataRepository>,
) -> Result<Vec<Physician>, String> {
    let user = auth
        .require_authenticated()
        .map_err(|error| error.to_string())?;
    reference_data
        .list_physicians(user.role == "admin")
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_physician(
    auth: State<'_, AuthService>,
    reference_data: State<'_, ReferenceDataRepository>,
    physician: PhysicianDraft,
) -> Result<Physician, String> {
    auth.require_admin().map_err(|error| error.to_string())?;
    reference_data
        .save_physician(physician)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_physician(
    auth: State<'_, AuthService>,
    reference_data: State<'_, ReferenceDataRepository>,
    physician_id: String,
) -> Result<bool, String> {
    auth.require_admin().map_err(|error| error.to_string())?;
    reference_data
        .delete_physician(&physician_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_order_types(
    auth: State<'_, AuthService>,
    reference_data: State<'_, ReferenceDataRepository>,
) -> Result<Vec<OrderType>, String> {
    let user = auth
        .require_authenticated()
        .map_err(|error| error.to_string())?;
    reference_data
        .list_order_types(user.role == "admin")
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_order_type(
    auth: State<'_, AuthService>,
    reference_data: State<'_, ReferenceDataRepository>,
    order_type: OrderTypeDraft,
) -> Result<OrderType, String> {
    auth.require_admin().map_err(|error| error.to_string())?;
    reference_data
        .save_order_type(order_type)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_order_type(
    auth: State<'_, AuthService>,
    reference_data: State<'_, ReferenceDataRepository>,
    order_type_id: String,
) -> Result<bool, String> {
    auth.require_admin().map_err(|error| error.to_string())?;
    reference_data
        .delete_order_type(&order_type_id)
        .map_err(|error| error.to_string())
}
