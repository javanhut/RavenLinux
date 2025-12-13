use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create database directory: {}", parent.display())
            })?;
        }

        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open database file: {}", path.display()))?;
        let db = Database { conn };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open_default() -> Result<Self> {
        Self::open("/var/lib/rvn/packages.db")
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS packages (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                version TEXT NOT NULL,
                description TEXT,
                license TEXT,
                homepage TEXT,
                install_date INTEGER NOT NULL,
                explicit INTEGER NOT NULL DEFAULT 1,
                size INTEGER
            );

            CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY,
                package_id INTEGER NOT NULL,
                path TEXT NOT NULL UNIQUE,
                hash TEXT,
                size INTEGER,
                mode INTEGER,
                FOREIGN KEY (package_id) REFERENCES packages(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS dependencies (
                id INTEGER PRIMARY KEY,
                package_id INTEGER NOT NULL,
                depends_on TEXT NOT NULL,
                dep_type TEXT NOT NULL DEFAULT 'runtime',
                FOREIGN KEY (package_id) REFERENCES packages(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS repository_packages (
                id INTEGER PRIMARY KEY,
                repo TEXT NOT NULL,
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                description TEXT,
                download_size INTEGER,
                installed_size INTEGER,
                filename TEXT NOT NULL,
                sha256 TEXT NOT NULL,
                UNIQUE(repo, name, version)
            );

            CREATE INDEX IF NOT EXISTS idx_packages_name ON packages(name);
            CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);
            CREATE INDEX IF NOT EXISTS idx_files_package ON files(package_id);
            CREATE INDEX IF NOT EXISTS idx_repo_name ON repository_packages(name);
            ",
        )?;
        Ok(())
    }

    pub fn is_installed(&self, name: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM packages WHERE name = ?",
            params![name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn get_installed_version(&self, name: &str) -> Result<Option<String>> {
        let result = self.conn.query_row(
            "SELECT version FROM packages WHERE name = ?",
            params![name],
            |row| row.get(0),
        );

        match result {
            Ok(version) => Ok(Some(version)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn record_installation(
        &self,
        name: &str,
        version: &str,
        description: Option<&str>,
        explicit: bool,
        files: &[&str],
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        tx.execute(
            "INSERT OR REPLACE INTO packages (name, version, description, install_date, explicit)
             VALUES (?, ?, ?, strftime('%s', 'now'), ?)",
            params![name, version, description, explicit as i32],
        )?;

        let package_id: i64 = tx.query_row(
            "SELECT id FROM packages WHERE name = ?",
            params![name],
            |row| row.get(0),
        )?;

        for file in files {
            tx.execute(
                "INSERT OR REPLACE INTO files (package_id, path) VALUES (?, ?)",
                params![package_id, file],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn remove_package(&self, name: &str) -> Result<Vec<String>> {
        let package_id: i64 = self.conn.query_row(
            "SELECT id FROM packages WHERE name = ?",
            params![name],
            |row| row.get(0),
        )?;

        // Get files to remove
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM files WHERE package_id = ?")?;
        let files: Vec<String> = stmt
            .query_map(params![package_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        // Delete package (cascades to files and dependencies)
        self.conn
            .execute("DELETE FROM packages WHERE id = ?", params![package_id])?;

        Ok(files)
    }

    pub fn list_installed(&self) -> Result<Vec<(String, String, bool)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, version, explicit FROM packages ORDER BY name")?;

        let packages = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i32>(2)? != 0,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(packages)
    }

    pub fn search(&self, query: &str) -> Result<Vec<(String, String, String)>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT name, version, COALESCE(description, '')
             FROM repository_packages
             WHERE name LIKE ? OR description LIKE ?
             ORDER BY name",
        )?;

        let results = stmt
            .query_map(params![&pattern, &pattern], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }
}
