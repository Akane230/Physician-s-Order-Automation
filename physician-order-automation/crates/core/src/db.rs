use crate::{CoreError, CoreResult};
use keyring::Entry;
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use zeroize::Zeroizing;

const KEY_SERVICE: &str = "com.homekure.physician-order-automation";
const KEY_ACCOUNT: &str = "sqlcipher-database-key-v1";
const SCHEMA_VERSION: i64 = 4;

/// Owns the encrypted local database path and opens configured connections.
/// The database key is created once and stored only in the OS credential store.
#[derive(Clone, Debug)]
pub struct Database {
    path: PathBuf,
    #[cfg(test)]
    test_key: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseStatus {
    pub schema_version: i64,
    pub cipher_version: String,
}

impl Database {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            #[cfg(test)]
            test_key: None,
        }
    }

    #[cfg(test)]
    pub fn new_with_test_key(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            test_key: Some("test-only-sqlcipher-key".into()),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Opens the SQLCipher database, applies migrations and enables WAL mode.
    pub fn open(&self) -> CoreResult<Connection> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| CoreError::Initialization(e.to_string()))?;
        }

        #[cfg(test)]
        let key = match &self.test_key {
            Some(value) => Zeroizing::new(value.clone()),
            None => database_key()?,
        };
        #[cfg(not(test))]
        let key = database_key()?;
        let mut conn = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "key", key.as_str())?;

        // Force key validation before changing connection settings. This also
        // fails closed when an existing database is opened with the wrong key.
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&mut conn)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "secure_delete", "ON")?;
        Ok(conn)
    }

    /// Returns non-sensitive diagnostics proving that migrations and SQLCipher are active.
    pub fn status(&self) -> CoreResult<DatabaseStatus> {
        let conn = self.open()?;
        let schema_version: i64 = conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )?;
        let cipher_version: String =
            conn.query_row("PRAGMA cipher_version", [], |row| row.get(0))?;
        Ok(DatabaseStatus {
            schema_version,
            cipher_version,
        })
    }
}

fn database_key() -> CoreResult<Zeroizing<String>> {
    let entry =
        Entry::new(KEY_SERVICE, KEY_ACCOUNT).map_err(|e| CoreError::KeyStore(e.to_string()))?;

    match entry.get_secret() {
        Ok(bytes) => {
            let key = String::from_utf8(bytes)
                .map_err(|_| CoreError::KeyStore("stored database key is invalid".into()))?;
            Ok(Zeroizing::new(key))
        }
        Err(keyring::Error::NoEntry) => {
            let mut bytes = [0u8; 32];
            getrandom::fill(&mut bytes).map_err(|_| CoreError::Random)?;
            let key = Zeroizing::new(hex_encode(&bytes));
            bytes.zeroize();
            entry
                .set_secret(key.as_bytes())
                .map_err(|e| CoreError::KeyStore(e.to_string()))?;
            Ok(key)
        }
        Err(e) => Err(CoreError::KeyStore(e.to_string())),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn migrate(conn: &mut Connection) -> CoreResult<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if current > SCHEMA_VERSION {
        return Err(CoreError::UnsupportedSchema {
            found: current,
            supported: SCHEMA_VERSION,
        });
    }
    if current == SCHEMA_VERSION {
        return Ok(());
    }

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if current < 1 {
        tx.execute_batch(MIGRATION_1)?;
        tx.pragma_update(None, "user_version", 1)?;
    }
    if current < 2 {
        tx.execute_batch(MIGRATION_2)?;
        tx.pragma_update(None, "user_version", 2)?;
    }
    if current < 3 {
        tx.execute_batch(MIGRATION_3)?;
        tx.pragma_update(None, "user_version", 3)?;
    }
    if current < 4 {
        tx.execute_batch(MIGRATION_4)?;
        tx.pragma_update(None, "user_version", 4)?;
    }
    tx.commit()?;
    Ok(())
}

const MIGRATION_1: &str = r#"
CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    display_name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('admin', 'staff')),
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    failed_attempts INTEGER NOT NULL DEFAULT 0 CHECK (failed_attempts >= 0),
    locked_until TEXT,
    must_change_password INTEGER NOT NULL DEFAULT 0 CHECK (must_change_password IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE patients (
    id TEXT PRIMARY KEY NOT NULL,
    last_name TEXT,
    first_name TEXT,
    full_name TEXT NOT NULL,
    gender TEXT,
    address TEXT,
    dob TEXT,
    phone TEXT,
    medicare TEXT,
    other_insurance TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    created_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    updated_by TEXT REFERENCES users(id) ON DELETE SET NULL
);
CREATE UNIQUE INDEX patients_medicare_unique
    ON patients(medicare) WHERE medicare IS NOT NULL AND trim(medicare) <> '';
CREATE INDEX patients_name_dob ON patients(last_name, first_name, dob);

CREATE TABLE physicians (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    npi TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE order_types (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    md_order_body TEXT NOT NULL DEFAULT '',
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE order_type_icd (
    order_type_id TEXT NOT NULL REFERENCES order_types(id) ON DELETE CASCADE,
    icd_code TEXT NOT NULL,
    rank INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (order_type_id, icd_code)
);

CREATE TABLE icd_codes (
    code TEXT PRIMARY KEY NOT NULL,
    description TEXT NOT NULL,
    fiscal_year INTEGER NOT NULL
);
CREATE VIRTUAL TABLE icd_fts USING fts5(code UNINDEXED, description, content='icd_codes', content_rowid='rowid');
CREATE TRIGGER icd_codes_ai AFTER INSERT ON icd_codes BEGIN
    INSERT INTO icd_fts(rowid, code, description) VALUES (new.rowid, new.code, new.description);
END;
CREATE TRIGGER icd_codes_ad AFTER DELETE ON icd_codes BEGIN
    INSERT INTO icd_fts(icd_fts, rowid, code, description)
    VALUES ('delete', old.rowid, old.code, old.description);
END;
CREATE TRIGGER icd_codes_au AFTER UPDATE ON icd_codes BEGIN
    INSERT INTO icd_fts(icd_fts, rowid, code, description)
    VALUES ('delete', old.rowid, old.code, old.description);
    INSERT INTO icd_fts(rowid, code, description) VALUES (new.rowid, new.code, new.description);
END;

CREATE TABLE settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE audit_log (
    id TEXT PRIMARY KEY NOT NULL,
    ts TEXT NOT NULL,
    user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT,
    detail TEXT NOT NULL DEFAULT '{}',
    prev_hash TEXT NOT NULL,
    hash TEXT NOT NULL
);
CREATE INDEX audit_log_ts ON audit_log(ts);

CREATE TABLE schema_version (
    version INTEGER PRIMARY KEY NOT NULL,
    applied_at TEXT NOT NULL
);
INSERT INTO schema_version(version, applied_at)
VALUES (1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
"#;

const MIGRATION_2: &str = r#"
ALTER TABLE patients ADD COLUMN medicare_normalized TEXT;
CREATE INDEX patients_medicare_normalized ON patients(medicare_normalized);
ALTER TABLE patients ADD COLUMN name_normalized TEXT;
CREATE INDEX patients_name_normalized_dob ON patients(name_normalized, dob);
INSERT INTO schema_version(version, applied_at)
VALUES (2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
"#;

const MIGRATION_3: &str = r#"
CREATE TABLE auth_sessions (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    secret_hash TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);
INSERT INTO schema_version(version, applied_at)
VALUES (3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
"#;

const MIGRATION_4: &str = r#"
DROP TABLE IF EXISTS auth_sessions;

CREATE TABLE patients_without_user_refs (
    id TEXT PRIMARY KEY NOT NULL,
    last_name TEXT,
    first_name TEXT,
    full_name TEXT NOT NULL,
    gender TEXT,
    address TEXT,
    dob TEXT,
    phone TEXT,
    medicare TEXT,
    other_insurance TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    created_by TEXT,
    updated_by TEXT,
    medicare_normalized TEXT,
    name_normalized TEXT
);
INSERT INTO patients_without_user_refs (
    id,last_name,first_name,full_name,gender,address,dob,phone,medicare,other_insurance,
    created_at,updated_at,created_by,updated_by,medicare_normalized,name_normalized
)
SELECT id,last_name,first_name,full_name,gender,address,dob,phone,medicare,other_insurance,
       created_at,updated_at,NULL,NULL,medicare_normalized,name_normalized
FROM patients;
DROP TABLE patients;
ALTER TABLE patients_without_user_refs RENAME TO patients;
CREATE UNIQUE INDEX patients_medicare_unique
    ON patients(medicare) WHERE medicare IS NOT NULL AND trim(medicare) <> '';
CREATE INDEX patients_name_dob ON patients(last_name, first_name, dob);
CREATE INDEX patients_medicare_normalized ON patients(medicare_normalized);
CREATE INDEX patients_name_normalized_dob ON patients(name_normalized, dob);

CREATE TABLE audit_log_without_user_ref (
    id TEXT PRIMARY KEY NOT NULL,
    ts TEXT NOT NULL,
    user_id TEXT,
    action TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT,
    detail TEXT NOT NULL DEFAULT '{}',
    prev_hash TEXT NOT NULL,
    hash TEXT NOT NULL
);
INSERT INTO audit_log_without_user_ref (id,ts,user_id,action,entity_type,entity_id,detail,prev_hash,hash)
SELECT id,ts,NULL,action,entity_type,entity_id,detail,prev_hash,hash FROM audit_log;
DROP TABLE audit_log;
ALTER TABLE audit_log_without_user_ref RENAME TO audit_log;
CREATE INDEX audit_log_ts ON audit_log(ts);

DROP TABLE users;
INSERT INTO schema_version(version, applied_at)
VALUES (4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_removes_users_and_nulls_attribution_without_losing_records() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let db = Database::new_with_test_key(dir.path().join("migration.db"));
        let mut conn = Connection::open(db.path()).expect("open legacy db");
        conn.pragma_update(None, "key", "test-only-sqlcipher-key")
            .expect("set test key");
        conn.execute_batch(MIGRATION_1).expect("migration 1");
        conn.pragma_update(None, "user_version", 1)
            .expect("version 1");
        conn.execute_batch(MIGRATION_2).expect("migration 2");
        conn.pragma_update(None, "user_version", 2)
            .expect("version 2");
        conn.execute_batch(MIGRATION_3).expect("migration 3");
        conn.pragma_update(None, "user_version", 3)
            .expect("version 3");
        conn.execute("INSERT INTO users(id,username,display_name,password_hash,role,created_at,updated_at) VALUES('u1','legacy','Legacy User','hash','admin','now','now')", []).expect("legacy user");
        conn.execute("INSERT INTO patients(id,full_name,created_at,updated_at,created_by,updated_by) VALUES('p1','Test Patient','created','updated','u1','u1')", []).expect("legacy patient");
        conn.execute("INSERT INTO audit_log(id,ts,user_id,action,entity_type,detail,prev_hash,hash) VALUES('a1','now','u1','save','patient','{}','','h')", []).expect("legacy audit row");
        drop(conn);

        let conn = db.open().expect("apply migration 4");
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("read version");
        assert_eq!(version, 4);
        let user_table_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='users'",
                [],
                |row| row.get(0),
            )
            .expect("check user table");
        assert_eq!(user_table_count, 0);
        let session_table_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='auth_sessions'",
                [],
                |row| row.get(0),
            )
            .expect("check session table");
        assert_eq!(session_table_count, 0);
        let patient: (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT full_name,created_by,updated_by FROM patients WHERE id='p1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("preserved patient");
        assert_eq!(patient, ("Test Patient".into(), None, None));
        let audit_user: Option<String> = conn
            .query_row("SELECT user_id FROM audit_log WHERE id='a1'", [], |row| {
                row.get(0)
            })
            .expect("preserved audit row");
        assert_eq!(audit_user, None);
        let fk_violations: i64 = conn
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .expect("foreign key check");
        assert_eq!(fk_violations, 0);
        drop(conn);

        let patients = crate::patients::PatientRepository::new(db.clone());
        let saved = patients
            .save(crate::patients::PatientDraft {
                full_name: "New Local Patient".into(),
                gender: None,
                address: None,
                dob: None,
                phone: None,
                medicare: None,
                other_insurance: None,
            })
            .expect("save patient without user identity");
        let crate::patients::SavePatientResult::Created(patient_id) = saved else {
            panic!("expected new patient");
        };
        let conn = db.open().expect("read new patient");
        let attribution: (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT created_by,updated_by FROM patients WHERE id=?1",
                [&patient_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read attribution");
        assert_eq!(attribution, (None, None));
    }
}

use zeroize::Zeroize;
