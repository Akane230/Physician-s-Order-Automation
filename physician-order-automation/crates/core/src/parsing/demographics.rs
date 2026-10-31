use crate::{CoreError, CoreResult};
use chrono::NaiveDate;
use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;

const MAX_INPUT_CHARS: usize = 30_000;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedDemographics {
    pub full_name: Option<String>,
    pub gender: Option<String>,
    pub address: Option<String>,
    pub dob: Option<String>,
    pub phone: Option<String>,
    pub medicare: Option<String>,
    pub other_insurance: Option<String>,
    pub warnings: Vec<String>,
}

/// Extracts demographic fields with deterministic label matching and conservative fallbacks.
/// The input is processed in memory only and is never logged or persisted here.
pub fn parse_demographics(input: &str) -> CoreResult<ParsedDemographics> {
    if input.chars().count() > MAX_INPUT_CHARS {
        return Err(CoreError::Validation(format!(
            "pasted text exceeds {MAX_INPUT_CHARS} characters"
        )));
    }

    let mut result = ParsedDemographics::default();
    let mut first_name = None;
    let mut last_name = None;
    let mut unstructured_address = None;
    let mut street_address = None;
    let mut city = None;
    let mut state = None;
    let mut postal_code = None;
    let mut country = None;

    for line in input.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Some((raw_label, value)) = split_label(line) else {
            continue;
        };
        let value = clean_value(value);
        if value.is_empty() {
            continue;
        }
        match normalize_label(raw_label).as_str() {
            "patientname" | "name" | "fullname" | "clientname" | "beneficiaryname"
            | "membername" => set_if_missing(&mut result.full_name, value),
            "firstname" | "givenname" => set_if_missing(&mut first_name, value),
            "lastname" | "surname" | "familyname" => set_if_missing(&mut last_name, value),
            "gender" | "sex" => {
                if let Some(gender) = normalize_gender(&value) {
                    set_if_missing(&mut result.gender, gender.to_string());
                }
            }
            "streetaddress" | "patientaddress" | "mailingaddress" | "homeaddress"
            | "residentialaddress" => set_if_missing(&mut street_address, value),
            "address" => set_if_missing(&mut unstructured_address, value),
            "city" | "town" | "locality" => set_if_missing(&mut city, value),
            "state" | "province" | "stateprovince" | "region" => set_if_missing(&mut state, value),
            "zip" | "zipcode" | "postalcode" | "postcode" => {
                set_if_missing(&mut postal_code, value)
            }
            "country" | "countryregion" => set_if_missing(&mut country, value),
            "dob" | "dateofbirth" | "birthdate" => {
                if let Some(dob) = normalize_date(&value) {
                    set_if_missing(&mut result.dob, dob);
                }
            }
            "phone" | "phoneno" | "phonenumber" | "cell" | "cellphone" | "mobilenumber"
            | "homephone" | "home" | "mobile" => set_if_missing(&mut result.phone, value),
            "medicareid"
            | "medicarenumber"
            | "medicareno"
            | "medicarebeneficiaryidentifier"
            | "mbi" => set_if_missing(&mut result.medicare, value),
            "primaryinsuranceid"
            | "primaryinsuranceidnumber"
            | "primaryinsid"
            | "primaryid"
            | "primaryidnumber" => set_if_missing(&mut result.medicare, value),
            "secondaryinsuranceid"
            | "secondaryinsuranceidnumber"
            | "secondaryinsid"
            | "secondaryid"
            | "secondaryidnumber"
            | "otherinsuranceid"
            | "otherinsuranceidnumber" => set_if_missing(&mut result.other_insurance, value),
            _ => {}
        }
    }

    if let Some(raw_address) = unstructured_address {
        parse_composite_address(
            &raw_address,
            &mut street_address,
            &mut city,
            &mut state,
            &mut postal_code,
            &mut country,
        );
        if street_address.is_none() {
            street_address = Some(raw_address);
        }
    }

    result.address = format_address_parts(
        street_address.as_deref(),
        city.as_deref(),
        state.as_deref(),
        postal_code.as_deref(),
        country.as_deref(),
    );

    if result.full_name.is_none() {
        let name = match (first_name, last_name) {
            (Some(first), Some(last)) => Some(format!("{first} {last}")),
            (Some(name), None) | (None, Some(name)) => Some(name),
            (None, None) => find_unformatted_name(input),
        };
        result.full_name = name;
    }

    if result.dob.is_none() {
        result.dob = find_unformatted_date(input);
    }
    if result.gender.is_none() {
        result.gender = find_unformatted_gender(input);
    }
    if result.phone.is_none() {
        result.phone = find_unformatted_phone(input);
    }
    let (primary_id, secondary_id) = extract_section_insurance_ids(input);
    if result.medicare.is_none() {
        result.medicare = primary_id;
    }
    if result.other_insurance.is_none() {
        result.other_insurance = secondary_id;
    }

    if result.full_name.is_none() {
        result
            .warnings
            .push("Patient name was not identified; enter it manually.".into());
    }
    if result.dob.is_none() {
        result
            .warnings
            .push("Date of birth was not identified; check the field manually.".into());
    }
    if result.address.is_none()
        || street_address.is_none()
        || city.is_none()
        || state.is_none()
        || postal_code.is_none()
        || country.is_none()
    {
        result.warnings.push(
            "Review the imported address; any component that could not be identified was omitted."
                .into(),
        );
    }
    if !input.trim().is_empty() {
        result
            .warnings
            .push("Review every populated field before saving the patient.".into());
    }
    Ok(result)
}

fn split_label(line: &str) -> Option<(&str, &str)> {
    if let Some((label, value)) = line.split_once(':') {
        return Some((label, value));
    }
    if let Some((label, value)) = line.split_once('\t') {
        return Some((label, value));
    }
    for label in [
        "Patient Name",
        "Full Name",
        "First Name",
        "Last Name",
        "Date of Birth",
        "State/Province",
        "Postal Code",
        "Zip Code",
        "Street Address",
        "Home Address",
        "Mailing Address",
        "Other Insurance",
        "Secondary Insurance",
        "Secondary Payer",
        "Secondary Plan Name",
        "Other Insurance Plan",
        "Phone Number",
        "Phone No.",
        "Medicare ID",
        "Medicare No.",
        "Primary Insurance ID Number",
        "Primary Insurance ID",
        "Secondary Insurance ID Number",
        "Secondary Insurance ID",
        "Other Insurance ID Number",
        "Other Insurance ID",
        "ID Number",
        "ID #",
        "Country",
        "City",
        "State",
        "Gender",
        "Sex",
        "Address",
        "DOB",
        "Phone",
        "Cell #",
        "Home #",
        "Mobile Phone",
        "Name",
    ] {
        let Some(value) = line.get(label.len()..) else {
            continue;
        };
        if line[..label.len()].eq_ignore_ascii_case(label)
            && (value.is_empty() || value.chars().next().is_some_and(char::is_whitespace))
        {
            if label.eq_ignore_ascii_case("Address")
                && value
                    .trim_start()
                    .to_ascii_lowercase()
                    .starts_with("of accident occurrence")
            {
                continue;
            }
            return Some((&line[..label.len()], value.trim()));
        }
    }
    None
}

pub fn format_address_parts(
    street: Option<&str>,
    city: Option<&str>,
    state: Option<&str>,
    postal_code: Option<&str>,
    country: Option<&str>,
) -> Option<String> {
    fn clean(part: Option<&str>) -> Option<&str> {
        part.map(str::trim).filter(|part| !part.is_empty())
    }
    let mut parts = Vec::new();
    if let Some(street) = clean(street) {
        parts.push(street.to_string());
    }
    let city_state = match (clean(city), clean(state)) {
        (Some(city), Some(state)) => Some(format!("{city}, {state}")),
        (Some(city), None) => Some(city.to_string()),
        (None, Some(state)) => Some(state.to_string()),
        (None, None) => None,
    };
    if let Some(city_state) = city_state {
        parts.push(city_state);
    }
    for part in [clean(postal_code), clean(country)].into_iter().flatten() {
        parts.push(part.to_string());
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

fn parse_composite_address(
    value: &str,
    street: &mut Option<String>,
    city: &mut Option<String>,
    state: &mut Option<String>,
    postal_code: &mut Option<String>,
    country: &mut Option<String>,
) {
    static US_ADDRESS: OnceLock<Regex> = OnceLock::new();
    let pattern = US_ADDRESS.get_or_init(|| {
        Regex::new(r"^\s*(.+?),\s*([^,]+),\s*([A-Z]{2})\s+(\d{5}(?:-\d{4})?)(?:,\s*(.+))?\s*$")
            .expect("valid composite US address pattern")
    });
    let Some(captures) = pattern.captures(value) else {
        return;
    };
    set_if_missing(
        street,
        captures
            .get(1)
            .map(|v| v.as_str().trim().to_string())
            .unwrap_or_default(),
    );
    set_if_missing(
        city,
        captures
            .get(2)
            .map(|v| v.as_str().trim().to_string())
            .unwrap_or_default(),
    );
    set_if_missing(
        state,
        captures
            .get(3)
            .map(|v| v.as_str().to_string())
            .unwrap_or_default(),
    );
    set_if_missing(
        postal_code,
        captures
            .get(4)
            .map(|v| v.as_str().to_string())
            .unwrap_or_default(),
    );
    if let Some(captured_country) = captures.get(5) {
        set_if_missing(country, captured_country.as_str().trim().to_string());
    }
}

/// The first two generic ID # rows in the insurance section are the primary
/// and secondary member identifiers. Payer IDs and plan names are not member IDs.
fn extract_section_insurance_ids(input: &str) -> (Option<String>, Option<String>) {
    let lines = input.lines().map(str::trim).collect::<Vec<_>>();
    let mut ids = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some((label, inline)) = split_label(line) else {
            continue;
        };
        let normalized_label = normalize_label(label);
        if !matches!(normalized_label.as_str(), "id" | "idnumber" | "idno")
            || (normalized_label == "id" && !label.contains('#'))
        {
            continue;
        }
        let inline = inline.trim().trim_start_matches('#').trim();
        let value = if is_identifier(inline) {
            Some(inline.to_string())
        } else {
            lines
                .get(index + 1)
                .map(|value| value.trim())
                .filter(|value| is_identifier(value))
                .map(|value| value.to_string())
        };
        ids.push(value);
    }
    (
        ids.first().cloned().flatten(),
        ids.get(1).cloned().flatten(),
    )
}

fn is_identifier(value: &str) -> bool {
    let candidate = value.trim().trim_matches([':', '#']);
    candidate.len() >= 3
        && candidate
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        && !matches!(
            candidate.to_ascii_lowercase().as_str(),
            "group" | "group#" | "plan" | "name" | "payer"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_structured_and_confident_composite_addresses_without_filling_missing_parts() {
        let parsed = parse_demographics(
            "Street Address: 7800 West Airport Apt 811\nCity: Houston\nState: TX\nZip Code: 77071\nCountry: UNITED STATES",
        ).unwrap();
        assert_eq!(
            parsed.address.as_deref(),
            Some("7800 West Airport Apt 811, Houston, TX, 77071, UNITED STATES")
        );

        let composite =
            parse_demographics("Address: 14 Sample Road, Dallas, TX 75001, USA").unwrap();
        assert_eq!(
            composite.address.as_deref(),
            Some("14 Sample Road, Dallas, TX, 75001, USA")
        );

        let uncertain = parse_demographics("Address: Near the old library").unwrap();
        assert_eq!(uncertain.address.as_deref(), Some("Near the old library"));

        let accident_label =
            parse_demographics("Address of Accident Occurrence\nWork site").unwrap();
        assert_eq!(accident_label.address, None);
    }

    #[test]
    fn maps_primary_and_secondary_insurance_ids_but_not_plan_names_or_payer_ids() {
        let parsed = parse_demographics(
            "Primary Insurance\nPayer ID: 04412\nID #: 2XX2XX1XX00\nPlan Name: Medicare TX\nSecondary Insurance\nPayer ID: 88888\nID #: ABC123456\nPlan Name: Sample Coverage",
        ).unwrap();
        assert_eq!(parsed.medicare.as_deref(), Some("2XX2XX1XX00"));
        assert_eq!(parsed.other_insurance.as_deref(), Some("ABC123456"));

        let insurer_is_not_an_id = parse_demographics(
            "Primary Insurance\nInsurer: Medicare TX\nPayer ID: 04412\nID #: 2XX2XX1XX00\nSecondary Insurance\nInsurer: Sample Plan\nID #: ABC123456",
        )
        .unwrap();
        assert_eq!(
            insurer_is_not_an_id.medicare.as_deref(),
            Some("2XX2XX1XX00")
        );
        assert_ne!(insurer_is_not_an_id.medicare.as_deref(), Some("TX"));

        let names_only =
            parse_demographics("Primary Insurance: Medicare TX\nSecondary Payer: Sample Coverage")
                .unwrap();
        assert_eq!(names_only.medicare, None);
        assert_eq!(names_only.other_insurance, None);
    }
}

fn normalize_label(label: &str) -> String {
    label
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn clean_value(value: &str) -> String {
    value.trim().trim_matches(['|', ';']).trim().to_string()
}

fn set_if_missing(destination: &mut Option<String>, value: String) {
    if destination.is_none() {
        *destination = Some(value);
    }
}

fn normalize_gender(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "male" | "m" => Some("Male"),
        "female" | "f" => Some("Female"),
        "other" | "unknown" | "nonbinary" | "non-binary" => Some("Other"),
        _ => None,
    }
}

fn normalize_date(value: &str) -> Option<String> {
    ["%m/%d/%Y", "%m/%d/%y", "%Y-%m-%d", "%B %d, %Y", "%b %d, %Y"]
        .iter()
        .find_map(|format| NaiveDate::parse_from_str(value.trim(), format).ok())
        .map(|date| date.format("%Y-%m-%d").to_string())
}

fn find_unformatted_date(input: &str) -> Option<String> {
    static DATE_PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = DATE_PATTERN.get_or_init(|| {
        Regex::new(r"\b(?:\d{1,2}[/-]\d{1,2}[/-]\d{2,4}|\d{4}-\d{2}-\d{2})\b")
            .expect("valid date pattern")
    });
    pattern
        .find_iter(input)
        .find_map(|matched| normalize_date(matched.as_str()))
}

fn find_unformatted_gender(input: &str) -> Option<String> {
    static GENDER_PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = GENDER_PATTERN.get_or_init(|| {
        Regex::new(r"(?i)\b(male|female|non[- ]?binary)\b").expect("valid gender pattern")
    });
    pattern
        .captures(input)
        .and_then(|capture| normalize_gender(capture.get(1)?.as_str()))
        .map(str::to_string)
}

fn find_unformatted_phone(input: &str) -> Option<String> {
    static PHONE_PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PHONE_PATTERN.get_or_init(|| {
        Regex::new(r"(?:\+?1[\s.-]?)?(?:\(\d{3}\)|\b\d{3})[\s.-]?\d{3}[\s.-]?\d{4}\b")
            .expect("valid phone pattern")
    });
    pattern
        .find(input)
        .map(|matched| matched.as_str().trim().to_string())
}

fn find_unformatted_name(input: &str) -> Option<String> {
    input.lines().map(str::trim).find_map(|line| {
        let candidate = line.trim_matches(|character: char| character == ',' || character == ';');
        let words = candidate.split_whitespace().collect::<Vec<_>>();
        if !(2..=4).contains(&words.len()) || !words.iter().all(|word| is_name_token(word)) {
            return None;
        }
        let first_word = normalize_label(words[0]);
        if matches!(
            first_word.as_str(),
            "city"
                | "state"
                | "country"
                | "zip"
                | "postalcode"
                | "phone"
                | "gender"
                | "sex"
                | "address"
                | "patient"
                | "first"
                | "last"
                | "primary"
                | "secondary"
                | "insurance"
                | "medicare"
                | "provider"
        ) {
            return None;
        }
        let normalized = normalize_label(candidate);
        if matches!(
            normalized.as_str(),
            "patientdemographics"
                | "demographics"
                | "unitedstates"
                | "primaryprovider"
                | "patientname"
                | "firstname"
                | "lastname"
                | "dateofbirth"
                | "otherinsurance"
        ) {
            return None;
        }
        Some(candidate.to_string())
    })
}

fn is_name_token(word: &str) -> bool {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_uppercase()
        && chars.all(|character| character.is_alphabetic() || matches!(character, '-' | '\''))
}
