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

    for line in input.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Some((raw_label, value)) = split_label(line) else {
            continue;
        };
        let value = clean_value(value);
        if value.is_empty() {
            continue;
        }
        match normalize_label(raw_label).as_str() {
            "patientname" | "name" | "fullname" | "clientname" | "beneficiaryname" => {
                set_if_missing(&mut result.full_name, value)
            }
            "firstname" | "givenname" => set_if_missing(&mut first_name, value),
            "lastname" | "surname" | "familyname" => set_if_missing(&mut last_name, value),
            "gender" | "sex" => {
                if let Some(gender) = normalize_gender(&value) {
                    set_if_missing(&mut result.gender, gender.to_string());
                }
            }
            "address" | "streetaddress" | "patientaddress" | "mailingaddress" => {
                set_if_missing(&mut result.address, value)
            }
            "dob" | "dateofbirth" | "birthdate" => {
                if let Some(dob) = normalize_date(&value) {
                    set_if_missing(&mut result.dob, dob);
                }
            }
            "phone" | "phoneno" | "phonenumber" | "cell" | "cellphone" | "mobilenumber"
            | "homephone" | "home" => set_if_missing(&mut result.phone, value),
            "medicare" | "medicareid" | "medicarenumber" | "medicareno" => {
                set_if_missing(&mut result.medicare, value)
            }
            "otherinsurance" | "secondaryinsurance" | "supplementalinsurance" => {
                set_if_missing(&mut result.other_insurance, value)
            }
            _ => {}
        }
    }

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
        "Other Insurance",
        "Phone Number",
        "Phone No.",
        "Medicare ID",
        "Medicare No.",
        "Gender",
        "Sex",
        "Address",
        "DOB",
        "Phone",
        "Medicare",
        "Name",
    ] {
        let Some(value) = line.get(label.len()..) else {
            continue;
        };
        if line[..label.len()].eq_ignore_ascii_case(label)
            && value.chars().next().is_some_and(char::is_whitespace)
        {
            return Some((&line[..label.len()], value.trim()));
        }
    }
    None
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
