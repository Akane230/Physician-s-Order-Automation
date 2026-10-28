use thiserror::Error;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("database error")]
    Database(#[from] rusqlite::Error),
    #[error("secure key storage error: {0}")]
    KeyStore(String),
    #[error("secure random generation failed")]
    Random,
    #[error("database schema version {found} is newer than supported version {supported}")]
    UnsupportedSchema { found: i64, supported: i64 },
    #[error("database initialization failed: {0}")]
    Initialization(String),
    #[error("invalid input: {0}")]
    Validation(String),
    #[error("initial administrator has already been created")]
    SetupComplete,
    #[error("username or password is incorrect")]
    InvalidCredentials,
    #[error("account is temporarily locked")]
    AccountLocked,
    #[error("no active user session")]
    SessionRequired,
    #[error("administrator role required")]
    PermissionDenied,
    #[error("PDF generation failed: {0}")]
    Pdf(String),
}
