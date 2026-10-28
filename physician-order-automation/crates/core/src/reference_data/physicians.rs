use super::{
    ReferenceDataRepository,
    helpers::{random_id, required_text},
};
use crate::{CoreError, CoreResult};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Physician {
    pub id: String,
    pub name: String,
    pub npi: String,
    pub active: bool,
    pub sort_order: i32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicianDraft {
    pub id: Option<String>,
    pub name: String,
    pub npi: String,
    pub active: bool,
    pub sort_order: i32,
}

impl ReferenceDataRepository {
    pub fn get_active_physician(&self, id: &str) -> CoreResult<Physician> {
        let conn = self.database.open()?;
        conn.query_row(
            "SELECT id, name, npi, active, sort_order FROM physicians WHERE id = ?1 AND active = 1",
            [id],
            |row| {
                Ok(Physician {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    npi: row.get(2)?,
                    active: row.get(3)?,
                    sort_order: row.get(4)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| CoreError::Validation("selected physician is unavailable".into()))
    }

    pub fn list_physicians(&self, include_inactive: bool) -> CoreResult<Vec<Physician>> {
        let conn = self.database.open()?;
        let mut statement = conn.prepare(
            "SELECT id, name, npi, active, sort_order FROM physicians WHERE ?1 OR active = 1 ORDER BY sort_order, name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([include_inactive], |row| {
            Ok(Physician {
                id: row.get(0)?,
                name: row.get(1)?,
                npi: row.get(2)?,
                active: row.get(3)?,
                sort_order: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CoreError::from)
    }

    pub fn save_physician(&self, draft: PhysicianDraft) -> CoreResult<Physician> {
        let name = required_text(draft.name, "physician name", 200)?;
        if draft.sort_order < 0 {
            return Err(CoreError::Validation(
                "display order cannot be negative".into(),
            ));
        }
        let npi = draft.npi.trim().to_string();
        if npi.len() != 10 || !npi.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(CoreError::Validation(
                "NPI must contain exactly 10 digits".into(),
            ));
        }

        let mut conn = self.database.open()?;
        let tx = conn.transaction()?;
        let duplicate_name: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM physicians WHERE name = ?1 COLLATE NOCASE AND (?2 IS NULL OR id <> ?2))",
            (&name, &draft.id),
            |row| row.get(0),
        )?;
        let duplicate_npi: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM physicians WHERE npi = ?1 AND (?2 IS NULL OR id <> ?2))",
            (&npi, &draft.id),
            |row| row.get(0),
        )?;
        if duplicate_name {
            return Err(CoreError::Validation(
                "a physician with this name already exists".into(),
            ));
        }
        if duplicate_npi {
            return Err(CoreError::Validation(
                "a physician with this NPI already exists".into(),
            ));
        }

        let id = match draft.id {
            Some(id) => {
                let changed = tx.execute(
                    "UPDATE physicians SET name = ?1, npi = ?2, active = ?3, sort_order = ?4 WHERE id = ?5",
                    (&name, &npi, draft.active, draft.sort_order, &id),
                )?;
                if changed == 0 {
                    return Err(CoreError::Validation(
                        "physician record was not found".into(),
                    ));
                }
                id
            }
            None => {
                let id = random_id()?;
                tx.execute(
                    "INSERT INTO physicians (id, name, npi, active, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
                    (&id, &name, &npi, draft.active, draft.sort_order),
                )?;
                id
            }
        };
        tx.commit()?;
        Ok(Physician {
            id,
            name,
            npi,
            active: draft.active,
            sort_order: draft.sort_order,
        })
    }

    pub fn delete_physician(&self, id: &str) -> CoreResult<bool> {
        let conn = self.database.open()?;
        let changed = conn.execute("DELETE FROM physicians WHERE id = ?1", [id])?;
        Ok(changed > 0)
    }
}
