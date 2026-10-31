use orders_core::reference_data::{
    OrderType, OrderTypeDraft, Physician, PhysicianDraft, ReferenceDataRepository,
};
use tauri::State;

#[tauri::command]
pub fn list_physicians(
    reference_data: State<'_, ReferenceDataRepository>,
) -> Result<Vec<Physician>, String> {
    reference_data
        .list_physicians(true)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_physician(
    reference_data: State<'_, ReferenceDataRepository>,
    physician: PhysicianDraft,
) -> Result<Physician, String> {
    reference_data
        .save_physician(physician)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_physician(
    reference_data: State<'_, ReferenceDataRepository>,
    physician_id: String,
) -> Result<bool, String> {
    reference_data
        .delete_physician(&physician_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_order_types(
    reference_data: State<'_, ReferenceDataRepository>,
) -> Result<Vec<OrderType>, String> {
    reference_data
        .list_order_types(true)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_order_type(
    reference_data: State<'_, ReferenceDataRepository>,
    order_type: OrderTypeDraft,
) -> Result<OrderType, String> {
    reference_data
        .save_order_type(order_type)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_order_type(
    reference_data: State<'_, ReferenceDataRepository>,
    order_type_id: String,
) -> Result<bool, String> {
    reference_data
        .delete_order_type(&order_type_id)
        .map_err(|error| error.to_string())
}
