//! The encrypted local store (SQLCipher). Every record is a FHIR R4 resource
//! kept as JSON and belongs to one person (a Patient). Every change is written
//! at once and recorded in `history`, so it can be undone; deleting moves a
//! record to the trash, where it stays for [`TRASH_DAYS`]. Backups are
//! encrypted copies made with `sqlcipher_export`.

use std::path::{Path, PathBuf};

use jiff::{SignedDuration, Timestamp};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::fhir::{self, Invalid};
use crate::keys::Key;

/// Days a deleted record stays restorable.
pub const TRASH_DAYS: i64 = 30;
/// Daily backups kept in each backup folder.
pub const KEEP_BACKUPS: usize = 14;
const BACKUP_PREFIX: &str = "swasthya-";
/// Name prefix of the backups made before an update.
pub const PRE_UPDATE: &str = "pre-update-";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("file: {0}")]
    Io(#[from] std::io::Error),
    #[error("the key does not open this database")]
    WrongKey,
    #[error("no record with id {0}")]
    NotFound(String),
    #[error("no person with id {0}, or they are in the trash")]
    NoPerson(String),
    #[error("this database was made by a newer Arogya Swasthya (schema {0}); update the app")]
    TooNew(i64),
    #[error("{0}")]
    Invalid(#[from] Invalid),
    #[error("{0} already exists")]
    Exists(PathBuf),
}

pub type Result<T> = std::result::Result<T, Error>;

/// What a backup holds, shown before restoring it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub schema: i64,
    /// Names of the people in it (not in the trash).
    pub people: Vec<String>,
    /// Records other than people, not in the trash.
    pub records: usize,
    pub trashed: usize,
    /// When a record was last changed, if any.
    pub last_change: Option<Timestamp>,
}

/// Schema steps; index + 1 is the version they lead to. Never edit a step
/// that has shipped: add a new one.
const MIGRATIONS: &[&str] = &["
    CREATE TABLE records (
        id TEXT PRIMARY KEY,
        type TEXT NOT NULL,
        person_id TEXT NOT NULL,
        date TEXT,
        json TEXT NOT NULL,
        updated_at INTEGER NOT NULL,
        deleted_at INTEGER
    );
    CREATE INDEX records_person ON records(person_id, type, deleted_at);
    CREATE INDEX records_date ON records(person_id, date);
    CREATE INDEX records_deleted ON records(deleted_at);
    CREATE TABLE history (
        seq INTEGER PRIMARY KEY AUTOINCREMENT,
        group_id INTEGER NOT NULL,
        record_id TEXT NOT NULL,
        before_json TEXT,
        before_deleted_at INTEGER,
        label TEXT NOT NULL,
        at INTEGER NOT NULL
    );
    CREATE TABLE kv (key TEXT PRIMARY KEY, json TEXT NOT NULL);
"];

pub struct Store {
    conn: Connection,
}

/// What `undo` put back, for the "Undone: ..." message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Undone {
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trashed {
    pub resource: Value,
    pub deleted_at: Timestamp,
}

impl Store {
    /// Open (or create) the database at `path` with a 32-byte key.
    pub fn open(path: &Path, key: &Key) -> Result<Store> {
        let conn = Connection::open(path)?;
        conn.execute_batch(&format!("PRAGMA key = \"{}\";", key_literal(key)))?;
        Store::init(conn)
    }

    /// An in-memory store for tests and previews.
    pub fn memory() -> Result<Store> {
        Store::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Store> {
        // A wrong key only shows when the first page is read.
        if conn
            .query_row("SELECT count(*) FROM sqlite_master", [], |r| {
                r.get::<_, i64>(0)
            })
            .is_err()
        {
            return Err(Error::WrongKey);
        }
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        let mut store = Store { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    fn migrate(&mut self) -> Result<()> {
        let v = self.schema_version()?;
        if v > MIGRATIONS.len() as i64 {
            return Err(Error::TooNew(v));
        }
        for (i, step) in MIGRATIONS.iter().enumerate().skip(v as usize) {
            let tx = self.conn.transaction()?;
            tx.execute_batch(step)?;
            tx.pragma_update(None, "user_version", i as i64 + 1)?;
            tx.commit()?;
        }
        Ok(())
    }

    /// Everyone whose record is kept here, oldest first.
    pub fn people(&self) -> Result<Vec<Value>> {
        self.query(
            "SELECT json FROM records WHERE type = 'Patient' AND deleted_at IS NULL ORDER BY id",
            [],
        )
    }

    pub fn get(&self, id: &str) -> Result<Value> {
        let j: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM records WHERE id = ? AND deleted_at IS NULL",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(serde_json::from_str(
            &j.ok_or_else(|| Error::NotFound(id.into()))?,
        )?)
    }

    /// One person's records of one type (e.g. "Condition"), newest first.
    pub fn records(&self, person: &str, kind: &str) -> Result<Vec<Value>> {
        self.query(
            "SELECT json FROM records WHERE person_id = ?1 AND type = ?2 AND deleted_at IS NULL
             ORDER BY date IS NULL, date DESC, id",
            params![person, kind],
        )
    }

    /// Everything about one person except the Patient record itself, newest first;
    /// undated records come last.
    pub fn timeline(&self, person: &str) -> Result<Vec<Value>> {
        self.query(
            "SELECT json FROM records WHERE person_id = ? AND type != 'Patient' AND deleted_at IS NULL
             ORDER BY date IS NULL, date DESC, id",
            [person],
        )
    }

    pub fn trash(&self) -> Result<Vec<Trashed>> {
        let mut st = self
            .conn
            .prepare("SELECT json, deleted_at FROM records WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC, id")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        rows.map(|row| {
            let (j, at) = row?;
            Ok(Trashed {
                resource: serde_json::from_str(&j)?,
                deleted_at: ts(at),
            })
        })
        .collect()
    }

    /// Create or update a record and return it as stored. A new record gets an
    /// id; every save stamps `meta.lastUpdated`. Anything but a Patient must
    /// point at a person who is kept here and not in the trash.
    pub fn save(&mut self, mut resource: Value, now: Timestamp) -> Result<Value> {
        let kind = fhir::resource_type(&resource)?.to_string();
        let obj = resource.as_object_mut().ok_or(Invalid::NoType)?;
        match obj.get("id").and_then(Value::as_str) {
            Some(id) if !fhir::valid_id(id) => return Err(Invalid::BadId.into()),
            Some(_) => {}
            None => {
                obj.insert("id".into(), json!(uuid::Uuid::now_v7().to_string()));
            }
        }
        let meta = obj.entry("meta").or_insert_with(|| json!({}));
        if let Some(m) = meta.as_object_mut() {
            m.insert("lastUpdated".into(), json!(now.to_string()));
        }
        let person = fhir::person_of(&resource)?.expect("a Patient has an id by now");
        if kind != "Patient" && !self.is_live("Patient", &person)? {
            return Err(Error::NoPerson(person));
        }
        let id = resource["id"].as_str().unwrap_or_default().to_string();
        let verb = if self.exists(&id)? { "edit" } else { "add" };
        let label = format!("{verb} {}", describe(&resource));
        let json = serde_json::to_string(&resource)?;
        self.change(|tx, group| {
            record(tx, group, &id, &label, now)?;
            write(tx, &json, now.as_second(), None)
        })?;
        Ok(resource)
    }

    /// Move to the trash. Trashing a person trashes all their records with
    /// them, as one undo step.
    pub fn delete(&mut self, id: &str, now: Timestamp) -> Result<()> {
        let resource = self.get(id)?;
        let label = format!("delete {}", describe(&resource));
        let ids: Vec<String> = if resource["resourceType"] == "Patient" {
            let mut st = self
                .conn
                .prepare("SELECT id FROM records WHERE person_id = ? AND deleted_at IS NULL")?;
            st.query_map([id], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        } else {
            vec![id.to_string()]
        };
        self.change(|tx, group| {
            for rid in &ids {
                record(tx, group, rid, &label, now)?;
                tx.execute(
                    "UPDATE records SET deleted_at = ? WHERE id = ?",
                    params![now.as_second(), rid],
                )?;
            }
            Ok(())
        })
    }

    /// Take a record out of the trash. A person comes back with the records
    /// that were trashed together with them.
    pub fn restore(&mut self, id: &str, now: Timestamp) -> Result<()> {
        let row: Option<(String, String, i64)> = self
            .conn
            .query_row("SELECT type, person_id, deleted_at FROM records WHERE id = ? AND deleted_at IS NOT NULL", [id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()?;
        let (kind, person, deleted_at) = row.ok_or_else(|| Error::NotFound(id.into()))?;
        let ids: Vec<String> = if kind == "Patient" {
            let mut st = self
                .conn
                .prepare("SELECT id FROM records WHERE person_id = ? AND deleted_at = ?")?;
            st.query_map(params![id, deleted_at], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        } else if !self.is_live("Patient", &person)? {
            return Err(Error::NoPerson(person));
        } else {
            vec![id.to_string()]
        };
        let label = "restore from trash".to_string();
        self.change(|tx, group| {
            for rid in &ids {
                record(tx, group, rid, &label, now)?;
                tx.execute("UPDATE records SET deleted_at = NULL WHERE id = ?", [rid])?;
            }
            Ok(())
        })
    }

    /// Undo the last change (all rows it touched). `None` when nothing is left.
    pub fn undo(&mut self) -> Result<Option<Undone>> {
        let tx = self.conn.transaction()?;
        let Some((group, label)) = tx
            .query_row(
                "SELECT group_id, label FROM history ORDER BY seq DESC LIMIT 1",
                [],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?
        else {
            return Ok(None);
        };
        {
            let mut st = tx.prepare(
                "SELECT record_id, before_json, before_deleted_at, at FROM history WHERE group_id = ? ORDER BY seq DESC",
            )?;
            let rows: Vec<(String, Option<String>, Option<i64>, i64)> = st
                .query_map([group], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
            for (id, before, deleted, at) in rows {
                match before {
                    None => {
                        tx.execute("DELETE FROM records WHERE id = ?", [&id])?;
                    }
                    Some(j) => write(&tx, &j, at, deleted)?,
                }
            }
        }
        tx.execute("DELETE FROM history WHERE group_id = ?", [group])?;
        tx.commit()?;
        Ok(Some(Undone { label }))
    }

    /// Remove trash and undo history older than [`TRASH_DAYS`].
    pub fn purge(&mut self, now: Timestamp) -> Result<usize> {
        let cut = (now - SignedDuration::from_hours(24 * TRASH_DAYS)).as_second();
        let tx = self.conn.transaction()?;
        let n = tx.execute(
            "DELETE FROM records WHERE deleted_at IS NOT NULL AND deleted_at < ?",
            [cut],
        )?;
        tx.execute("DELETE FROM history WHERE at < ?", [cut])?;
        tx.commit()?;
        Ok(n)
    }

    pub fn setting<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let j: Option<String> = self
            .conn
            .query_row("SELECT json FROM kv WHERE key = ?", [key], |r| r.get(0))
            .optional()?;
        j.map(|j| Ok(serde_json::from_str(&j)?)).transpose()
    }

    pub fn set_setting<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.conn.execute(
            "INSERT INTO kv (key, json) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET json = ?2",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }

    /// The records as a FHIR Bundle (type `collection`): one person's, or
    /// everyone's. Plain JSON: the user chose to take a readable copy out.
    pub fn export_bundle(&self, person: Option<&str>, now: Timestamp) -> Result<Value> {
        let resources = match person {
            Some(p) => {
                let mut all = vec![self.get(p)?];
                all.extend(self.timeline(p)?);
                all
            }
            None => self.query("SELECT json FROM records WHERE deleted_at IS NULL ORDER BY person_id, type != 'Patient', date, id", [])?,
        };
        let entry: Vec<Value> = resources
            .into_iter()
            .map(|r| json!({ "fullUrl": format!("{}/{}", r["resourceType"].as_str().unwrap_or(""), r["id"].as_str().unwrap_or("")), "resource": r }))
            .collect();
        Ok(
            json!({ "resourceType": "Bundle", "type": "collection", "timestamp": now.to_string(), "entry": entry }),
        )
    }

    /// Write an encrypted copy to `path` (which must not exist), locked with `key`.
    pub fn export(&self, path: &Path, key: &Key) -> Result<()> {
        export_conn(&self.conn, path, key)
    }

    /// Today's backup into `dir`, keeping the newest [`KEEP_BACKUPS`]. Call it
    /// for the app's own folder and again for the folder the user picked.
    /// Returns the new file, or `None` if today's backup already exists.
    pub fn daily_backup(&self, dir: &Path, key: &Key, now: Timestamp) -> Result<Option<PathBuf>> {
        std::fs::create_dir_all(dir)?;
        let name = format!("{BACKUP_PREFIX}{}.db", now.strftime("%Y-%m-%d"));
        let path = dir.join(&name);
        if path.exists() {
            return Ok(None);
        }
        let tmp = dir.join(format!("{name}.partial"));
        let _ = std::fs::remove_file(&tmp);
        self.export(&tmp, key)?;
        std::fs::rename(&tmp, &path)?;
        prune(dir, BACKUP_PREFIX, KEEP_BACKUPS)?;
        Ok(Some(path))
    }

    /// Backup made just before installing another version, named after the
    /// version running now, so going back can restore it. Kept apart from the
    /// daily ones: the newest `keep` stay.
    pub fn pre_update_backup(
        &self,
        dir: &Path,
        key: &Key,
        version: &str,
        now: Timestamp,
        keep: usize,
    ) -> Result<PathBuf> {
        if version.is_empty()
            || !version
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            return Err(Error::Io(std::io::Error::other(format!(
                "odd version {version:?}"
            ))));
        }
        std::fs::create_dir_all(dir)?;
        let name = format!(
            "{PRE_UPDATE}{}-{version}.db",
            now.strftime("%Y-%m-%d-%H%M%S")
        );
        let path = dir.join(&name);
        let tmp = dir.join(format!("{name}.partial"));
        let _ = std::fs::remove_file(&tmp);
        self.export(&tmp, key)?;
        std::fs::rename(&tmp, &path)?;
        prune(dir, PRE_UPDATE, keep)?;
        Ok(path)
    }

    pub fn summary(&self) -> Result<Summary> {
        summarise(&self.conn)
    }

    fn query(&self, sql: &str, args: impl rusqlite::Params) -> Result<Vec<Value>> {
        let mut st = self.conn.prepare(sql)?;
        let rows = st.query_map(args, |r| r.get::<_, String>(0))?;
        rows.map(|j| Ok(serde_json::from_str(&j?)?)).collect()
    }

    fn exists(&self, id: &str) -> Result<bool> {
        Ok(self
            .conn
            .query_row("SELECT count(*) FROM records WHERE id = ?", [id], |r| {
                r.get::<_, i64>(0)
            })?
            > 0)
    }

    fn is_live(&self, kind: &str, id: &str) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT count(*) FROM records WHERE id = ? AND type = ? AND deleted_at IS NULL",
            [id, kind],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Run one change in a transaction; all its rows share one undo group.
    fn change(&mut self, f: impl FnOnce(&Transaction, i64) -> Result<()>) -> Result<()> {
        let tx = self.conn.transaction()?;
        let group: i64 = tx.query_row(
            "SELECT coalesce(max(group_id), 0) + 1 FROM history",
            [],
            |r| r.get(0),
        )?;
        f(&tx, group)?;
        tx.commit()?;
        Ok(())
    }
}

/// Backup files in `dir`, in no particular order.
pub fn backups(dir: &Path) -> Result<Vec<PathBuf>> {
    Ok(std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(BACKUP_PREFIX) && n.ends_with(".db"))
        })
        .collect())
}

/// Keep the newest `keep` backups whose names start with `prefix`. Names
/// carry the date first, so name order is age order.
fn prune(dir: &Path, prefix: &str, keep: usize) -> Result<()> {
    let mut all: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(prefix) && n.ends_with(".db"))
        })
        .collect();
    all.sort();
    let excess = all.len().saturating_sub(keep);
    for old in &all[..excess] {
        std::fs::remove_file(old)?;
    }
    Ok(())
}

fn export_conn(conn: &Connection, path: &Path, key: &Key) -> Result<()> {
    if path.exists() {
        return Err(Error::Exists(path.into()));
    }
    let p = path
        .to_str()
        .ok_or_else(|| Error::Io(std::io::Error::other("backup path is not UTF-8")))?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    conn.execute(
        "ATTACH DATABASE ?1 AS backup KEY ?2",
        params![p, key_literal(key)],
    )?;
    let r = conn
        .query_row("SELECT sqlcipher_export('backup')", [], |_| Ok(()))
        .and_then(|_| conn.execute_batch(&format!("PRAGMA backup.user_version = {version}")));
    conn.execute("DETACH DATABASE backup", [])?;
    Ok(r?)
}

fn summarise(conn: &Connection) -> Result<Summary> {
    let schema: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let mut st = conn.prepare("SELECT type, json, updated_at, deleted_at FROM records")?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, Option<i64>>(3)?,
        ))
    })?;
    let mut sum = Summary {
        schema,
        people: Vec::new(),
        records: 0,
        trashed: 0,
        last_change: None,
    };
    for row in rows {
        let (kind, json, updated, deleted) = row?;
        sum.last_change = sum.last_change.max(Timestamp::from_second(updated).ok());
        match (deleted, kind.as_str()) {
            (Some(_), _) => sum.trashed += 1,
            (None, "Patient") => {
                let r: Value = serde_json::from_str(&json)?;
                let name = r["name"][0]["text"].as_str().unwrap_or("Unnamed");
                sum.people.push(name.to_owned());
            }
            (None, _) => sum.records += 1,
        }
    }
    sum.people.sort();
    Ok(sum)
}

/// Open a copy of the backup at `src` with `key` (in `work`, removed again)
/// and say what it holds. `src` itself is only read.
pub fn inspect(src: &Path, key: &Key, work: &Path) -> Result<Summary> {
    let copy = work.join("inspect.db");
    let r = with_copy(src, key, &copy, summarise);
    let _ = std::fs::remove_file(&copy);
    r
}

/// Turn the backup at `src` (opened with `src_key`, which may be another
/// device's) into a database locked with `key`, written to `dest` (which must
/// not exist). Refuses a backup from a newer app. `src` itself is only read;
/// the caller swaps `dest` in.
pub fn restore_copy(src: &Path, src_key: &Key, dest: &Path, key: &Key) -> Result<Summary> {
    let copy = dest.with_extension("source");
    let r = with_copy(src, src_key, &copy, |conn| {
        let sum = summarise(conn)?;
        export_conn(conn, dest, key)?;
        Ok(sum)
    });
    let _ = std::fs::remove_file(&copy);
    if r.is_err() {
        let _ = std::fs::remove_file(dest);
    }
    r
}

fn with_copy<T>(
    src: &Path,
    key: &Key,
    copy: &Path,
    f: impl FnOnce(&Connection) -> Result<T>,
) -> Result<T> {
    let _ = std::fs::remove_file(copy);
    std::fs::copy(src, copy)?;
    let conn = Connection::open(copy)?;
    conn.execute_batch(&format!("PRAGMA key = \"{}\";", key_literal(key)))?;
    if conn
        .query_row("SELECT count(*) FROM sqlite_master", [], |r| {
            r.get::<_, i64>(0)
        })
        .is_err()
    {
        return Err(Error::WrongKey);
    }
    let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if v > MIGRATIONS.len() as i64 {
        return Err(Error::TooNew(v));
    }
    f(&conn)
}

/// Insert or replace a row from its JSON; the indexed columns come from the JSON.
fn write(tx: &Transaction, json: &str, updated_at: i64, deleted_at: Option<i64>) -> Result<()> {
    let r: Value = serde_json::from_str(json)?;
    let kind = fhir::resource_type(&r)?;
    let person = fhir::person_of(&r)?.ok_or(Invalid::BadId)?;
    tx.execute(
        "INSERT INTO records (id, type, person_id, date, json, updated_at, deleted_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET type = ?2, person_id = ?3, date = ?4, json = ?5, updated_at = ?6, deleted_at = ?7",
        params![r["id"].as_str(), kind, person, fhir::sort_date(&r), json, updated_at, deleted_at],
    )?;
    Ok(())
}

/// Remember the row as it was before this change.
fn record(tx: &Transaction, group: i64, id: &str, label: &str, now: Timestamp) -> Result<()> {
    let before: Option<(String, Option<i64>)> = tx
        .query_row(
            "SELECT json, deleted_at FROM records WHERE id = ?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (json, deleted) = before.map_or((None, None), |(j, d)| (Some(j), d));
    tx.execute(
        "INSERT INTO history (group_id, record_id, before_json, before_deleted_at, label, at) VALUES (?, ?, ?, ?, ?, ?)",
        params![group, id, json, deleted, label, now.as_second()],
    )?;
    Ok(())
}

/// A short name for undo messages: the record's own text if it has one.
fn describe(r: &Value) -> String {
    let kind = r["resourceType"].as_str().unwrap_or("record");
    let text = ["code", "vaccineCode", "medicationCodeableConcept"]
        .iter()
        .find_map(|f| r[f]["text"].as_str().map(String::from))
        .or_else(|| r["name"][0]["text"].as_str().map(String::from));
    match text {
        Some(t) => format!("{kind} “{t}”"),
        None => kind.to_string(),
    }
}

/// SQLCipher's raw-key form: x'<64 hex digits>', used as-is with no key derivation.
fn key_literal(key: &Key) -> String {
    let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
    format!("x'{hex}'")
}

fn ts(s: i64) -> Timestamp {
    Timestamp::from_second(s).unwrap_or(Timestamp::UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> Timestamp {
        s.parse().unwrap()
    }
    const KEY: Key = [7; 32];
    const NOW: &str = "2026-11-04T09:00:00Z";

    // Synthetic people only.
    fn person(s: &mut Store, name: &str) -> String {
        let p = s
            .save(
                json!({"resourceType": "Patient", "name": [{"text": name}]}),
                at(NOW),
            )
            .unwrap();
        p["id"].as_str().unwrap().to_string()
    }

    fn condition(s: &mut Store, who: &str, text: &str, onset: &str) -> String {
        let c = json!({"resourceType": "Condition", "subject": {"reference": format!("Patient/{who}")},
                       "code": {"text": text}, "onsetDateTime": onset});
        s.save(c, at(NOW)).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn records_belong_to_people_and_sort_by_date() {
        let mut s = Store::memory().unwrap();
        let asha = person(&mut s, "Asha Test");
        let ravi = person(&mut s, "Ravi Test");
        condition(&mut s, &asha, "Hypothyroidism", "2018-03");
        condition(&mut s, &asha, "Type 2 diabetes", "2021-06-14");
        condition(&mut s, &ravi, "Asthma", "2010");
        s.save(json!({"resourceType": "Immunization", "patient": {"reference": format!("Patient/{asha}")},
                      "vaccineCode": {"text": "Flu"}, "occurrenceDateTime": "2025-10-02"}), at(NOW)).unwrap();
        assert_eq!(s.people().unwrap().len(), 2);
        let names: Vec<_> = s
            .records(&asha, "Condition")
            .unwrap()
            .iter()
            .map(|c| c["code"]["text"].clone())
            .collect();
        assert_eq!(names, [json!("Type 2 diabetes"), json!("Hypothyroidism")]);
        let line = s.timeline(&asha).unwrap();
        assert_eq!(line.len(), 3);
        assert_eq!(line[0]["resourceType"], "Immunization");
        assert_eq!(s.timeline(&ravi).unwrap().len(), 1);
        assert_eq!(line[0]["meta"]["lastUpdated"], NOW);
    }

    #[test]
    fn refuses_records_without_a_live_person() {
        let mut s = Store::memory().unwrap();
        let c = json!({"resourceType": "Condition", "subject": {"reference": "Patient/nobody"}});
        assert!(matches!(s.save(c, at(NOW)), Err(Error::NoPerson(_))));
        assert!(matches!(
            s.save(json!({"resourceType": "Claim"}), at(NOW)),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            s.save(json!({"resourceType": "Patient", "id": "bad id"}), at(NOW)),
            Err(Error::Invalid(_))
        ));
    }

    #[test]
    fn edit_and_undo() {
        let mut s = Store::memory().unwrap();
        let p = person(&mut s, "Asha Test");
        let id = condition(&mut s, &p, "Back pain", "2024-01");
        let mut c = s.get(&id).unwrap();
        c["code"]["text"] = json!("Lower back pain, L4-L5");
        s.save(c, at(NOW)).unwrap();
        assert_eq!(
            s.undo().unwrap().unwrap().label,
            "edit Condition “Lower back pain, L4-L5”"
        );
        assert_eq!(s.get(&id).unwrap()["code"]["text"], "Back pain");
        assert_eq!(
            s.undo().unwrap().unwrap().label,
            "add Condition “Back pain”"
        );
        assert!(s.get(&id).is_err());
    }

    #[test]
    fn trashing_a_person_takes_their_records_and_brings_them_back() {
        let mut s = Store::memory().unwrap();
        let p = person(&mut s, "Asha Test");
        let early = condition(&mut s, &p, "Old sprain", "2012");
        let now = at(NOW);
        s.delete(&early, now - SignedDuration::from_hours(1))
            .unwrap();
        condition(&mut s, &p, "Asthma", "2015");
        s.delete(&p, now).unwrap();
        assert!(s.people().unwrap().is_empty());
        assert_eq!(s.trash().unwrap().len(), 3);
        // A record cannot come back while its person is in the trash.
        assert!(matches!(s.restore(&early, now), Err(Error::NoPerson(_))));
        s.restore(&p, now).unwrap();
        assert_eq!(
            s.timeline(&p).unwrap().len(),
            1,
            "only what was trashed with the person returns"
        );
        s.undo().unwrap();
        assert!(s.people().unwrap().is_empty());
        s.undo().unwrap();
        assert_eq!(
            s.timeline(&p).unwrap().len(),
            1,
            "undoing the delete restores the person and records"
        );
    }

    #[test]
    fn trash_is_kept_for_thirty_days() {
        let mut s = Store::memory().unwrap();
        let p = person(&mut s, "Asha Test");
        let id = condition(&mut s, &p, "Cold", "2026-01");
        s.delete(&id, at(NOW)).unwrap();
        assert_eq!(
            s.purge(at(NOW) + SignedDuration::from_hours(24 * 29))
                .unwrap(),
            0
        );
        assert_eq!(
            s.purge(at(NOW) + SignedDuration::from_hours(24 * 31))
                .unwrap(),
            1
        );
        assert!(s.trash().unwrap().is_empty());
    }

    #[test]
    fn exports_a_fhir_bundle() {
        let mut s = Store::memory().unwrap();
        let p = person(&mut s, "Asha Test");
        let other = person(&mut s, "Ravi Test");
        condition(&mut s, &p, "Asthma", "2015");
        condition(&mut s, &other, "Gout", "2020");
        let mine = s.export_bundle(Some(&p), at(NOW)).unwrap();
        assert_eq!(mine["type"], "collection");
        assert_eq!(mine["entry"].as_array().unwrap().len(), 2);
        assert_eq!(mine["entry"][0]["fullUrl"], format!("Patient/{p}"));
        assert_eq!(
            s.export_bundle(None, at(NOW)).unwrap()["entry"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
    }

    #[test]
    fn encrypted_file_backups_and_restore() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("swasthya.db");
        let backups_dir = dir.path().join("backups");
        let mut s = Store::open(&db, &KEY).unwrap();
        let p = person(&mut s, "Asha Test");
        condition(&mut s, &p, "Asthma", "2015");
        s.set_setting("backup.folder", &"/somewhere").unwrap();
        let day = at(NOW);
        let first = s.daily_backup(&backups_dir, &KEY, day).unwrap().unwrap();
        assert!(
            s.daily_backup(&backups_dir, &KEY, day).unwrap().is_none(),
            "one a day"
        );
        for d in 1..=20 {
            s.daily_backup(&backups_dir, &KEY, day + SignedDuration::from_hours(24 * d))
                .unwrap();
        }
        assert_eq!(backups(&backups_dir).unwrap().len(), KEEP_BACKUPS);
        assert!(!first.exists(), "oldest backups are removed");
        let keep = backups(&backups_dir).unwrap().into_iter().min().unwrap();

        // The backup is encrypted: the wrong key cannot open it, and its bytes do not show the data.
        assert!(matches!(Store::open(&keep, &[9; 32]), Err(Error::WrongKey)));
        assert!(
            !std::fs::read(&keep)
                .unwrap()
                .windows(6)
                .any(|w| w == b"Asthma")
        );

        // What the backup holds, read without touching it.
        let sum = inspect(&keep, &KEY, dir.path()).unwrap();
        assert_eq!(sum.people, ["Asha Test"]);
        assert_eq!((sum.records, sum.trashed), (1, 0));
        assert!(matches!(
            inspect(&keep, &[9; 32], dir.path()),
            Err(Error::WrongKey)
        ));

        // A backup from another device (its own key) becomes one locked with ours.
        s.delete(&p, day).unwrap();
        let dest = dir.path().join("restored.db");
        let other = [3; 32];
        let foreign = dir.path().join("foreign.db");
        s.export(&foreign, &other).unwrap();
        restore_copy(&keep, &KEY, &dest, &other).unwrap();
        let r = Store::open(&dest, &other).unwrap();
        assert_eq!(r.timeline(&p).unwrap().len(), 1, "the lost record is back");
        assert_eq!(
            r.setting::<String>("backup.folder").unwrap().as_deref(),
            Some("/somewhere")
        );
        assert!(Store::open(&foreign, &KEY).is_err());
        let back = dir.path().join("back.db");
        assert_eq!(
            restore_copy(&foreign, &other, &back, &KEY).unwrap().trashed,
            2
        );
        assert!(Store::open(&back, &KEY).is_ok());

        // A key that does not open the backup leaves nothing behind.
        let bad = dir.path().join("bad.db");
        assert!(restore_copy(&keep, &[9; 32], &bad, &KEY).is_err());
        assert!(!bad.exists());
    }

    #[test]
    fn pre_update_backups_are_kept_apart() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(&dir.path().join("swasthya.db"), &KEY).unwrap();
        let backups_dir = dir.path().join("backups");
        s.daily_backup(&backups_dir, &KEY, at(NOW)).unwrap();
        for m in 0..5 {
            s.pre_update_backup(
                &backups_dir,
                &KEY,
                "0.1.0",
                at(NOW) + SignedDuration::from_mins(m),
                3,
            )
            .unwrap();
        }
        let mut names: Vec<String> = std::fs::read_dir(&backups_dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names.len(), 4, "{names:?}");
        assert!(names[0].starts_with("pre-update-2026-11-04-090200-0.1.0"));
        assert!(names[3].starts_with("swasthya-"));
        assert_eq!(backups(&backups_dir).unwrap().len(), 1);
        assert!(
            s.pre_update_backup(&backups_dir, &KEY, "../x", at(NOW), 3)
                .is_err()
        );
    }

    #[test]
    fn wrong_key_and_newer_schema_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("swasthya.db");
        drop(Store::open(&db, &KEY).unwrap());
        assert!(matches!(Store::open(&db, &[1; 32]), Err(Error::WrongKey)));
        let s = Store::open(&db, &KEY).unwrap();
        s.conn.pragma_update(None, "user_version", 99).unwrap();
        drop(s);
        assert!(matches!(Store::open(&db, &KEY), Err(Error::TooNew(99))));
    }
}
