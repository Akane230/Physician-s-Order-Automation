use orders_core::parsing::pdf_demographics::parse_pdf_with_ocr;
use std::process::{Command, Stdio};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use zeroize::Zeroizing;

#[tauri::command]
pub fn import_demographics_pdf(
    app: AppHandle,
) -> Result<Option<orders_core::parsing::demographics::ParsedDemographics>, String> {
    let Some(file) = app
        .dialog()
        .file()
        .add_filter("PDF document", &["pdf"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = file
        .into_path()
        .map_err(|_| "Could not access the selected PDF".to_string())?;
    import_pdf_path(&path).map(Some)
}

#[tauri::command]
pub fn import_demographics_pdf_path(
    path: String,
) -> Result<orders_core::parsing::demographics::ParsedDemographics, String> {
    let path = std::path::PathBuf::from(path);
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_none_or(|ext| !ext.eq_ignore_ascii_case("pdf"))
    {
        return Err("Choose a PDF document".into());
    }
    import_pdf_path(&path)
}

fn import_pdf_path(
    path: &std::path::Path,
) -> Result<orders_core::parsing::demographics::ParsedDemographics, String> {
    let metadata =
        std::fs::metadata(path).map_err(|_| "Could not read the selected PDF".to_string())?;
    if metadata.len() > 20 * 1024 * 1024 {
        return Err("PDF exceeds the 20 MB import limit".into());
    }
    let bytes = Zeroizing::new(
        std::fs::read(path).map_err(|_| "Could not read the selected PDF".to_string())?,
    );
    parse_pdf_with_ocr(&bytes, |page| ocr_page(path, page)).map_err(|error| error.to_string())
}

fn ocr_page(path: &std::path::Path, page: u32) -> Result<String, String> {
    // Executables are resolved by the OS from PATH. File paths are passed as distinct arguments,
    // never interpolated into a shell command; image data streams through memory without temp files.
    let mut renderer = Command::new("pdftoppm")
        .args([
            "-f",
            &page.to_string(),
            "-l",
            &page.to_string(),
            "-singlefile",
            "-png",
            "-scale-to",
            "2000",
        ])
        .arg(path)
        .arg("-")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Poppler pdftoppm was not found".to_string())?;
    let png = renderer.stdout.take().ok_or("PDF rendering failed")?;
    let ocr = Command::new("tesseract")
        .arg("stdin")
        .arg("stdout")
        .args(["-l", "eng"])
        .stdin(Stdio::from(png))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Tesseract OCR was not found".to_string())?;
    let output = ocr
        .wait_with_output()
        .map_err(|_| "Tesseract OCR failed".to_string())?;
    let render_status = renderer
        .wait()
        .map_err(|_| "PDF rendering failed".to_string())?;
    if !output.status.success() || !render_status.success() {
        return Err("Local OCR failed".into());
    }
    String::from_utf8(output.stdout).map_err(|_| "Local OCR returned unreadable text".into())
}
