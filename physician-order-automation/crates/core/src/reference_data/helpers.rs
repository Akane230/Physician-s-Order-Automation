use crate::{CoreError, CoreResult};
use zeroize::Zeroize;

pub(super) fn required_text(value: String, field: &str, max_chars: usize) -> CoreResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max_chars {
        return Err(CoreError::Validation(format!(
            "{field} is required and must be at most {max_chars} characters"
        )));
    }
    Ok(value.to_string())
}

pub(super) fn optional_text(value: String, field: &str, max_chars: usize) -> CoreResult<String> {
    if value.chars().count() > max_chars {
        return Err(CoreError::Validation(format!(
            "{field} must be at most {max_chars} characters"
        )));
    }
    Ok(value.trim().to_string())
}

pub(super) fn random_id() -> CoreResult<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| CoreError::Random)?;
    let id = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    bytes.zeroize();
    Ok(id)
}
