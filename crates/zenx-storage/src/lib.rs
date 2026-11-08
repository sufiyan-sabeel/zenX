//! Local-first structured storage.
//!
//! One SQLite database holds every module's persistent state, which keeps
//! backup, restore and migration to a single file. Schema changes are applied
//! through an ordered migration list keyed on `PRAGMA user_version`, so an
//! older binary refuses to open a newer database instead of corrupting it.

use std::path::Path;

use rusqlite::Connection;
use zenx_core::{Result, ZenxError};

pub mod schema;

/// Handle to the zenX database.
pub struct Db {
    conn: Connection,
}

impl Db {
    /// Open (creating if needed) the database at `path` and migrate it.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ZenxError::io(parent, e))?;
        }
        let conn = Connection::open(path).map_err(map_err)?;
        Self::prepare(conn)
    }

    /// Ephemeral database, used by tests.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(map_err)?;
        Self::prepare(conn)
    }

    fn prepare(conn: Connection) -> Result<Self> {
        // WAL keeps readers from blocking writers; in-memory databases ignore it.
        let _ = conn.execute_batch("PRAGMA journal_mode = WAL;");
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(map_err)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(map_err)?;
        let mut db = Db { conn };
        db.migrate()?;
        Ok(db)
    }

    /// Apply every migration newer than the stored schema version.
    pub fn migrate(&mut self) -> Result<()> {
        let current: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(map_err)?;
        let migrations = schema::MIGRATIONS;
        if current as usize > migrations.len() {
            return Err(ZenxError::Storage(format!(
                "database schema version {current} is newer than this build supports ({}); upgrade zenX",
                migrations.len()
            )));
        }
        for (index, sql) in migrations.iter().enumerate() {
            let version = index as i64 + 1;
            if version <= current {
                continue;
            }
            self.conn.execute_batch(sql).map_err(map_err)?;
            self.conn
                .execute_batch(&format!("PRAGMA user_version = {version};"))
                .map_err(map_err)?;
        }
        Ok(())
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn schema_version(&self) -> Result<i64> {
        self.conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(map_err)
    }

    /// Row count of a table, for `zenx status`. The name is validated against a
    /// fixed allowlist because it is interpolated into SQL.
    pub fn count(&self, table: &str) -> Result<i64> {
        if !schema::TABLES.contains(&table) {
            return Err(ZenxError::Storage(format!("unknown table {table:?}")));
        }
        self.conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .map_err(map_err)
    }
}

/// Convert a driver error into the platform error type.
pub fn map_err(error: rusqlite::Error) -> ZenxError {
    ZenxError::Storage(error.to_string())
}

/// Milliseconds since the Unix epoch, for ordering.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_create_every_table() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(
            db.schema_version().unwrap(),
            schema::MIGRATIONS.len() as i64
        );
        for table in schema::TABLES {
            assert_eq!(db.count(table).unwrap(), 0, "{table} should be empty");
        }
    }

    #[test]
    fn migrations_are_idempotent() {
        let mut db = Db::open_in_memory().unwrap();
        db.migrate().unwrap();
        db.migrate().unwrap();
        assert_eq!(
            db.schema_version().unwrap(),
            schema::MIGRATIONS.len() as i64
        );
    }

    #[test]
    fn unknown_table_is_rejected() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.count("sqlite_master").is_err());
        assert!(db.count("events; DROP TABLE events").is_err());
    }
}
