//! The native host's single SQLite writer. No device or network is opened here.
use focaus_contracts::{
    canonical_hash, error, Command, CommandResult, CommandResultStatus, ErrorCode, Event,
    SafeError, CONTRACT_VERSION,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub type Result<T> = std::result::Result<T, SafeError>;

pub fn id() -> String {
    let mut b = [0u8; 16];
    // Host identity allocation, never called on the audio callback.
    fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .expect("OS entropy unavailable");
    b[6] = (b[6] & 15) | 64;
    b[8] = (b[8] & 63) | 128;
    format!("{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}", b[0],b[1],b[2],b[3],b[4],b[5],b[6],b[7],b[8],b[9],b[10],b[11],b[12],b[13],b[14],b[15])
}

pub fn now() -> String {
    timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default(),
    )
}
pub fn wall_after(milliseconds: u64) -> String {
    timestamp(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            + Duration::from_millis(milliseconds),
    )
}
fn timestamp(duration: Duration) -> String {
    let seconds = duration.as_secs();
    let days = (seconds / 86400) as i64 + 719468;
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:09}Z",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60,
        duration.subsec_nanos()
    )
}

pub fn sql_error(e: rusqlite::Error) -> SafeError {
    use rusqlite::ErrorCode as S;
    let code = match e.sqlite_error_code() {
        Some(S::DiskFull) => ErrorCode::DiskFull,
        Some(S::DatabaseBusy | S::DatabaseLocked | S::ConstraintViolation) => ErrorCode::Conflict,
        Some(S::DatabaseCorrupt | S::NotADatabase) => ErrorCode::CorruptAsset,
        _ => ErrorCode::InvalidResult,
    };
    // SQLite messages may contain content/paths. Never return them over IPC.
    error(code, "Local storage operation failed")
}
pub fn io_error(e: std::io::Error) -> SafeError {
    error(
        if e.raw_os_error() == Some(28) {
            ErrorCode::DiskFull
        } else {
            ErrorCode::CorruptAsset
        },
        "Local resource operation failed",
    )
}
pub fn encode<T: Serialize>(x: &T) -> Result<String> {
    serde_json::to_string(x)
        .map_err(|_| error(ErrorCode::InvalidResult, "Cannot encode host value"))
}
pub fn decode<T: DeserializeOwned>(x: &str) -> Result<T> {
    serde_json::from_str(x).map_err(|_| error(ErrorCode::CorruptAsset, "Invalid stored host value"))
}
pub fn value<T: Serialize>(x: &T) -> Result<Value> {
    serde_json::to_value(x).map_err(|_| error(ErrorCode::InvalidResult, "Cannot encode host value"))
}

#[derive(Clone)]
pub struct Store {
    connection: Arc<Mutex<Connection>>,
    root: Arc<PathBuf>,
}
impl Store {
    /// Only C-owned migrations are executed. Existing unknown versions fail closed.
    pub fn open(root: impl AsRef<Path>, migration: &str) -> Result<Self> {
        fs::create_dir_all(root.as_ref()).map_err(io_error)?;
        let root = fs::canonicalize(root.as_ref()).map_err(io_error)?;
        fs::create_dir_all(root.join("resources")).map_err(io_error)?;
        fs::create_dir_all(root.join("capture")).map_err(io_error)?;
        let c = Connection::open(root.join("library.sqlite3")).map_err(sql_error)?;
        c.busy_timeout(Duration::from_secs(2)).map_err(sql_error)?;
        c.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )
        .map_err(sql_error)?;
        let version: u64 = c
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(sql_error)?;
        match version {
            0 => c.execute_batch(migration).map_err(sql_error)?,
            1 => (),
            _ => {
                return Err(error(
                    ErrorCode::UnsupportedVersion,
                    "Unsupported local schema",
                ))
            }
        }
        let fk: u64 = c
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .map_err(sql_error)?;
        if fk != 1 {
            return Err(error(
                ErrorCode::InvalidResult,
                "Foreign key enforcement required",
            ));
        }
        Ok(Self {
            connection: Arc::new(Mutex::new(c)),
            root: Arc::new(root),
        })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub(crate) fn read<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let c = self
            .connection
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "Storage unavailable"))?;
        f(&c)
    }
    pub(crate) fn write<T>(&self, f: impl FnOnce(&Transaction<'_>) -> Result<T>) -> Result<T> {
        let mut c = self
            .connection
            .lock()
            .map_err(|_| error(ErrorCode::InvalidResult, "Storage unavailable"))?;
        let t = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sql_error)?;
        let output = f(&t)?;
        t.commit().map_err(sql_error)?;
        Ok(output)
    }
    pub fn visibility_epoch(&self) -> Result<u64> {
        self.read(epoch)
    }
    pub fn head(&self, entity_id: &str) -> Result<Value> {
        self.read(|c| head(c, entity_id))
    }
    pub(crate) fn preflight(
        &self,
        actor_id: &str,
        command: &Command,
    ) -> Result<Option<CommandResult>> {
        self.read(|c|{
            let name=value(&command.name)?.as_str().unwrap_or("").to_owned();
            let key=command.idempotency_key.as_deref().filter(|s|!s.is_empty()).ok_or_else(||error(ErrorCode::InvalidArgument,"Idempotency key required"))?;
            let request_hash=canonical_hash(&json!({"name":name,"payload":command.payload,"expected_revisions":command.expected_revisions,"window_id":command.window_id,"version":command.contract_version}));
            let prior:Option<(String,String)>=c.query_row("SELECT request_hash,response_json FROM idempotency WHERE actor_id=?1 AND command_name=?2 AND idempotency_key=?3",params![actor_id,name,key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql_error)?;
            if let Some((hash,response))=prior {if hash!=request_hash{return Err(error(ErrorCode::IdempotencyMismatch,"Intent key reused with different input"));}let mut response:CommandResult=decode(&response)?;response.command_id=command.command_id.clone();return Ok(Some(response));}
            if let Some(revisions)=&command.expected_revisions {for (id,rev) in revisions {check_command_revision(c,command,id,rev)?;}}
            Ok(None)
        })
    }
    /// Both the intent receipt and all business changes commit in this transaction.
    /// Caller identity comes from the authenticated channel, not command.actor.
    pub(crate) fn command(
        &self,
        actor_id: &str,
        command: &Command,
        f: impl FnOnce(&Transaction<'_>) -> Result<Value>,
    ) -> Result<CommandResult> {
        let name = value(&command.name)?.as_str().unwrap_or("").to_owned();
        let key = command
            .idempotency_key
            .as_deref()
            .filter(|k| !k.is_empty())
            .ok_or_else(|| error(ErrorCode::InvalidArgument, "Idempotency key required"))?;
        let request_hash = canonical_hash(
            &json!({"name":name,"payload":command.payload,"expected_revisions":command.expected_revisions,"window_id":command.window_id,"version":command.contract_version}),
        );
        self.write(|t| {
            let prior: Option<(String,String)> = t.query_row("SELECT request_hash,response_json FROM idempotency WHERE actor_id=?1 AND command_name=?2 AND idempotency_key=?3",params![actor_id,name,key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql_error)?;
            if let Some((hash,response))=prior {
                if hash!=request_hash { return Err(error(ErrorCode::IdempotencyMismatch,"Intent key reused with different input")); }
                let mut result: CommandResult=decode(&response)?;
                result.command_id=command.command_id.clone();
                return Ok(result);
            }
            if let Some(revisions)=&command.expected_revisions { for (entity,revision) in revisions { check_command_revision(t,command,entity,revision)?; } }
            let before=sequence(t)?;
            let result = f(t)?;
            let mut revisions=BTreeMap::new();
            let mut q=t.prepare("SELECT entity_id,revision_id FROM outbox WHERE global_seq>?1").map_err(sql_error)?;
            for row in q.query_map([before],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(sql_error)? { let (id,rev)=row.map_err(sql_error)?; revisions.insert(id,rev); }
            let response=CommandResult {contract_version:CONTRACT_VERSION.into(),command_id:command.command_id.clone(),status:CommandResultStatus::Ok,result:Some(result),committed_revisions:Some(revisions),event_seq:Some(sequence(t)?),task_id:None,current_state:None,error:None};
            t.execute("INSERT INTO idempotency VALUES(?1,?2,?3,?4,?5)",params![actor_id,name,key,request_hash,encode(&response)?]).map_err(sql_error)?;
            Ok(response)
        })
    }
    /// Data and event watermark are read under the same connection gate.
    pub(crate) fn consistent_snapshot<T>(
        &self,
        f: impl FnOnce(&Connection, u64) -> Result<T>,
    ) -> Result<T> {
        self.read(|c| f(c, sequence(c)?))
    }
    pub fn flush(&self) -> Result<()> {
        self.read(|c| {
            c.execute_batch("PRAGMA wal_checkpoint(FULL)")
                .map_err(sql_error)
        })
    }
}

pub(crate) fn sequence(c: &Connection) -> Result<u64> {
    c.query_row("SELECT COALESCE(MAX(global_seq),0) FROM outbox", [], |r| {
        r.get(0)
    })
    .map_err(sql_error)
}
pub(crate) fn epoch(c: &Connection) -> Result<u64> {
    c.query_row(
        "SELECT visibility_epoch FROM visibility_barrier WHERE singleton=1",
        [],
        |r| r.get(0),
    )
    .map_err(sql_error)
}
pub(crate) fn lifecycle(c: &Connection, entity: &str) -> Result<String> {
    c.query_row(
        "SELECT lifecycle FROM entities WHERE entity_id=?1",
        [entity],
        |r| r.get(0),
    )
    .optional()
    .map_err(sql_error)?
    .ok_or_else(|| error(ErrorCode::NotFound, "Entity unavailable"))
}
pub(crate) fn check_revision(c: &Connection, entity: &str, revision: &str) -> Result<()> {
    let state: Option<String> = c
        .query_row(
            "SELECT lifecycle FROM entities WHERE entity_id=?1",
            [entity],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    if state.is_none() {
        let view: Option<String> = c
            .query_row(
                "SELECT view_revision FROM views WHERE view_id=?1",
                [entity],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql_error)?;
        return if view.as_deref() == Some(revision) {
            Ok(())
        } else {
            Err(error(
                ErrorCode::Conflict,
                "View revision changed or unavailable",
            ))
        };
    }
    if state.as_deref() == Some("deleted") {
        return Err(error(
            ErrorCode::SourceDeleted,
            "Source permanently deleted",
        ));
    }
    let current: Option<String> = c
        .query_row(
            "SELECT revision_id FROM entities WHERE entity_id=?1",
            [entity],
            |r| r.get(0),
        )
        .map_err(sql_error)?;
    if current.as_deref() != Some(revision) {
        return Err(error(ErrorCode::Conflict, "Entity revision changed"));
    }
    Ok(())
}
fn check_command_revision(
    c: &Connection,
    command: &Command,
    entity: &str,
    revision: &str,
) -> Result<()> {
    let name = value(&command.name)?.as_str().unwrap_or("").to_owned();
    if !["recording.pause", "recording.stop", "recording.resume"].contains(&name.as_str())
        || command.payload["recording_id"].as_str() != Some(entity)
    {
        return check_revision(c, entity, revision);
    }
    let current = head(c, entity)?;
    if current["revision_id"].as_str() == Some(revision) {
        return Ok(());
    }
    let old: Option<String> = c
        .query_row(
            "SELECT body_json FROM revisions WHERE entity_id=?1 AND revision_id=?2",
            params![entity, revision],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_error)?
        .flatten();
    let Some(old) = old else {
        return Err(error(
            ErrorCode::Conflict,
            "Recording control base is unavailable",
        ));
    };
    let mut old: Value = decode(&old)?;
    let mut current = current;
    for key in ["revision_id", "entity_seq", "durable_watermark_ms"] {
        if let Some(o) = old.as_object_mut() {
            o.remove(key);
        }
        if let Some(o) = current.as_object_mut() {
            o.remove(key);
        }
    }
    if old != current {
        return Err(error(
            ErrorCode::Conflict,
            "Recording control state or assignment changed",
        ));
    }
    Ok(())
}
pub(crate) fn head(c: &Connection, entity: &str) -> Result<Value> {
    if lifecycle(c, entity)? == "deleted" {
        return Err(error(
            ErrorCode::SourceDeleted,
            "Source permanently deleted",
        ));
    }
    let body:Option<String>=c.query_row("SELECT r.body_json FROM entities e JOIN revisions r ON e.revision_id=r.revision_id WHERE e.entity_id=?1",[entity],|r|r.get(0)).optional().map_err(sql_error)?.flatten();
    body.map(|s| decode(&s))
        .unwrap_or_else(|| Err(error(ErrorCode::SourceUnavailable, "Content unavailable")))
}
pub(crate) fn commit_entity(
    t: &Transaction<'_>,
    entity: &str,
    kind: &str,
    body: Value,
    event_type: &str,
    scope: Value,
    cause: Option<&str>,
    task: Option<&str>,
) -> Result<Value> {
    commit_version(t, entity, kind, &id(), body, event_type, scope, cause, task)
}
/// Host-confirmed immutable import versions preserve engine object/revision IDs.
/// Collision checks run before any head is advanced.
pub(crate) fn commit_version(
    t: &Transaction<'_>,
    entity: &str,
    kind: &str,
    revision: &str,
    mut body: Value,
    event_type: &str,
    scope: Value,
    cause: Option<&str>,
    task: Option<&str>,
) -> Result<Value> {
    let existing: Option<(String, String)> = t
        .query_row(
            "SELECT entity_id,content_hash FROM revisions WHERE revision_id=?1",
            [revision],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql_error)?;
    if let Some((owner, hash)) = existing {
        if owner == entity && hash == canonical_hash(&body) {
            return Ok(body);
        }
        return Err(error(
            ErrorCode::Conflict,
            "Immutable revision identity conflicts with stored content",
        ));
    }
    let old_kind: Option<String> = t
        .query_row(
            "SELECT kind FROM entities WHERE entity_id=?1",
            [entity],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    if old_kind.as_ref().is_some_and(|old| old != kind) {
        return Err(error(
            ErrorCode::Conflict,
            "Entity identity belongs to a different kind",
        ));
    }
    let current: Option<(Option<String>, u64, String)> = t
        .query_row(
            "SELECT revision_id,entity_seq,lifecycle FROM entities WHERE entity_id=?1",
            [entity],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql_error)?;
    if current.as_ref().is_some_and(|(_, _, l)| l == "deleted") {
        return Err(error(
            ErrorCode::SourceDeleted,
            "Entity permanently deleted",
        ));
    }
    let seq = current.as_ref().map(|x| x.1 + 1).unwrap_or(1);
    if let Some(o) = body.as_object_mut() {
        if o.contains_key("revision_id") {
            o.insert("revision_id".into(), json!(revision));
        }
        if o.contains_key("entity_seq") {
            o.insert("entity_seq".into(), json!(seq));
        }
    }
    if let Some(o) = body.as_object_mut() {
        if o.contains_key("body_revision_id") {
            o.insert("body_revision_id".into(), json!(revision));
        }
    }
    t.execute("INSERT INTO entities(entity_id,kind,revision_id,entity_seq) VALUES(?1,?2,?3,?4) ON CONFLICT(entity_id) DO UPDATE SET revision_id=excluded.revision_id,entity_seq=excluded.entity_seq",params![entity,kind,revision,seq]).map_err(sql_error)?;
    t.execute(
        "INSERT INTO revisions VALUES(?1,?2,?3,?4,?5)",
        params![
            revision,
            entity,
            canonical_hash(&body),
            encode(&body)?,
            now()
        ],
    )
    .map_err(sql_error)?;
    if let Some((Some(parent), _, _)) = current {
        t.execute(
            "INSERT INTO revision_parents VALUES(?1,?2)",
            params![revision, parent],
        )
        .map_err(sql_error)?;
    }
    emit(
        t,
        event_type,
        entity,
        seq,
        &revision,
        scope,
        json!({"entity_id":entity,"entity_kind":kind,"revision_id":revision,"recording_id":body.get("recording_id")}),
        cause,
        task,
    )?;
    // Search current-head reads invalidate immediately, before asynchronous projection.
    t.execute("UPDATE search_index_state SET dirty_ranges_json=json_insert(dirty_ranges_json,'$[#]',?1) WHERE scope_id=?1",[entity]).map_err(sql_error)?;
    Ok(body)
}
pub(crate) fn emit(
    t: &Transaction<'_>,
    event_type: &str,
    entity: &str,
    seq: u64,
    revision: &str,
    scope: Value,
    payload: Value,
    cause: Option<&str>,
    task: Option<&str>,
) -> Result<u64> {
    t.execute("INSERT INTO outbox(event_id,scope_json,event_type,entity_id,entity_seq,revision_id,cause_command_id,task_id,committed_at,payload_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![id(),encode(&scope)?,event_type,entity,seq,revision,cause,task,now(),encode(&payload)?]).map_err(sql_error)?;
    Ok(t.last_insert_rowid() as u64)
}
pub(crate) fn events(c: &Connection, after: u64, limit: u64) -> Result<(u64, Vec<Event>)> {
    let mut q=c.prepare("SELECT global_seq,event_id,scope_json,event_type,entity_id,entity_seq,revision_id,cause_command_id,task_id,committed_at,payload_json FROM outbox WHERE global_seq>?1 ORDER BY global_seq LIMIT ?2").map_err(sql_error)?;
    let rows = q
        .query_map(params![after, limit.min(1000)], |r| {
            Ok((
                r.get::<_, u64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, u64>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, String>(9)?,
                r.get::<_, String>(10)?,
            ))
        })
        .map_err(sql_error)?;
    let mut output = Vec::new();
    let mut through = after;
    for row in rows {
        let (
            global_seq,
            event_id,
            scope,
            event_type,
            entity_id,
            entity_seq,
            revision_id,
            cause_command_id,
            task_id,
            committed_at,
            payload,
        ) = row.map_err(sql_error)?;
        through = global_seq;
        output.push(Event {
            event_id,
            contract_version: CONTRACT_VERSION.into(),
            global_seq,
            scope: decode(&scope)?,
            event_type,
            entity_id,
            entity_seq,
            revision_id,
            cause_command_id,
            task_id,
            committed_at,
            payload: decode(&payload)?,
        });
    }
    Ok((through, output))
}
