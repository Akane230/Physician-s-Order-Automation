use crate::{CoreError, CoreResult, db::Database};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{Duration, Utc};
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::sync::Mutex;
use zeroize::Zeroizing;

const MAX_FAILED_ATTEMPTS: i64 = 5;
const LOCKOUT_MINUTES: i64 = 15;
const SESSION_IDLE_MINUTES: i64 = 15;

/// Local account and in-memory session operations. Session credentials are never persisted.
pub struct AuthService {
    database: Database,
    session: Mutex<Option<Session>>,
}

struct Session {
    user: AuthenticatedUser,
    last_activity: chrono::DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatedUser {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub must_change_password: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub setup_required: bool,
    pub user: Option<AuthenticatedUser>,
}

impl AuthService {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            session: Mutex::new(None),
        }
    }

    pub fn status(&self) -> CoreResult<AuthStatus> {
        let conn = self.database.open()?;
        let setup_required: bool =
            conn.query_row("SELECT NOT EXISTS(SELECT 1 FROM users)", [], |row| {
                row.get(0)
            })?;

        let mut session = self
            .session
            .lock()
            .map_err(|_| CoreError::Initialization("session state unavailable".into()))?;
        let user = match session.as_mut() {
            Some(active)
                if Utc::now() - active.last_activity < Duration::minutes(SESSION_IDLE_MINUTES) =>
            {
                active.last_activity = Utc::now();
                Some(active.user.clone())
            }
            Some(_) => {
                *session = None;
                None
            }
            None => None,
        };
        Ok(AuthStatus {
            setup_required,
            user,
        })
    }

    /// Creates the only administrator allowed before the first account exists.
    pub fn setup_initial_admin(
        &self,
        username: &str,
        display_name: &str,
        password: String,
    ) -> CoreResult<AuthenticatedUser> {
        let username = normalize_username(username)?;
        let display_name = validate_nonempty(display_name, "display name", 120)?;
        let password = validate_password(password)?;
        let password_hash = hash_password(password.as_bytes())?;
        let now = Utc::now().to_rfc3339();
        let id = random_id()?;

        let mut conn = self.database.open()?;
        let tx = conn.transaction()?;
        let user_count: i64 = tx.query_row("SELECT count(*) FROM users", [], |row| row.get(0))?;
        if user_count != 0 {
            return Err(CoreError::SetupComplete);
        }
        tx.execute(
            "INSERT INTO users (id, username, display_name, password_hash, role, created_at, updated_at)\
             VALUES (?1, ?2, ?3, ?4, 'admin', ?5, ?5)",
            (&id, &username, &display_name, &password_hash, &now),
        )?;
        tx.commit()?;

        let user = AuthenticatedUser {
            id,
            username,
            display_name,
            role: "admin".into(),
            must_change_password: false,
        };
        self.establish_session(user.clone())?;
        Ok(user)
    }

    pub fn login(&self, username: &str, password: String) -> CoreResult<AuthenticatedUser> {
        let username = normalize_username(username)?;
        let password = Zeroizing::new(password);
        let conn = self.database.open()?;
        let account = conn.query_row(
            "SELECT id, username, display_name, password_hash, role, active, failed_attempts, locked_until, must_change_password\
             FROM users WHERE username = ?1 COLLATE NOCASE",
            [&username],
            |row| Ok((
                row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?,
                row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, bool>(5)?,
                row.get::<_, i64>(6)?, row.get::<_, Option<String>>(7)?, row.get::<_, bool>(8)?,
            )),
        ).optional()?;

        let Some((
            id,
            username,
            display_name,
            stored_hash,
            role,
            active,
            attempts,
            locked_until,
            must_change_password,
        )) = account
        else {
            return Err(CoreError::InvalidCredentials);
        };
        if !active {
            return Err(CoreError::InvalidCredentials);
        }

        let now = Utc::now();
        if locked_until.as_deref().is_some_and(|until| {
            chrono::DateTime::parse_from_rfc3339(until)
                .map(|timestamp| timestamp.with_timezone(&Utc) > now)
                .unwrap_or(true)
        }) {
            return Err(CoreError::AccountLocked);
        }
        if verify_password(password.as_bytes(), &stored_hash).is_err() {
            let new_attempts = attempts + 1;
            if new_attempts >= MAX_FAILED_ATTEMPTS {
                let until = (now + Duration::minutes(LOCKOUT_MINUTES)).to_rfc3339();
                conn.execute(
                    "UPDATE users SET failed_attempts = ?1, locked_until = ?2, updated_at = ?3 WHERE id = ?4",
                    (&new_attempts, &until, &now.to_rfc3339(), &id),
                )?;
                return Err(CoreError::AccountLocked);
            }
            conn.execute(
                "UPDATE users SET failed_attempts = ?1, updated_at = ?2 WHERE id = ?3",
                (&new_attempts, &now.to_rfc3339(), &id),
            )?;
            return Err(CoreError::InvalidCredentials);
        }

        conn.execute(
            "UPDATE users SET failed_attempts = 0, locked_until = NULL, updated_at = ?1 WHERE id = ?2",
            (&now.to_rfc3339(), &id),
        )?;
        let user = AuthenticatedUser {
            id,
            username,
            display_name,
            role,
            must_change_password,
        };
        self.establish_session(user.clone())?;
        Ok(user)
    }

    pub fn logout(&self) -> CoreResult<()> {
        *self
            .session
            .lock()
            .map_err(|_| CoreError::Initialization("session state unavailable".into()))? = None;
        Ok(())
    }

    fn establish_session(&self, user: AuthenticatedUser) -> CoreResult<()> {
        *self
            .session
            .lock()
            .map_err(|_| CoreError::Initialization("session state unavailable".into()))? =
            Some(Session {
                user,
                last_activity: Utc::now(),
            });
        Ok(())
    }
}

fn normalize_username(value: &str) -> CoreResult<String> {
    let username = validate_nonempty(value, "username", 64)?;
    if username.chars().any(char::is_control) {
        return Err(CoreError::Validation(
            "username contains unsupported characters".into(),
        ));
    }
    Ok(username)
}

fn validate_nonempty(value: &str, field: &str, max_chars: usize) -> CoreResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max_chars {
        return Err(CoreError::Validation(format!(
            "{field} is required and must be at most {max_chars} characters"
        )));
    }
    Ok(value.to_string())
}

fn validate_password(password: String) -> CoreResult<Zeroizing<String>> {
    if password.chars().count() < 12 || password.chars().count() > 256 {
        return Err(CoreError::Validation(
            "password must be between 12 and 256 characters".into(),
        ));
    }
    Ok(Zeroizing::new(password))
}

fn hash_password(password: &[u8]) -> CoreResult<String> {
    // password-hash generates a cryptographically random per-password salt.
    Argon2::default()
        .hash_password(password)
        .map(|hash| hash.to_string())
        .map_err(|_| CoreError::Initialization("password hashing failed".into()))
}

fn verify_password(password: &[u8], stored_hash: &str) -> CoreResult<()> {
    let parsed = PasswordHash::new(stored_hash)
        .map_err(|_| CoreError::Initialization("stored password hash is invalid".into()))?;
    Argon2::default()
        .verify_password(password, &parsed)
        .map_err(|_| CoreError::InvalidCredentials)
}

fn random_id() -> CoreResult<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| CoreError::Random)?;
    let id = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    bytes.zeroize();
    Ok(id)
}

use zeroize::Zeroize;
