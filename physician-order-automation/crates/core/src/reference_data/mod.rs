mod helpers;
mod order_types;
mod physicians;

use crate::db::Database;

#[derive(Clone)]
pub struct ReferenceDataRepository {
    database: Database,
}

impl ReferenceDataRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }
}

pub use order_types::{OrderType, OrderTypeDraft};
pub use physicians::{Physician, PhysicianDraft};
