pub fn ping() -> String {
    "orders_core is alive".to_string()
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use std::io::Read;

    #[test]
    fn sqlcipher_is_active_and_enforces_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");

        // Create an encrypted DB
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "key", "correct-key").unwrap();

            // Returns a row only if SQLCipher is really linked
            let version: String = conn
                .query_row("PRAGMA cipher_version", [], |r| r.get(0))
                .expect("SQLCipher is not active");
            println!("cipher_version = {version}");

            conn.execute("CREATE TABLE t (id INTEGER)", []).unwrap();
            conn.execute("INSERT INTO t VALUES (1)", []).unwrap();
        }

        // The file must not start with the plain SQLite header
        let mut header = [0u8; 15];
        std::fs::File::open(&path).unwrap().read_exact(&mut header).unwrap();
        assert_ne!(&header, b"SQLite format 3", "database is NOT encrypted");

        // Wrong key must fail
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "key", "wrong-key").unwrap();
            let r: Result<i64, _> = conn.query_row("SELECT count(*) FROM t", [], |r| r.get(0));
            assert!(r.is_err(), "wrong key should not read the data");
        }

        // Correct key must work
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "key", "correct-key").unwrap();
            let n: i64 = conn.query_row("SELECT count(*) FROM t", [], |r| r.get(0)).unwrap();
            assert_eq!(n, 1);
        }
    }
}