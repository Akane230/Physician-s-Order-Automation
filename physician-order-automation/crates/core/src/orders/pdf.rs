use super::PdfOrder;
use crate::{CoreError, CoreResult, parsing::diagnoses::Diagnosis};
use lopdf::{
    Document, Object, StringFormat,
    content::{Content, Operation},
    dictionary,
};

const TEMPLATE_BACKGROUND: &[u8] = include_bytes!("../../resources/po-format-background.jpg");
const PAGE_WIDTH: f32 = 595.3;
const PAGE_HEIGHT: f32 = 841.9;
const IMAGE_WIDTH: i64 = 2481;
const IMAGE_HEIGHT: i64 = 3508;
const FONT: &[u8] = b"FPOA";
const BOLD: &[u8] = b"FPOAB";
const TABLE_LEFT: f32 = 52.0;
const LABEL_DIVIDER: f32 = 146.0;
const TABLE_RIGHT: f32 = 542.0;
const TABLE_TOP: f32 = 682.0;
const COMMENTS_TOP: f32 = 223.0;
const MD_LINE_HEIGHT: f32 = 13.2;

struct FieldRow {
    label: String,
    label_lines: Vec<String>,
    value_lines: Vec<String>,
    height: f32,
    font_size: f32,
}

pub(super) fn render_pdf(order: PdfOrder<'_>) -> CoreResult<Vec<u8>> {
    let patient_name = order.patient_name.trim();
    let diagnoses = order
        .diagnoses
        .iter()
        .map(diagnosis_text)
        .filter(|line| !line.is_empty())
        .flat_map(|line| wrap_paragraph(&line, 380.0, 9.0))
        .collect::<Vec<_>>();
    let diagnosis_first = diagnoses.iter().take(3).cloned().collect::<Vec<_>>();
    let diagnosis_overflow = diagnoses
        .iter()
        .skip(3)
        .map(|line| format!("Diagnosis: {line}"))
        .collect::<Vec<_>>();
    let rows = patient_rows(&order, patient_name);
    let diagnosis_height = 40.0_f32.max(diagnosis_first.len() as f32 * 12.0 + 4.0);
    let demographics_bottom =
        TABLE_TOP - rows.iter().map(|row| row.height).sum::<f32>() - diagnosis_height;
    let body_start = demographics_bottom - 43.0;
    let body_capacity = ((body_start - 236.0) / MD_LINE_HEIGHT).floor() as usize;
    if body_capacity < 3 {
        return Err(CoreError::Validation(
            "Patient address and demographic details are too long for the order form. Shorten the values before generating the PDF.".into(),
        ));
    }

    let mut order_lines = Vec::new();
    if !order.order_type_name.trim().is_empty() {
        order_lines.push(format!("Order type: {}", order.order_type_name.trim()));
    }
    order_lines.extend(diagnosis_overflow);
    if !order.md_order_body.trim().is_empty() {
        order_lines.extend(
            order
                .md_order_body
                .lines()
                .flat_map(|line| wrap_paragraph(line, 430.0, 10.0)),
        );
    }
    if !order.order_details.trim().is_empty() {
        order_lines.extend(
            order
                .order_details
                .lines()
                .flat_map(|line| wrap_paragraph(line, 430.0, 10.0)),
        );
    }
    if order_lines.is_empty() {
        order_lines.push("".to_string());
    }
    let order_pages = chunks(order_lines, body_capacity);

    let comment_lines = if order.comments.trim().is_empty() {
        Vec::new()
    } else {
        order
            .comments
            .lines()
            .flat_map(|line| wrap_paragraph(line, 470.0, 9.5))
            .collect()
    };
    let comment_pages = if comment_lines.is_empty() {
        vec![Vec::new()]
    } else {
        chunks(comment_lines, 4)
    };
    let page_count = order_pages.len().max(comment_pages.len()).max(1);
    if page_count > 50 {
        return Err(CoreError::Validation(
            "Order content exceeds the 50-page PDF limit. Shorten the order details or comments before generating.".into(),
        ));
    }
    let printed_name = format!("{}  NPI: {}", order.physician_name, order.physician_npi);
    let signature_lines = wrap_paragraph(&printed_name, 460.0, 9.5);
    if signature_lines.len() > 3 {
        return Err(CoreError::Validation(
            "The selected physician name and NPI do not fit in the signature block.".into(),
        ));
    }

    let mut document = Document::with_version("1.4");
    let pages_id = document.new_object_id();
    let font_regular = add_font(&mut document, "Helvetica");
    let font_bold = add_font(&mut document, "Helvetica-Bold");
    let image_id = document.add_object(lopdf::Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => IMAGE_WIDTH,
            "Height" => IMAGE_HEIGHT,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
            "Filter" => "DCTDecode",
        },
        TEMPLATE_BACKGROUND.to_vec(),
    ));
    let mut page_ids = Vec::with_capacity(page_count);
    for page_index in 0..page_count {
        let mut operations = vec![
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![
                    Object::Real(PAGE_WIDTH),
                    0.into(),
                    0.into(),
                    Object::Real(PAGE_HEIGHT),
                    0.into(),
                    0.into(),
                ],
            ),
            Operation::new("Do", vec![Object::Name(b"Bg".to_vec())]),
            Operation::new("Q", vec![]),
        ];
        white_rect(
            &mut operations,
            TABLE_LEFT,
            COMMENTS_TOP + 0.8,
            TABLE_RIGHT - TABLE_LEFT,
            TABLE_TOP - COMMENTS_TOP - 0.8,
        );
        let diagnosis_row = FieldRow {
            label: "Diagnoses:".into(),
            label_lines: vec!["Diagnoses:".into()],
            value_lines: diagnosis_first.clone(),
            height: diagnosis_height,
            font_size: 9.0,
        };
        let mut page_rows = rows.iter().map(clone_field_row).collect::<Vec<_>>();
        page_rows.push(diagnosis_row);
        draw_patient_table(&mut operations, &page_rows)?;
        let md_top = demographics_bottom;
        draw_text(&mut operations, "MD ORDER", 93.8, md_top - 22.0, 14.0, true)?;

        if page_index > 0 {
            draw_text(
                &mut operations,
                "(CONTINUED)",
                177.0,
                md_top - 22.0,
                9.0,
                true,
            )?;
        }
        if let Some(lines) = order_pages.get(page_index) {
            for (line_index, line) in lines.iter().enumerate() {
                draw_text(
                    &mut operations,
                    line,
                    93.8,
                    body_start - line_index as f32 * MD_LINE_HEIGHT,
                    10.0,
                    false,
                )?;
            }
        }

        if let Some(lines) = comment_pages.get(page_index) {
            for (line_index, line) in lines.iter().enumerate() {
                draw_text(
                    &mut operations,
                    line,
                    58.0,
                    204.0 - line_index as f32 * 12.0,
                    9.5,
                    false,
                )?;
            }
        }
        if page_index + 1 == page_count {
            for (index, line) in signature_lines.iter().enumerate() {
                draw_text(
                    &mut operations,
                    line,
                    57.8,
                    118.0 - index as f32 * 12.0,
                    9.5,
                    false,
                )?;
            }
        }

        let content = Content { operations }
            .encode()
            .map_err(|error| CoreError::Pdf(error.to_string()))?;
        let content_id = document.add_object(lopdf::Stream::new(lopdf::Dictionary::new(), content));
        let page_id = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), PAGE_WIDTH.into(), PAGE_HEIGHT.into()],
            "Resources" => dictionary! {
                "Font" => dictionary! { "FPOA" => font_regular, "FPOAB" => font_bold },
                "XObject" => dictionary! { "Bg" => image_id },
            },
            "Contents" => content_id,
        });
        page_ids.push(page_id);
    }
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => page_ids.iter().copied().map(Object::Reference).collect::<Vec<_>>(),
            "Count" => page_ids.len() as i64,
        }),
    );
    let catalog_id = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    document.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .map_err(|error| CoreError::Pdf(error.to_string()))?;
    Ok(bytes)
}

fn patient_rows(order: &PdfOrder<'_>, patient_name: &str) -> Vec<FieldRow> {
    let mut name_lines = wrap_paragraph(patient_name, 382.0, 9.5);
    let mut address_lines = wrap_paragraph(order.address, 382.0, 9.5);
    let mut physician_lines = wrap_paragraph(
        &format!("{}  NPI: {}", order.physician_name, order.physician_npi),
        382.0,
        9.5,
    );
    let mut rows = vec![
        make_field_row("Patient Name:", &mut name_lines, 9.5),
        make_field_row("Address:", &mut address_lines, 9.5),
        make_field_row("DOB:", &mut wrap_paragraph(order.dob, 382.0, 9.5), 9.5),
        make_field_row(
            "Phone No.:",
            &mut wrap_paragraph(order.phone, 382.0, 9.5),
            9.5,
        ),
        make_field_row(
            "Primary Insurance ID:",
            &mut wrap_paragraph(order.medicare, 382.0, 9.5),
            9.5,
        ),
        make_field_row(
            "Secondary Insurance ID:",
            &mut wrap_paragraph(order.other_insurance, 382.0, 9.5),
            9.5,
        ),
        make_field_row(
            "Order Date:",
            &mut wrap_paragraph(&order.order_date, 382.0, 9.5),
            9.5,
        ),
        make_field_row("Physician:", &mut physician_lines, 9.5),
    ];
    // Do not leave empty values represented as artificial wrapped rows.
    for row in &mut rows {
        if row.value_lines.len() == 1 && row.value_lines[0].is_empty() {
            row.value_lines.clear();
        }
    }
    rows
}

fn make_field_row(label: &str, value_lines: &mut Vec<String>, font_size: f32) -> FieldRow {
    let label_lines = wrap_paragraph(label, LABEL_DIVIDER - TABLE_LEFT - 12.0, 8.5);
    let height = 15.0_f32
        .max(value_lines.len() as f32 * 12.0)
        .max(label_lines.len() as f32 * 10.5);
    FieldRow {
        label: label.to_string(),
        label_lines,
        value_lines: value_lines.clone(),
        height,
        font_size,
    }
}

fn clone_field_row(row: &FieldRow) -> FieldRow {
    FieldRow {
        label: row.label.clone(),
        label_lines: row.label_lines.clone(),
        value_lines: row.value_lines.clone(),
        height: row.height,
        font_size: row.font_size,
    }
}

fn draw_patient_table(operations: &mut Vec<Operation>, rows: &[FieldRow]) -> CoreResult<()> {
    let diagnosis = rows.last().expect("diagnosis row is always present");
    let table_bottom = TABLE_TOP - rows.iter().map(|row| row.height).sum::<f32>();
    draw_line(operations, TABLE_LEFT, TABLE_TOP, TABLE_RIGHT, TABLE_TOP);
    draw_line(
        operations,
        TABLE_LEFT,
        table_bottom,
        TABLE_RIGHT,
        table_bottom,
    );
    draw_line(operations, TABLE_LEFT, TABLE_TOP, TABLE_LEFT, table_bottom);
    draw_line(
        operations,
        TABLE_RIGHT,
        TABLE_TOP,
        TABLE_RIGHT,
        table_bottom,
    );
    draw_line(
        operations,
        TABLE_LEFT,
        table_bottom,
        TABLE_LEFT,
        COMMENTS_TOP,
    );
    draw_line(
        operations,
        TABLE_RIGHT,
        table_bottom,
        TABLE_RIGHT,
        COMMENTS_TOP,
    );
    draw_line(
        operations,
        TABLE_LEFT,
        COMMENTS_TOP,
        TABLE_RIGHT,
        COMMENTS_TOP,
    );
    let diagnosis_top = table_bottom + diagnosis.height;
    draw_line(
        operations,
        LABEL_DIVIDER,
        TABLE_TOP,
        LABEL_DIVIDER,
        diagnosis_top,
    );

    let mut top = TABLE_TOP;
    for row in rows {
        let baseline = top - 10.5;
        for (index, line) in row.label_lines.iter().enumerate() {
            draw_text(
                operations,
                line,
                TABLE_LEFT + 5.8,
                baseline - index as f32 * 10.5,
                8.5,
                true,
            )?;
        }
        for (index, line) in row.value_lines.iter().enumerate() {
            draw_text(
                operations,
                line,
                LABEL_DIVIDER + 6.3,
                baseline - index as f32 * 12.0,
                row.font_size,
                false,
            )?;
        }
        top -= row.height;
        draw_line(operations, TABLE_LEFT, top, TABLE_RIGHT, top);
    }
    Ok(())
}

fn draw_line(operations: &mut Vec<Operation>, x1: f32, y1: f32, x2: f32, y2: f32) {
    operations.extend([
        Operation::new("w", vec![Object::Real(0.55)]),
        Operation::new("m", vec![Object::Real(x1), Object::Real(y1)]),
        Operation::new("l", vec![Object::Real(x2), Object::Real(y2)]),
        Operation::new("S", vec![]),
    ]);
}

fn add_font(document: &mut Document, base_font: &str) -> (u32, u16) {
    document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => base_font,
        "Encoding" => "WinAnsiEncoding",
    })
}

fn draw_text(
    operations: &mut Vec<Operation>,
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    bold: bool,
) -> CoreResult<()> {
    let bytes = win_ansi(text)?;
    operations.extend([
        Operation::new("BT", vec![]),
        Operation::new(
            "Tf",
            vec![
                Object::Name(if bold { BOLD.to_vec() } else { FONT.to_vec() }),
                Object::Real(size),
            ],
        ),
        Operation::new("Td", vec![Object::Real(x), Object::Real(y)]),
        Operation::new("Tj", vec![Object::String(bytes, StringFormat::Literal)]),
        Operation::new("ET", vec![]),
    ]);
    Ok(())
}

fn white_rect(operations: &mut Vec<Operation>, x: f32, y: f32, width: f32, height: f32) {
    operations.extend([
        Operation::new("q", vec![]),
        Operation::new(
            "rg",
            vec![Object::Real(1.0), Object::Real(1.0), Object::Real(1.0)],
        ),
        Operation::new(
            "re",
            vec![
                Object::Real(x),
                Object::Real(y),
                Object::Real(width),
                Object::Real(height),
            ],
        ),
        Operation::new("f", vec![]),
        Operation::new("Q", vec![]),
    ]);
}

fn chunks<T>(items: Vec<T>, size: usize) -> Vec<Vec<T>> {
    let mut result = Vec::new();
    let mut items = items.into_iter();
    loop {
        let chunk = items.by_ref().take(size).collect::<Vec<_>>();
        if chunk.is_empty() {
            break;
        }
        result.push(chunk);
    }
    result
}

fn diagnosis_text(diagnosis: &Diagnosis) -> String {
    match (&diagnosis.code, diagnosis.description.trim().is_empty()) {
        (Some(code), false) => format!("{code}  {}", diagnosis.description.trim()),
        (Some(code), true) => code.clone(),
        (None, _) => diagnosis.description.trim().to_string(),
    }
}

fn wrap_paragraph(text: &str, max_width: f32, size: f32) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if estimate_width(&candidate, size) <= max_width {
            current = candidate;
        } else {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            if estimate_width(word, size) <= max_width {
                current.push_str(word);
            } else {
                for character in word.chars() {
                    let candidate = format!("{current}{character}");
                    if !current.is_empty() && estimate_width(&candidate, size) > max_width {
                        lines.push(std::mem::take(&mut current));
                    }
                    current.push(character);
                }
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn estimate_width(text: &str, size: f32) -> f32 {
    text.chars()
        .map(|character| {
            let units = match character {
                ' ' => 0.28,
                'i' | 'l' | 'I' | '.' | ',' | ':' | ';' | '!' | '|' | '\'' => 0.27,
                'm' | 'w' | 'M' | 'W' | '@' | '%' => 0.82,
                'A'..='Z' => 0.62,
                '0'..='9' => 0.56,
                _ => 0.50,
            };
            size * units
        })
        .sum()
}

fn win_ansi(value: &str) -> CoreResult<Vec<u8>> {
    value
        .chars()
        .map(|character| {
            let code = character as u32;
            let byte = if code <= 0x7f || (0xa0..=0xff).contains(&code) {
                Some(code as u8)
            } else {
                match character {
                    '€' => Some(0x80),
                    '‚' => Some(0x82),
                    'ƒ' => Some(0x83),
                    '„' => Some(0x84),
                    '…' => Some(0x85),
                    '†' => Some(0x86),
                    '‡' => Some(0x87),
                    'ˆ' => Some(0x88),
                    '‰' => Some(0x89),
                    'Š' => Some(0x8a),
                    '‹' => Some(0x8b),
                    'Œ' => Some(0x8c),
                    'Ž' => Some(0x8e),
                    '‘' => Some(0x91),
                    '’' => Some(0x92),
                    '“' => Some(0x93),
                    '”' => Some(0x94),
                    '•' => Some(0x95),
                    '–' => Some(0x96),
                    '—' => Some(0x97),
                    '˜' => Some(0x98),
                    '™' => Some(0x99),
                    'š' => Some(0x9a),
                    '›' => Some(0x9b),
                    'œ' => Some(0x9c),
                    'ž' => Some(0x9e),
                    'Ÿ' => Some(0x9f),
                    '\t' => Some(b' '),
                    _ if character.is_control() => Some(b' '),
                    _ => None,
                }
            };
            byte.ok_or_else(|| {
                CoreError::Validation(
                    "PDF text contains a character unsupported by the built-in font".into(),
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_order<'a>(
        address: &'a str,
        diagnoses: &'a [Diagnosis],
        body: &'a str,
        comments: &'a str,
    ) -> PdfOrder<'a> {
        PdfOrder {
            patient_name: "Taylor Test".into(),
            address,
            dob: "03/26/1954",
            phone: "555-010-1234",
            medicare: "PRIMARY123",
            other_insurance: "SECONDARY456",
            order_date: "11/01/2026".into(),
            physician_name: "Dr. Example Physician",
            physician_npi: "1234567890",
            diagnoses,
            order_type_name: "Sample Order",
            md_order_body: body,
            order_details: "",
            comments,
        }
    }

    #[test]
    fn wraps_long_order_content_onto_additional_pages_without_losing_text() {
        let body = (1..=80)
            .map(|index| format!("Order instruction line {index}: provide the requested service."))
            .collect::<Vec<_>>()
            .join("\n");
        let pdf = render_pdf(sample_order(
            "14 Sample Road, Dallas, TX, 75001, USA",
            &[],
            &body,
            "Coordinate with the caregiver before the visit.",
        ))
        .unwrap();
        let document = Document::load_mem(&pdf).unwrap();
        let pages = document.get_pages();
        assert!(pages.len() > 1);
        let page_numbers = pages.keys().copied().collect::<Vec<_>>();
        let text = document.extract_text(&page_numbers).unwrap();
        assert!(text.contains("Order instruction line 1:"));
        assert!(text.contains("Order instruction line 80:"));
        assert!(text.contains("14 Sample Road, Dallas, TX, 75001, USA"));
        assert!(text.contains("14 Sample Road"));
        assert!(text.contains("Dallas, TX"));
        assert!(text.contains("75001"));
        assert!(text.contains("USA"));
    }

    #[test]
    fn rejects_demographic_blocks_that_leave_no_safe_order_body_area() {
        let address = "Long Street Name ".repeat(500);
        let error = render_pdf(sample_order(&address, &[], "Short order", ""))
            .expect_err("oversized address should be rejected rather than clipped");
        assert!(error.to_string().contains("too long for the order form"));
    }
}
