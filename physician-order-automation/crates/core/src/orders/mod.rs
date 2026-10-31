mod pdf;

use crate::{
    CoreError, CoreResult,
    parsing::diagnoses::Diagnosis,
    patients::PatientDraft,
    reference_data::{OrderType, Physician},
};
use chrono::NaiveDate;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderRequest {
    pub patient: PatientDraft,
    pub physician_id: String,
    pub order_type_id: String,
    /// ISO format: YYYY-MM-DD.
    pub order_date: String,
    pub diagnoses: Vec<Diagnosis>,
    #[serde(default)]
    pub order_details: String,
    pub comments: String,
}

pub fn render_order_pdf(
    request: &OrderRequest,
    physician: &Physician,
    order_type: &OrderType,
) -> CoreResult<Vec<u8>> {
    let order_date = NaiveDate::parse_from_str(&request.order_date, "%Y-%m-%d")
        .map_err(|_| CoreError::Validation("order date must use YYYY-MM-DD format".into()))?;
    if request.patient.full_name.trim().is_empty()
        || request.patient.full_name.chars().count() > 200
    {
        return Err(CoreError::Validation(
            "patient name is required and must be at most 200 characters".into(),
        ));
    }
    if request.comments.chars().count() > 5_000 {
        return Err(CoreError::Validation(
            "comments exceed 5000 characters".into(),
        ));
    }
    if request.order_details.chars().count() > 10_000 {
        return Err(CoreError::Validation(
            "order details exceed 10000 characters".into(),
        ));
    }
    if request.diagnoses.len() > 100 {
        return Err(CoreError::Validation(
            "no more than 100 diagnoses may be included".into(),
        ));
    }

    let mut patient_name = request.patient.full_name.trim().to_string();
    if let Some(gender) = request
        .patient
        .gender
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        patient_name.push_str(&format!(" ({})", gender.trim()));
    }
    let display_date = order_date.format("%m/%d/%Y").to_string();
    let display_dob = request
        .patient
        .dob
        .as_deref()
        .and_then(|dob| NaiveDate::parse_from_str(dob, "%Y-%m-%d").ok())
        .map(|dob| dob.format("%m/%d/%Y").to_string())
        .unwrap_or_default();
    let body = order_type
        .md_order_body
        .replace("{{patient_name}}", request.patient.full_name.trim())
        .replace("{{order_date}}", &display_date)
        .replace("{{physician_name}}", &physician.name)
        .replace("{{physician_npi}}", &physician.npi);

    pdf::render_pdf(PdfOrder {
        patient_name,
        address: request.patient.address.as_deref().unwrap_or_default(),
        dob: &display_dob,
        phone: request.patient.phone.as_deref().unwrap_or_default(),
        medicare: request.patient.medicare.as_deref().unwrap_or_default(),
        other_insurance: request
            .patient
            .other_insurance
            .as_deref()
            .unwrap_or_default(),
        order_date: display_date,
        physician_name: &physician.name,
        physician_npi: &physician.npi,
        diagnoses: &request.diagnoses,
        order_type_name: &order_type.name,
        md_order_body: &body,
        order_details: &request.order_details,
        comments: &request.comments,
    })
}

struct PdfOrder<'a> {
    patient_name: String,
    address: &'a str,
    dob: &'a str,
    phone: &'a str,
    medicare: &'a str,
    other_insurance: &'a str,
    order_date: String,
    physician_name: &'a str,
    physician_npi: &'a str,
    diagnoses: &'a [Diagnosis],
    order_type_name: &'a str,
    md_order_body: &'a str,
    order_details: &'a str,
    comments: &'a str,
}

pub fn export_filename(patient_name: &str, order_date: &str) -> CoreResult<String> {
    let date = NaiveDate::parse_from_str(order_date, "%Y-%m-%d")
        .map_err(|_| CoreError::Validation("order date must use YYYY-MM-DD format".into()))?;
    let mut words = patient_name.split_whitespace();
    let first = words.next().unwrap_or("Patient");
    let last = words.last().unwrap_or(first);
    let last_name = filename_part(last);
    let initial = filename_part(&first.chars().next().unwrap_or('P').to_string());
    Ok(format!(
        "PhysicianOrder_{last_name}_{initial}_{}.pdf",
        date.format("%Y-%m-%d")
    ))
}

fn filename_part(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .chars()
        .take(40)
        .collect::<String>()
        .if_empty("Patient")
}

trait EmptyFallback {
    fn if_empty(self, fallback: &str) -> String;
}

impl EmptyFallback for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}
