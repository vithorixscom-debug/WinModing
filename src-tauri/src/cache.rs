use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};

use crate::domain::{InstanceRecord, LoaderKind, StoredAccount};
use crate::error::LauncherError;

#[derive(Clone)]
pub struct MetadataCache {
    conn: Arc<Mutex<Connection>>,
}

impl MetadataCache {
    pub fn open(path: &std::path::Path) -> Result<Self, LauncherError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| crate::error::io_at(parent, err))?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS api_cache (
                cache_key TEXT PRIMARY KEY,
                body TEXT NOT NULL,
                stored_unix INTEGER NOT NULL,
                ttl_secs INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS instances (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                game_version TEXT NOT NULL,
                loader TEXT NOT NULL,
                loader_version TEXT,
                java_major INTEGER,
                created_unix INTEGER NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                username TEXT NOT NULL,
                uuid TEXT NOT NULL,
                access_token TEXT NOT NULL,
                refresh_token TEXT,
                expires_unix INTEGER
            );
            CREATE TABLE IF NOT EXISTS installed_versions (
                id TEXT PRIMARY KEY,
                version_type TEXT NOT NULL,
                json_path TEXT NOT NULL,
                installed INTEGER NOT NULL
            );
            ",
        )?;
        // Older DBs created before sort_order — add and backfill from created_unix.
        let _ = conn.execute(
            "ALTER TABLE instances ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = conn.execute(
            "UPDATE instances SET sort_order = created_unix WHERE sort_order = 0 OR sort_order IS NULL",
            [],
        );
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn cached_body(&self, key: &str) -> Result<Option<String>, LauncherError> {
        let db = self.conn.clone();
        let key = key.to_string();
        spawn_db(move || {
            let conn = lock_db(&db);
            let mut stmt = conn.prepare(
                "SELECT body, stored_unix, ttl_secs FROM api_cache WHERE cache_key = ?1",
            )?;
            let mut rows = stmt.query(params![key])?;
            let Some(row) = rows.next()? else {
                return Ok(None);
            };
            let body: String = row.get(0)?;
            let stored: i64 = row.get(1)?;
            let ttl: i64 = row.get(2)?;
            let now = unix_now() as i64;
            if now.saturating_sub(stored) > ttl {
                return Ok(None);
            }
            Ok(Some(body))
        })
        .await
    }

    pub async fn store_body(
        &self,
        key: &str,
        body: &str,
        ttl_secs: i64,
    ) -> Result<(), LauncherError> {
        let db = self.conn.clone();
        let key = key.to_string();
        let body = body.to_string();
        spawn_db(move || {
            let conn = lock_db(&db);
            conn.execute(
                "INSERT INTO api_cache (cache_key, body, stored_unix, ttl_secs)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(cache_key) DO UPDATE SET
                    body = excluded.body,
                    stored_unix = excluded.stored_unix,
                    ttl_secs = excluded.ttl_secs",
                params![key, body, unix_now() as i64, ttl_secs],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn insert_instance(&self, record: InstanceRecord) -> Result<(), LauncherError> {
        let db = self.conn.clone();
        spawn_db(move || {
            let conn = lock_db(&db);
            let next_order: i64 = conn
                .query_row(
                    "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM instances",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(record.created_unix);
            conn.execute(
                "INSERT INTO instances (id, name, game_version, loader, loader_version, java_major, created_unix, sort_order)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    record.id,
                    record.name,
                    record.game_version,
                    record.loader.as_str(),
                    record.loader_version,
                    record.java_major.map(|v| v as i64),
                    record.created_unix,
                    if record.sort_order == 0 {
                        next_order
                    } else {
                        record.sort_order
                    }
                ],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn list_instances(&self) -> Result<Vec<InstanceRecord>, LauncherError> {
        let db = self.conn.clone();
        spawn_db(move || {
            let conn = lock_db(&db);
            let mut stmt = conn.prepare(
                "SELECT id, name, game_version, loader, loader_version, java_major, created_unix, sort_order
                 FROM instances ORDER BY sort_order ASC, created_unix DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                let loader: String = row.get(3)?;
                let java_major: Option<i64> = row.get(5)?;
                Ok(InstanceRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    game_version: row.get(2)?,
                    loader: LoaderKind::parse(&loader),
                    loader_version: row.get(4)?,
                    java_major: java_major.map(|v| v as u32),
                    created_unix: row.get(6)?,
                    sort_order: row.get(7).unwrap_or(0),
                })
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
        .await
    }

    pub async fn get_instance(&self, id: &str) -> Result<Option<InstanceRecord>, LauncherError> {
        let db = self.conn.clone();
        let id = id.to_string();
        spawn_db(move || {
            let conn = lock_db(&db);
            let mut stmt = conn.prepare(
                "SELECT id, name, game_version, loader, loader_version, java_major, created_unix, sort_order
                 FROM instances WHERE id = ?1",
            )?;
            let mut rows = stmt.query(params![id])?;
            let Some(row) = rows.next()? else {
                return Ok(None);
            };
            let loader: String = row.get(3)?;
            let java_major: Option<i64> = row.get(5)?;
            Ok(Some(InstanceRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                game_version: row.get(2)?,
                loader: LoaderKind::parse(&loader),
                loader_version: row.get(4)?,
                java_major: java_major.map(|v| v as u32),
                created_unix: row.get(6)?,
                sort_order: row.get(7).unwrap_or(0),
            }))
        })
        .await
    }

    /// Swap sort_order with the neighbour above (`up`) or below (`down`).
    pub async fn move_instance(&self, id: &str, direction: &str) -> Result<Vec<InstanceRecord>, LauncherError> {
        let db = self.conn.clone();
        let id = id.to_string();
        let up = direction.eq_ignore_ascii_case("up");
        spawn_db(move || {
            let conn = lock_db(&db);
            let mut stmt = conn.prepare(
                "SELECT id, sort_order FROM instances ORDER BY sort_order ASC, created_unix DESC",
            )?;
            let rows: Vec<(String, i64)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect();
            let Some(idx) = rows.iter().position(|(row_id, _)| row_id == &id) else {
                return Err(LauncherError::Message("instance not found".into()));
            };
            let swap_with = if up {
                idx.checked_sub(1)
            } else if idx + 1 < rows.len() {
                Some(idx + 1)
            } else {
                None
            };
            if let Some(other) = swap_with {
                let (id_a, order_a) = &rows[idx];
                let (id_b, order_b) = &rows[other];
                conn.execute(
                    "UPDATE instances SET sort_order = ?2 WHERE id = ?1",
                    params![id_a, order_b],
                )?;
                conn.execute(
                    "UPDATE instances SET sort_order = ?2 WHERE id = ?1",
                    params![id_b, order_a],
                )?;
            }
            Ok(())
        })
        .await?;
        self.list_instances().await
    }

    pub async fn update_instance(&self, record: InstanceRecord) -> Result<(), LauncherError> {
        let db = self.conn.clone();
        spawn_db(move || {
            let conn = lock_db(&db);
            conn.execute(
                "UPDATE instances SET name = ?2, game_version = ?3, loader = ?4, loader_version = ?5, java_major = ?6
                 WHERE id = ?1",
                params![
                    record.id,
                    record.name,
                    record.game_version,
                    record.loader.as_str(),
                    record.loader_version,
                    record.java_major.map(|v| v as i64),
                ],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn delete_instance(&self, id: &str) -> Result<(), LauncherError> {
        let db = self.conn.clone();
        let id = id.to_string();
        spawn_db(move || {
            let conn = lock_db(&db);
            conn.execute("DELETE FROM instances WHERE id = ?1", params![id])?;
            Ok(())
        })
        .await
    }

    pub async fn upsert_account(&self, account: StoredAccount) -> Result<(), LauncherError> {
        let db = self.conn.clone();
        spawn_db(move || {
            let conn = lock_db(&db);
            conn.execute(
                "INSERT INTO accounts (id, kind, username, uuid, access_token, refresh_token, expires_unix)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                    kind = excluded.kind,
                    username = excluded.username,
                    uuid = excluded.uuid,
                    access_token = excluded.access_token,
                    refresh_token = excluded.refresh_token,
                    expires_unix = excluded.expires_unix",
                params![
                    account.id,
                    account.kind,
                    account.username,
                    account.uuid,
                    account.access_token,
                    account.refresh_token,
                    account.expires_unix
                ],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn list_account_views(
        &self,
    ) -> Result<Vec<crate::domain::AccountView>, LauncherError> {
        let accounts = self.list_accounts().await?;
        Ok(accounts.iter().map(StoredAccount::view).collect())
    }

    pub async fn list_accounts(&self) -> Result<Vec<StoredAccount>, LauncherError> {
        let db = self.conn.clone();
        spawn_db(move || {
            let conn = lock_db(&db);
            let mut stmt = conn.prepare(
                "SELECT id, kind, username, uuid, access_token, refresh_token, expires_unix FROM accounts",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(StoredAccount {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    username: row.get(2)?,
                    uuid: row.get(3)?,
                    access_token: row.get(4)?,
                    refresh_token: row.get(5)?,
                    expires_unix: row.get(6)?,
                })
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
        .await
    }

    pub async fn get_account(&self, id: &str) -> Result<Option<StoredAccount>, LauncherError> {
        let accounts = self.list_accounts().await?;
        Ok(accounts.into_iter().find(|account| account.id == id))
    }

    pub async fn delete_account(&self, id: &str) -> Result<(), LauncherError> {
        let db = self.conn.clone();
        let id = id.to_string();
        spawn_db(move || {
            let conn = lock_db(&db);
            conn.execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
            Ok(())
        })
        .await
    }

    pub async fn mark_version_installed(
        &self,
        id: &str,
        version_type: &str,
        json_path: &str,
    ) -> Result<(), LauncherError> {
        let db = self.conn.clone();
        let id = id.to_string();
        let version_type = version_type.to_string();
        let json_path = json_path.to_string();
        spawn_db(move || {
            let conn = lock_db(&db);
            conn.execute(
                "INSERT INTO installed_versions (id, version_type, json_path, installed)
                 VALUES (?1, ?2, ?3, 1)
                 ON CONFLICT(id) DO UPDATE SET
                    version_type = excluded.version_type,
                    json_path = excluded.json_path,
                    installed = 1",
                params![id, version_type, json_path],
            )?;
            Ok(())
        })
        .await
    }
}

fn lock_db(db: &Mutex<Connection>) -> MutexGuard<'_, Connection> {
    db.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

async fn spawn_db<T, F>(job: F) -> Result<T, LauncherError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, LauncherError> + Send + 'static,
{
    match tokio::task::spawn_blocking(job).await {
        Ok(result) => result,
        Err(err) => Err(LauncherError::Cache(format!("database task failed: {err}"))),
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
