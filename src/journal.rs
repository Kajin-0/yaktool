use crate::{
    error::{Error, Result},
    execute::ExecutionResult,
    plan::Plan,
};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::{
    fs::{File, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

pub struct Journal {
    connection: Connection,
    _lock: Option<File>,
    directory_identity: crate::filesystem::Identity,
    database_identity: crate::filesystem::Identity,
}
pub struct Record {
    pub plan: Plan,
    pub status: String,
    pub result: Option<ExecutionResult>,
    pub reversible: bool,
}
impl Journal {
    /// Parent must already exist. No database belongs in the repository.
    pub fn open(path: &Path) -> Result<Self> {
        let directory = File::from(rustix::fs::openat2(
            rustix::fs::CWD,
            path.parent()
                .ok_or_else(|| Error::new("DATABASE_ERROR", "Missing journal parent"))?,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
            rustix::fs::ResolveFlags::NO_SYMLINKS,
        )?);
        let parent_metadata = directory.metadata()?;
        if parent_metadata.uid() != rustix::process::geteuid().as_raw()
            || parent_metadata.mode() & 0o022 != 0
        {
            return Err(Error::new(
                "DATABASE_ERROR",
                "Journal directory must be owned by this user and not writable by others",
            ));
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path.with_extension("lock"))?;
        let lock_metadata = lock.metadata()?;
        if !lock_metadata.is_file()
            || lock_metadata.nlink() != 1
            || lock_metadata.uid() != rustix::process::geteuid().as_raw()
            || lock_metadata.mode() & 0o077 != 0
        {
            return Err(Error::new(
                "DATABASE_ERROR",
                "Journal lock must be a private regular file owned by this user",
            ));
        }
        rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive).map_err(
            |e| {
                Error::new(
                    "DATABASE_ERROR",
                    format!("Another YakTool command holds the journal lock: {e}"),
                )
            },
        )?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)?;
        let m = file.metadata()?;
        if !m.is_file()
            || m.nlink() != 1
            || m.uid() != rustix::process::geteuid().as_raw()
            || m.mode() & 0o077 != 0
        {
            return Err(Error::new(
                "DATABASE_ERROR",
                "Journal must be a private, singly linked regular file owned by this user",
            ));
        }
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        connection.execute_batch(
            "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS transactions (
                id TEXT PRIMARY KEY, created_at TEXT NOT NULL, confirmed_at TEXT,
                action TEXT NOT NULL, plan_hash TEXT NOT NULL, plan_json TEXT NOT NULL,
                result_json TEXT, status TEXT NOT NULL, reversible INTEGER NOT NULL DEFAULT 0,
                undo_of TEXT
            );",
        )?;
        directory.sync_all()?;
        Ok(Self {
            connection,
            _lock: Some(lock),
            directory_identity: crate::filesystem::Snapshot::from_meta(&parent_metadata).identity(),
            database_identity: crate::filesystem::Snapshot::from_meta(&m).identity(),
        })
    }
    pub fn read_only(path: &Path) -> Result<Self> {
        let file = File::from(rustix::fs::openat2(
            rustix::fs::CWD,
            path,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
            rustix::fs::ResolveFlags::NO_SYMLINKS,
        )?);
        let parent = File::from(rustix::fs::openat2(
            rustix::fs::CWD,
            path.parent()
                .ok_or_else(|| Error::new("DATABASE_ERROR", "Missing journal parent"))?,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
            rustix::fs::ResolveFlags::NO_SYMLINKS,
        )?);
        Ok(Self {
            connection: Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
            )?,
            _lock: None,
            directory_identity: crate::filesystem::Snapshot::from_meta(&parent.metadata()?)
                .identity(),
            database_identity: crate::filesystem::Snapshot::from_meta(&file.metadata()?).identity(),
        })
    }
    pub fn store(&self, plan: &Plan) -> Result<()> {
        for op in &plan.data().operations {
            if op.source_parent == self.directory_identity
                || op.destination_parent == self.directory_identity
                || op.source_snapshot.identity() == self.database_identity
            {
                return Err(Error::new(
                    "JOURNAL_PROTECTED",
                    "Journal files and its directory cannot participate in a move",
                ));
            }
        }
        self.connection.execute("INSERT INTO transactions(id,created_at,action,plan_hash,plan_json,status,undo_of) VALUES(?1,?2,?3,?4,?5,'planned',?6)", params![plan.data().plan_id, plan.data().created_at, plan.data().action, plan.hash(), plan.json()?, plan.data().undo_of])?;
        Ok(())
    }
    pub fn confirm(&self, plan: &Plan) -> Result<()> {
        let count = self.connection.execute("UPDATE transactions SET status='confirmed',confirmed_at=?1 WHERE id=?2 AND plan_hash=?3 AND plan_json=?4 AND status='planned'", params![chrono::Utc::now().to_rfc3339(), plan.data().plan_id, plan.hash(), plan.json()?])?;
        if count != 1 {
            return Err(Error::new(
                "PLAN_INVALIDATED",
                "Plan missing, changed, or already attempted",
            ));
        }
        Ok(())
    }
    pub fn save_result(&self, plan: &Plan, result: &ExecutionResult) -> Result<()> {
        let reversible = plan.data().undo_of.is_none()
            && result
                .operations
                .iter()
                .any(|o| o.state == crate::execute::OperationState::Verified);
        let count = self.connection.execute(
            "UPDATE transactions SET status=?1,result_json=?2,reversible=?3 WHERE id=?4",
            params![
                result.status,
                serde_json::to_string(result)?,
                reversible,
                plan.data().plan_id
            ],
        )?;
        if count != 1 {
            return Err(Error::new(
                "DATABASE_ERROR",
                "Transaction disappeared while recording execution",
            ));
        }
        Ok(())
    }
    pub fn get(&self, id: &str) -> Result<Record> {
        let (json, hash, status, result, reversible): (String, String, String, Option<String>, bool) = self.connection.query_row("SELECT plan_json,plan_hash,status,result_json,reversible FROM transactions WHERE id=?1", [id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
        Ok(Record {
            plan: Plan::stored(&json, &hash)?,
            status,
            result: result.map(|r| serde_json::from_str(&r)).transpose()?,
            reversible,
        })
    }
    pub fn recent(&self) -> Result<Vec<Record>> {
        let mut stmt = self
            .connection
            .prepare("SELECT id FROM transactions ORDER BY rowid DESC LIMIT 20")?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter().map(|id| self.get(id)).collect()
    }
    pub fn undo_candidate(&self) -> Result<Record> {
        // An interrupted or failed undo is never replayed or guessed at.
        let id: Option<String> = self.connection.query_row("SELECT id FROM transactions t WHERE reversible=1 AND status IN ('verified','partial') AND NOT EXISTS(SELECT 1 FROM transactions u WHERE u.undo_of=t.id AND u.status != 'planned') ORDER BY rowid DESC LIMIT 1", [], |r| r.get(0)).optional()?;
        self.get(
            &id.ok_or_else(|| Error::new("NOTHING_TO_UNDO", "No safely reversible transaction"))?,
        )
    }
    pub fn finish_undo(&mut self, original: &str, success: bool) -> Result<()> {
        self.connection.execute(
            "UPDATE transactions SET status=?1,reversible=0 WHERE id=?2",
            params![if success { "undone" } else { "undo_failed" }, original],
        )?;
        Ok(())
    }
    pub fn check_readonly(path: &Path) -> Result<String> {
        match rustix::fs::openat2(
            rustix::fs::CWD,
            path,
            rustix::fs::OFlags::PATH | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
            rustix::fs::ResolveFlags::NO_SYMLINKS,
        ) {
            Err(rustix::io::Errno::NOENT) => {
                return Ok("Not initialized (created on first mutating request)".into())
            }
            Err(e) => return Err(e.into()),
            Ok(_) => (),
        }
        let c = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        let status: String = c.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        Ok(status)
    }
}
