use crate::{CoreError, CoreResult, db::Database};
use chrono::{NaiveDate, Utc};
use rusqlite::{OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

const MAX_SEARCH_RESULTS: i64 = 50;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatientDraft {
    pub full_name: String,
    pub gender: Option<String>,
    pub address: Option<String>,
    /// ISO format: YYYY-MM-DD.
    pub dob: Option<String>,
    pub phone: Option<String>,
    pub medicare: Option<String>,
    pub other_insurance: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatientRecord {
    pub id: String,
    pub full_name: String,
    pub gender: Option<String>,
    pub address: Option<String>,
    pub dob: Option<String>,
    pub phone: Option<String>,
    pub medicare: Option<String>,
    pub other_insurance: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "status", content = "patientId")]
pub enum SavePatientResult {
    Created(String),
    Updated(String),
    Conflict(String),
}

#[derive(Clone)]
pub struct PatientRepository {
    database: Database,
}

impl PatientRepository {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn search(&self, query: &str) -> CoreResult<Vec<PatientRecord>> {
        let query = query.trim();
        if query.chars().count() > 128 {
            return Err(CoreError::Validation("search text is too long".into()));
        }
        let conn = self.database.open()?;
        let mut statement = conn.prepare(
            r#"SELECT id, full_name, gender, address, dob, phone, medicare, other_insurance, created_at, updated_at
               FROM patients
               WHERE ?1 = '' OR instr(lower(full_name), lower(?1)) > 0
                  OR instr(COALESCE(medicare, ''), ?1) > 0
                  OR instr(COALESCE(phone, ''), ?1) > 0
               ORDER BY last_name COLLATE NOCASE, first_name COLLATE NOCASE
               LIMIT ?2"#,
        )?;
        let rows = statement.query_map((query, MAX_SEARCH_RESULTS), patient_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(CoreError::from)
    }

    pub fn get(&self, patient_id: &str) -> CoreResult<Option<PatientRecord>> {
        let conn = self.database.open()?;
        conn.query_row(
            r#"SELECT id, full_name, gender, address, dob, phone, medicare, other_insurance, created_at, updated_at
               FROM patients WHERE id = ?1"#,
            [patient_id],
            patient_from_row,
        )
        .optional()
        .map_err(CoreError::from)
    }

    pub fn save(&self, draft: PatientDraft, user_id: &str) -> CoreResult<SavePatientResult> {
        let patient = ValidatedPatient::try_from(draft)?;
        let mut conn = self.database.open()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = Utc::now().to_rfc3339();

        let medicare_match = if let Some(ref normalized) = patient.medicare_normalized {
            tx.query_row(
                "SELECT id, name_normalized, dob FROM patients WHERE medicare_normalized = ?1 LIMIT 1",
                [normalized],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, Option<String>>(2)?)),
            ).optional()?
        } else {
            None
        };

        let matching_id = if let Some((id, name, dob)) = medicare_match {
            if name.as_deref() != Some(patient.name_normalized.as_str()) || dob != patient.dob {
                return Ok(SavePatientResult::Conflict(id));
            }
            Some(id)
        } else if let Some(ref dob) = patient.dob {
            tx.query_row(
                "SELECT id FROM patients WHERE name_normalized = ?1 AND dob = ?2 LIMIT 1",
                (&patient.name_normalized, dob),
                |row| row.get::<_, String>(0),
            )
            .optional()?
        } else {
            None
        };

        let result = if let Some(id) = matching_id {
            tx.execute(
                "UPDATE patients SET last_name = ?1, first_name = ?2, full_name = ?3, gender = ?4, address = ?5, dob = ?6, phone = ?7, medicare = ?8, other_insurance = ?9, medicare_normalized = ?10, name_normalized = ?11, updated_at = ?12, updated_by = ?13 WHERE id = ?14",
                (
                    &patient.last_name, &patient.first_name, &patient.full_name,
                    &patient.gender, &patient.address, &patient.dob, &patient.phone,
                    &patient.medicare, &patient.other_insurance, &patient.medicare_normalized,
                    &patient.name_normalized, &now, user_id, &id,
                ),
            )?;
            SavePatientResult::Updated(id)
        } else {
            let id = random_id()?;
            tx.execute(
                r#"INSERT INTO patients (id, last_name, first_name, full_name, gender, address, dob, phone, medicare, other_insurance, medicare_normalized, name_normalized, created_at, updated_at, created_by, updated_by)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13, ?14, ?14)"#,
                (
                    &id, &patient.last_name, &patient.first_name, &patient.full_name,
                    &patient.gender, &patient.address, &patient.dob, &patient.phone,
                    &patient.medicare, &patient.other_insurance, &patient.medicare_normalized,
                    &patient.name_normalized, &now, user_id,
                ),
            )?;
            SavePatientResult::Created(id)
        };
        tx.commit()?;
        Ok(result)
    }

    pub fn delete(&self, patient_id: &str) -> CoreResult<bool> {
        let conn = self.database.open()?;
        let deleted = conn.execute("DELETE FROM patients WHERE id = ?1", [patient_id])?;
        Ok(deleted > 0)
    }
}

struct ValidatedPatient {
    full_name: String,
    first_name: String,
    last_name: String,
    name_normalized: String,
    gender: Option<String>,
    address: Option<String>,
    dob: Option<String>,
    phone: Option<String>,
    medicare: Option<String>,
    medicare_normalized: Option<String>,
    other_insurance: Option<String>,
}

impl TryFrom<PatientDraft> for ValidatedPatient {
    type Error = CoreError;

    fn try_from(draft: PatientDraft) -> Result<Self, Self::Error> {
        let full_name = required_text(draft.full_name, "patient name", 200)?;
        let words = full_name.split_whitespace().collect::<Vec<_>>();
        let first_name = words.first().copied().unwrap_or_default().to_string();
        let last_name = words.last().copied().unwrap_or_default().to_string();
        let name_normalized = normalize_name(&full_name);
        let gender = optional_text(draft.gender, 32)?;
        if gender
            .as_deref()
            .is_some_and(|value| !matches!(value, "Male" | "Female" | "Other"))
        {
            return Err(CoreError::Validation(
                "gender must be Male, Female, Other, or blank".into(),
            ));
        }
        let dob = optional_text(draft.dob, 10)?;
        if dob
            .as_deref()
            .is_some_and(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err())
        {
            return Err(CoreError::Validation(
                "date of birth must use YYYY-MM-DD format".into(),
            ));
        }
        let phone = optional_text(draft.phone, 64)?;
        let medicare = optional_text(draft.medicare, 64)?;
        let medicare_normalized = medicare
            .as_deref()
            .map(normalize_identifier)
            .filter(|value| !value.is_empty());
        Ok(Self {
            full_name,
            first_name,
            last_name,
            name_normalized,
            gender,
            address: optional_text(draft.address, 500)?,
            dob,
            phone,
            medicare,
            medicare_normalized,
            other_insurance: optional_text(draft.other_insurance, 200)?,
        })
    }
}

fn patient_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PatientRecord> {
    Ok(PatientRecord {
        id: row.get(0)?,
        full_name: row.get(1)?,
        gender: row.get(2)?,
        address: row.get(3)?,
        dob: row.get(4)?,
        phone: row.get(5)?,
        medicare: row.get(6)?,
        other_insurance: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn required_text(value: String, field: &str, max_chars: usize) -> CoreResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max_chars {
        return Err(CoreError::Validation(format!(
            "{field} is required and must be at most {max_chars} characters"
        )));
    }
    Ok(value.to_string())
}

fn optional_text(value: Option<String>, max_chars: usize) -> CoreResult<Option<String>> {
    let Some(value) = value else { return Ok(None) };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > max_chars {
        return Err(CoreError::Validation(format!(
            "field exceeds {max_chars} characters"
        )));
    }
    Ok(Some(value.to_string()))
}

fn normalize_name(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

fn normalize_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

fn random_id() -> CoreResult<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| CoreError::Random)?;
    let id = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    bytes.zeroize();
    Ok(id)
}

use zeroize::Zeroize;
