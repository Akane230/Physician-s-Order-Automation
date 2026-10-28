use super::{
    ReferenceDataRepository,
    helpers::{optional_text, random_id, required_text},
};
use crate::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderType {
    pub id: String,
    pub name: String,
    pub md_order_body: String,
    pub active: bool,
    pub sort_order: i32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderTypeDraft {
    pub id: Option<String>,
    pub name: String,
    pub md_order_body: String,
    pub active: bool,
    pub sort_order: i32,
}

impl ReferenceDataRepository {
    pub fn list_order_types(&self, include_inactive: bool) -> CoreResult<Vec<OrderType>> {
        let conn = self.database.open()?;
        let mut statement = conn.prepare(
            "SELECT id, name, md_order_body, active, sort_order FROM order_types WHERE ?1 OR active = 1 ORDER BY sort_order, name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([include_inactive], |row| {
            Ok(OrderType {
                id: row.get(0)?,
                name: row.get(1)?,
                md_order_body: row.get(2)?,
                active: row.get(3)?,
                sort_order: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CoreError::from)
    }

    pub fn save_order_type(&self, draft: OrderTypeDraft) -> CoreResult<OrderType> {
        let name = required_text(draft.name, "order type name", 120)?;
        if draft.sort_order < 0 {
            return Err(CoreError::Validation(
                "display order cannot be negative".into(),
            ));
        }
        let md_order_body = optional_text(draft.md_order_body, "order template", 20_000)?;
        let mut conn = self.database.open()?;
        let tx = conn.transaction()?;
        let duplicate_name: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM order_types WHERE name = ?1 COLLATE NOCASE AND (?2 IS NULL OR id <> ?2))",
            (&name, &draft.id),
            |row| row.get(0),
        )?;
        if duplicate_name {
            return Err(CoreError::Validation(
                "an order type with this name already exists".into(),
            ));
        }

        let id = match draft.id {
            Some(id) => {
                let changed = tx.execute(
                    "UPDATE order_types SET name = ?1, md_order_body = ?2, active = ?3, sort_order = ?4 WHERE id = ?5",
                    (&name, &md_order_body, draft.active, draft.sort_order, &id),
                )?;
                if changed == 0 {
                    return Err(CoreError::Validation(
                        "order type record was not found".into(),
                    ));
                }
                id
            }
            None => {
                let id = random_id()?;
                tx.execute(
                    "INSERT INTO order_types (id, name, md_order_body, active, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
                    (&id, &name, &md_order_body, draft.active, draft.sort_order),
                )?;
                id
            }
        };
        tx.commit()?;
        Ok(OrderType {
            id,
            name,
            md_order_body,
            active: draft.active,
            sort_order: draft.sort_order,
        })
    }

    pub fn delete_order_type(&self, id: &str) -> CoreResult<bool> {
        let conn = self.database.open()?;
        let changed = conn.execute("DELETE FROM order_types WHERE id = ?1", [id])?;
        Ok(changed > 0)
    }
}
