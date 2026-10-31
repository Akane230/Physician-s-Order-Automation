use super::demographics::{ParsedDemographics, format_address_parts, parse_demographics};
use crate::{CoreError, CoreResult};
use lopdf::Document;
use zeroize::Zeroizing;

pub const MAX_PDF_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_PDF_PAGES: usize = 20;
const MAX_TEXT_CHARS: usize = 30_000;

/// Extract embedded PDF text and OCR pages without usable text through the supplied local callback.
pub fn parse_pdf_with_ocr<F>(bytes: &[u8], mut ocr_page: F) -> CoreResult<ParsedDemographics>
where
    F: FnMut(u32) -> Result<String, String>,
{
    if bytes.len() > MAX_PDF_BYTES {
        return Err(CoreError::Validation(
            "PDF exceeds the 20 MB import limit".into(),
        ));
    }
    let document = Document::load_mem(bytes).map_err(|_| {
        CoreError::Validation("Could not read this PDF. Choose an unprotected, valid PDF.".into())
    })?;
    let pages = document.get_pages();
    if pages.is_empty() || pages.len() > MAX_PDF_PAGES {
        return Err(CoreError::Validation(
            "PDF must contain between 1 and 20 pages".into(),
        ));
    }
    let page_numbers = pages.keys().copied().collect::<Vec<_>>();
    let mut text = Zeroizing::new(String::new());
    for page in page_numbers {
        let embedded = Zeroizing::new(
            document
                .extract_text_with_limit(&[page], MAX_TEXT_CHARS * 4)
                .unwrap_or_default(),
        );
        let has_usable_text_layer = embedded.chars().any(char::is_alphanumeric);
        let page_text = if has_usable_text_layer {
            embedded.to_string()
        } else {
            ocr_page(page).map_err(|_| CoreError::Validation(
                "This PDF has a page without usable embedded text and local OCR is unavailable or failed. Install Tesseract OCR and Poppler (pdftoppm), then try again.".into()
            ))?
        };
        let page_text = Zeroizing::new(page_text);
        text.push_str(&page_text);
        text.push('\n');
        if text.chars().count() > MAX_TEXT_CHARS {
            return Err(CoreError::Validation(
                "Extracted PDF text exceeds the import limit".into(),
            ));
        }
    }
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(CoreError::Validation(
            "Extracted PDF text exceeds the import limit".into(),
        ));
    }
    if text.trim().is_empty() {
        return Err(CoreError::Validation(
            "No readable text was found in this PDF".into(),
        ));
    }
    // Restrict PDF parsing to the patient demographics section. Intake PDFs often
    // continue with insurance and accident forms whose labels overlap patient fields.
    let patient_text = patient_demographics_section(&text);

    // Some PDF producers place each visual row in a separate text operation without
    // exposing line breaks in the extracted stream. Restore boundaries before known labels.
    // PDF text extraction can concatenate adjacent visual rows (for example,
    // `ExampleSex:`). Requiring a field delimiter after the label avoids splitting
    // ordinary words while still restoring those boundaries.
    let labels = regex::Regex::new(r"(?i)(?:patient\s+name|full\s+name|first\s+name|last\s+name|name|date\s+of\s+birth|birth\s+date|dob|gender|sex|street\s+address|home\s+address|address|city|state(?:\s*/\s*province)?|province|zip(?:\s+code)?|postal\s+code|country|phone(?:\s+number|\s+no\.?)?|mobile(?:\s+phone)?|cell(?:\s+phone)?|cell\s+#|home\s+#)(?:\s*[:\t]|\s+)")
        .expect("valid label boundary expression");
    let cleaned = Zeroizing::new(
        patient_text.chars()
            .map(|character| {
                if ('\u{fb00}'..='\u{fb06}').contains(&character) {
                    ' '
                } else {
                    character
                }
            })
            .collect::<String>(),
    );
    let normalized = Zeroizing::new(labels.replace_all(&cleaned, "\n$0").into_owned());
    let lines = normalized.lines().map(str::trim).collect::<Vec<_>>();
    // Extract only fields used by the order. Do not run the general paste parser
    // here: its broader label support can absorb unrelated text into address values.
    let first_name = field_value(&lines, &["first name"]);
    let last_name = field_value(&lines, &["last name"]);
    let full_name = field_value(&lines, &["patient name", "full name", "name"])
        .or_else(|| match (first_name, last_name) {
            (Some(first), Some(last)) => Some(format!("{first} {last}")),
            (Some(name), None) | (None, Some(name)) => Some(name),
            (None, None) => None,
        });
    let gender_value = field_value(&lines, &["gender", "sex"]);
    let gender = gender_value.and_then(|value| {
        parse_demographics(&format!("Gender: {value}"))
            .ok()
            .and_then(|parsed| parsed.gender)
    });
    let dob_value = field_value(&lines, &["date of birth", "birth date", "dob"]);
    let dob = dob_value.and_then(|value| {
        parse_demographics(&format!("DOB: {value}"))
            .ok()
            .and_then(|parsed| parsed.dob)
    });
    let phone_value = field_value(
        &lines,
        &["phone number", "phone no.", "mobile phone", "cell phone", "cell #", "home #", "phone", "mobile"],
    );
    let phone = phone_value.and_then(|value| {
        parse_demographics(&format!("Phone: {value}"))
            .ok()
            .and_then(|parsed| parsed.phone)
    });
    let street = field_value(&lines, &["street address", "home address", "address"]);
    let city = field_value(&lines, &["city"]);
    let state = field_value(&lines, &["state/province", "state", "province"]);
    let postal_code = field_value(&lines, &["zip", "zip code", "postal code"]);
    let country = field_value(&lines, &["country"]);
    let address = format_address_parts(
        street.as_deref(),
        city.as_deref(),
        state.as_deref(),
        postal_code.as_deref(),
        country.as_deref(),
    );
    let mut parsed = ParsedDemographics {
        full_name,
        gender,
        address,
        dob,
        phone,
        ..ParsedDemographics::default()
    };
    if parsed.gender.is_none() {
        parsed
            .warnings
            .push("Gender was not identified; check the field manually.".into());
    }
    if parsed.address.is_none() {
        parsed
            .warnings
            .push("Address was not identified; enter it manually if needed.".into());
    }
    if parsed.phone.is_none() {
        parsed
            .warnings
            .push("Phone was not identified; check the field manually.".into());
    }
    Ok(parsed)
}

fn patient_demographics_section(text: &str) -> &str {
    let lower = text.to_ascii_lowercase();
    let patient_start = lower.find("patient demographics").map(|index| {
        index + "patient demographics".len()
    });
    let start = patient_start.unwrap_or(0);
    let rest = &lower[start..];
    let section_end = ["primary insurance", "secondary insurance", "auto accident insurance", "worker's compensation", "workers compensation"]
        .iter()
        .filter_map(|heading| rest.find(heading))
        .min()
        .map(|offset| start + offset)
        .unwrap_or(text.len());
    &text[start..section_end]
}

fn field_value(lines: &[&str], labels: &[&str]) -> Option<String> {
    for (index, line) in lines.iter().enumerate() {
        let lower = line.to_ascii_lowercase();
        for label in labels {
            if lower == *label {
                return lines
                    .get(index + 1)
                    .map(|next| next.trim())
                    .filter(|value| !value.is_empty() && !is_supported_field_label(value))
                    .map(str::to_string);
            }
            if lower.starts_with(label)
                && lower[label.len()..]
                    .chars()
                    .next()
                    .is_some_and(char::is_whitespace)
            {
                let value = line[label.len()..].trim().trim_start_matches([':', '\t']).trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

fn is_supported_field_label(value: &str) -> bool {
    let value = value.trim().trim_end_matches(':').to_ascii_lowercase();
    [
        "patient name", "full name", "first name", "last name", "gender", "sex",
        "street address", "home address", "address", "country", "zip", "zip code",
        "postal code", "city", "state", "state/province", "province", "phone",
        "phone number", "phone no.", "mobile", "mobile phone", "cell phone", "cell #",
        "home #", "dob", "date of birth", "birth date",
    ]
    .contains(&value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{
        Document, Object, Stream,
        content::{Content, Operation},
        dictionary,
    };

    fn pdf(text: &str) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Tj", vec![Object::string_literal(text)]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {"Type" => "Page", "Parent" => pages_id, "Contents" => content_id,
            "Resources" => dictionary! {"Font" => dictionary! {"F1" => dictionary! {"Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica"}}}});
        doc.objects.insert(
            pages_id,
            Object::Dictionary(
                dictionary! {"Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1},
            ),
        );
        let catalog_id = doc.add_object(dictionary! {"Type" => "Catalog", "Pages" => pages_id});
        doc.trailer.set("Root", catalog_id);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn extracts_embedded_text_and_normalizes_fields_without_ocr() {
        let bytes = pdf(
            "Patient Name: Jamie Example\nSex: F\nDOB: 2/3/1980\nStreet Address: 1 Main St\nCity: Houston\nState: TX\nZip Code: 77071\nCountry: USA\nMobile: (817) 555-0123\nMBI: 1EG4-TE5-MK72\nPrimary Insurance ID Number: PRIMARY123\nSecondary Insurance ID Number: SECONDARY456\nSecondary Plan Name: Example Plan",
        );
        let parsed =
            parse_pdf_with_ocr(&bytes, |_| panic!("embedded text should bypass OCR")).unwrap();
        assert_eq!(parsed.full_name.as_deref(), Some("Jamie Example"));
        assert_eq!(parsed.gender.as_deref(), Some("Female"));
        assert_eq!(parsed.dob.as_deref(), Some("1980-02-03"));
        assert_eq!(
            parsed.address.as_deref(),
            Some("1 Main St, Houston, TX, 77071, USA")
        );
        assert_eq!(parsed.phone.as_deref(), Some("(817) 555-0123"));
        assert_eq!(parsed.medicare, None);
        assert_eq!(parsed.other_insurance, None);
    }

    #[test]
    fn uses_ocr_when_embedded_text_is_absent_and_fails_clearly_when_unavailable() {
        let bytes = pdf("");
        let parsed = parse_pdf_with_ocr(&bytes, |_| {
            Ok("Patient Name: Jamie Example\nDOB: 01/02/1980".into())
        })
        .unwrap();
        assert_eq!(parsed.full_name.as_deref(), Some("Jamie Example"));
        let error = parse_pdf_with_ocr(&bytes, |_| Err("internal command detail".into()))
            .unwrap_err()
            .to_string();
        assert!(error.contains("local OCR is unavailable or failed"));
        assert!(!error.contains("internal command detail"));
    }

    #[test]
    fn keeps_short_but_usable_embedded_text_and_does_not_run_ocr() {
        let bytes = pdf("DOB: 01/02/1980");
        let parsed =
            parse_pdf_with_ocr(&bytes, |_| panic!("usable text must not invoke OCR")).unwrap();
        assert_eq!(parsed.dob.as_deref(), Some("1980-01-02"));
    }

    #[test]
    fn ignores_insurance_sections_in_pdf_import() {
        let bytes = pdf(
            "Primary Insurance\nInsurer: Medicare TX\nPayer ID: 04412\nID #: PRIMARY123\nSecondary Insurance\nInsurer: Sample Plan\nID #: SECONDARY456",
        );
        let parsed =
            parse_pdf_with_ocr(&bytes, |_| panic!("embedded text should bypass OCR")).unwrap();
        assert_eq!(parsed.medicare, None);
        assert_eq!(parsed.other_insurance, None);
    }

    #[test]
    fn does_not_treat_accident_occurrence_label_as_patient_address() {
        let bytes = pdf("Address of Accident Occurrence\nWork site");
        let parsed =
            parse_pdf_with_ocr(&bytes, |_| panic!("embedded text should bypass OCR")).unwrap();
        assert_eq!(parsed.address, None);
    }

    #[test]
    fn reads_standalone_address_rows() {
        let address = field_value(
            &["Street Address", "14 Sample Road", "Zip", "75001"],
            &["street address"],
        );
        assert_eq!(address.as_deref(), Some("14 Sample Road"));
    }

}
