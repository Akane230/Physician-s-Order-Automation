use crate::{CoreError, CoreResult};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const MAX_INPUT_CHARS: usize = 30_000;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    pub code: Option<String>,
    pub description: String,
}

/// Converts pasted diagnosis text into a deduplicated list without saving or rewriting it.
pub fn parse_diagnoses(input: &str) -> CoreResult<Vec<Diagnosis>> {
    if input.chars().count() > MAX_INPUT_CHARS {
        return Err(CoreError::Validation(format!(
            "pasted diagnoses exceed {MAX_INPUT_CHARS} characters"
        )));
    }

    static ICD_CODE: OnceLock<Regex> = OnceLock::new();
    let pattern = ICD_CODE.get_or_init(|| {
        Regex::new(r"(?i)\b([A-Z][0-9][A-Z0-9](?:\.[A-Z0-9]{1,4})?)\b")
            .expect("valid ICD-10-CM code pattern")
    });

    let mut output = Vec::new();
    for raw_line in input.lines() {
        let line = raw_line
            .trim()
            .trim_start_matches(|character: char| matches!(character, '-' | '*' | '•' | '–'))
            .trim();
        if line.is_empty() || is_heading(line) {
            continue;
        }

        let codes = pattern
            .captures_iter(line)
            .filter_map(|capture| {
                let matched = capture.get(1)?;
                Some((
                    matched.start(),
                    matched.end(),
                    matched.as_str().to_ascii_uppercase(),
                ))
            })
            .collect::<Vec<_>>();

        if codes.is_empty() {
            push_unique(
                &mut output,
                Diagnosis {
                    code: None,
                    description: clean(line),
                },
            );
            continue;
        }

        for (index, (start, end, code)) in codes.iter().enumerate() {
            let segment_end = codes.get(index + 1).map_or(line.len(), |next| next.0);
            let before = clean(&line[..*start]);
            let after = clean(&line[*end..segment_end]);
            let before = strip_label(&before);
            let description = if after.is_empty() { before } else { after };
            push_unique(
                &mut output,
                Diagnosis {
                    code: Some(code.clone()),
                    description,
                },
            );
        }
    }
    Ok(output)
}

fn is_heading(line: &str) -> bool {
    matches!(
        line.trim()
            .trim_end_matches(':')
            .to_ascii_lowercase()
            .as_str(),
        "diagnosis" | "diagnoses" | "icd-10-cm" | "icd 10 cm"
    )
}

fn strip_label(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    for label in [
        "icd-10-cm condition",
        "icd 10 cm condition",
        "diagnosis",
        "condition",
    ] {
        if lower.ends_with(label) {
            return value[..value.len() - label.len()]
                .trim()
                .trim_end_matches([':', '-', '–', '|'])
                .trim()
                .to_string();
        }
    }
    value.to_string()
}

fn clean(value: &str) -> String {
    value
        .trim()
        .trim_matches([':', '-', '–', '|', ';', ','])
        .trim()
        .to_string()
}

fn push_unique(output: &mut Vec<Diagnosis>, diagnosis: Diagnosis) {
    if diagnosis.code.is_none() && diagnosis.description.is_empty() {
        return;
    }
    let duplicate = output
        .iter()
        .any(|existing| match (&existing.code, &diagnosis.code) {
            (Some(left), Some(right)) => left == right,
            (None, None) => normalize(&existing.description) == normalize(&diagnosis.description),
            _ => false,
        });
    if !duplicate {
        output.push(diagnosis);
    }
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}
