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

pub(super) fn render_pdf(order: PdfOrder<'_>) -> CoreResult<Vec<u8>> {
    let patient_name = order.patient_name.trim();
    let diagnoses = order
        .diagnoses
        .iter()
        .map(diagnosis_text)
        .filter(|line| !line.is_empty())
        .flat_map(|line| wrap_paragraph(&line, 380.0, 9.0))
        .collect::<Vec<_>>();
    let (diagnosis_first, diagnosis_overflow) = diagnoses.split_at(diagnoses.len().min(2));

    let mut order_lines = Vec::new();
    if !order.order_type_name.trim().is_empty() {
        order_lines.push(format!("Order type: {}", order.order_type_name.trim()));
    }
    order_lines.extend(
        diagnosis_overflow
            .iter()
            .map(|line| format!("Diagnosis: {line}")),
    );
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
    let order_pages = chunks(order_lines, 15);

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
        draw_patient_fields(&mut operations, &order, patient_name);
        for (line_index, line) in diagnosis_first.iter().enumerate() {
            draw_text(
                &mut operations,
                line,
                152.0,
                553.0 - line_index as f32 * 12.0,
                9.0,
                false,
            )?;
        }

        if page_index > 0 {
            draw_text(&mut operations, "(CONTINUED)", 177.0, 474.0, 9.0, true)?;
        }
        if let Some(lines) = order_pages.get(page_index) {
            for (line_index, line) in lines.iter().enumerate() {
                draw_text(
                    &mut operations,
                    line,
                    93.8,
                    449.0 - line_index as f32 * 13.2,
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
            let printed_name = format!("{}  NPI: {}", order.physician_name, order.physician_npi);
            draw_text(&mut operations, &printed_name, 57.8, 103.0, 10.0, false)?;
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

fn draw_patient_fields(operations: &mut Vec<Operation>, order: &PdfOrder<'_>, patient_name: &str) {
    let rows = [
        (patient_name, 669.0),
        (order.address, 653.8),
        (order.dob, 638.7),
        (order.phone, 623.5),
        (order.medicare, 608.4),
        (order.other_insurance, 593.2),
        (&order.order_date, 578.1),
    ];
    for (value, y) in rows {
        draw_fit_text(operations, value, 152.3, y, 385.0, 10.2, false);
    }
    let physician = format!("{}  NPI: {}", order.physician_name, order.physician_npi);
    draw_fit_text(operations, &physician, 152.3, 562.9, 385.0, 10.2, true);
}

fn add_font(document: &mut Document, base_font: &str) -> (u32, u16) {
    document.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => base_font,
        "Encoding" => "WinAnsiEncoding",
    })
}

fn draw_fit_text(
    operations: &mut Vec<Operation>,
    text: &str,
    x: f32,
    y: f32,
    max_width: f32,
    max_size: f32,
    bold: bool,
) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    let width_at_size = estimate_width(text, max_size);
    let size = if width_at_size > max_width {
        (max_size * max_width / width_at_size).max(7.0)
    } else {
        max_size
    };
    let _ = draw_text(operations, text, x, y, size, bold);
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
