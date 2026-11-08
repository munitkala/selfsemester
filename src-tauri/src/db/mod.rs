//! SQLite storage: one file, versioned with `PRAGMA user_version`.

use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

pub mod jobs;

/// Migrations in order; index + 1 = schema version after applying it.
const MIGRATIONS: &[&str] = &[include_str!("schema_v1.sql"), include_str!("schema_v2.sql"), include_str!("schema_v3.sql"), include_str!("schema_v4.sql"), include_str!("schema_v5.sql")];

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    configure(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

/// The schema version this build upgrades databases to.
pub fn latest_schema() -> u32 {
    MIGRATIONS.len() as u32
}

pub fn schema_version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.pragma_query_value(None, "user_version", |r| r.get(0))
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let current = schema_version(conn)? as usize;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as u32)?;
        tx.commit()?;
    }
    Ok(())
}

pub fn now_utc() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
        .optional()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_to_latest_and_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("app.db");
        let conn = open(&path).unwrap();
        assert_eq!(schema_version(&conn).unwrap() as usize, MIGRATIONS.len());
        drop(conn);
        let conn = open(&path).unwrap(); // reopening must not re-run migrations
        assert_eq!(schema_version(&conn).unwrap() as usize, MIGRATIONS.len());
    }

    #[test]
    fn fts5_indexes_chunks() {
        let conn = open_in_memory().unwrap();
        let now = now_utc();
        conn.execute("INSERT INTO profiles(id,name,created_at) VALUES ('p','Me',?1)", [&now]).unwrap();
        conn.execute(
            "INSERT INTO courses(id,profile_id,title,subject,created_at,updated_at)
             VALUES ('c','p','QM 1','quantum mechanics',?1,?1)",
            [&now],
        )
        .unwrap();
        conn.execute("INSERT INTO sources(id,course_id,url) VALUES ('s','c','https://openstax.org')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO chunks(source_id,seq,text) VALUES ('s',0,'The Schrodinger equation describes the wave function')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO chunks(source_id,seq,text) VALUES ('s',1,'Unrelated text about kilns')", [])
            .unwrap();
        let hits: i64 = conn
            .query_row("SELECT count(*) FROM chunks_fts WHERE chunks_fts MATCH 'schrodinger'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(hits, 1);
    }

    #[test]
    fn settings_roundtrip() {
        let conn = open_in_memory().unwrap();
        assert_eq!(get_setting(&conn, "k").unwrap(), None);
        set_setting(&conn, "k", "a").unwrap();
        set_setting(&conn, "k", "b").unwrap();
        assert_eq!(get_setting(&conn, "k").unwrap().as_deref(), Some("b"));
    }
}
