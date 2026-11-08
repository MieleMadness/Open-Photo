//! Reviewed XMP transactions with handle-bound backup and no-overwrite publication.
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
// All SQLite, file-handle and write-lock work belongs inside this closure.
// Synchronous Tauri commands otherwise occupy the WebView event-loop thread;
// source hashing and a contended save lock can both take substantial time.
async fn xmp_task<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| {
            format!(
                "The XMP task stopped unexpectedly. Refresh saved changes before retrying: {error}"
            )
        })?
}
async fn xmp_write_task<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    xmp_task(move || {
        // Decisions must serialize with publication, too: cancellation cannot
        // slip between a new sidecar's journal commit and its publication.
        let _lock = crate::XMP_WRITE_LOCK.lock().map_err(|_| {
            "An earlier XMP save stopped unexpectedly. Restart Open-Photo, then review saved changes before retrying.".to_string()
        })?;
        work()
    }).await
}
fn schema(db: &Connection) -> Result<(), String> {
    db.execute_batch("PRAGMA synchronous=FULL;
      CREATE TABLE IF NOT EXISTS xmp_operations(
        id INTEGER PRIMARY KEY, workspace TEXT NOT NULL, root TEXT NOT NULL,
        path TEXT NOT NULL, original TEXT, candidate TEXT NOT NULL,
        original_hash TEXT, candidate_hash TEXT NOT NULL, identity TEXT,
        state TEXT NOT NULL, decision_hash TEXT);
      CREATE TABLE IF NOT EXISTS xmp_folder_bindings(workspace TEXT NOT NULL,root TEXT NOT NULL,identity TEXT NOT NULL,PRIMARY KEY(workspace,root));
      CREATE UNIQUE INDEX IF NOT EXISTS xmp_pending ON xmp_operations(workspace,path)
        WHERE state IN ('prepared','review_required');").map_err(|e| e.to_string())?;
    let columns = db
        .prepare("PRAGMA table_info(xmp_operations)")
        .map_err(|e| e.to_string())?
        .query_map([], |r| r.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    for column in [
        "source_identity",
        "root_identity",
        "backup_name",
        "source_hash",
    ] {
        if !columns.iter().any(|c| c == column) {
            db.execute_batch(&format!(
                "ALTER TABLE xmp_operations ADD COLUMN {column} TEXT"
            ))
            .map_err(|e| e.to_string())?;
        }
    }
    // An interrupted Apply must block another proposal for the same item.
    db.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS xmp_pending_v2 ON xmp_operations(workspace,path)
        WHERE state IN ('prepared','review_required','applying','backed_up','restoring');",
    )
    .map_err(|e| e.to_string())?;
    db.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS xmp_pending_v3 ON xmp_operations(workspace,path) WHERE state IN ('prepared','review_required','applying','backed_up','restoring');
      CREATE INDEX IF NOT EXISTS xmp_review_pages ON xmp_operations(workspace,id DESC);
      CREATE INDEX IF NOT EXISTS xmp_pending_pages ON xmp_operations(workspace,id DESC) WHERE state IN ('prepared','review_required','applying','backed_up','restoring');").map_err(|e|e.to_string())
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct Review {
    id: i64,
    path: String,
    state: String,
    original: Option<String>,
    candidate: String,
    original_hash: Option<String>,
    candidate_hash: String,
    identity: Option<String>,
    source_identity: Option<String>,
    root_identity: Option<String>,
    backup_name: Option<String>,
    source_hash: Option<String>,
    integrity_error: Option<String>,
    // Internal intent bindings; do not expand the frontend wire contract.
    #[serde(skip)]
    root: String,
    #[serde(skip)]
    decision_hash: Option<String>,
}

#[cfg(windows)]
struct Guard {
    _directories: Vec<fs::File>,
    _source: fs::File,
    sidecar: Option<fs::File>,
    identity: Option<String>,
    root_identity: String,
    source_identity: String,
    path: PathBuf,
}
#[cfg(windows)]
impl Guard {
    fn open(root: &str, path: &str) -> Result<Self, String> {
        Self::open_mode(root, path, false)
    }
    fn open_mode(root: &str, path: &str, for_apply: bool) -> Result<Self, String> {
        use std::os::windows::{
            fs::{MetadataExt, OpenOptionsExt},
            io::AsRawHandle,
        };
        use windows_sys::Win32::Storage::FileSystem::*;
        if [Path::new(root),Path::new(path)].iter().any(|p|p.components().any(|c|matches!(c,std::path::Component::Normal(name) if name.to_string_lossy().contains(':')))) {
            return Err("XMP refuses alternate data streams".into());
        }
        fn open(
            path: &Path,
            directory: bool,
            writable_parent: bool,
            for_apply: bool,
        ) -> Result<fs::File, String> {
            let file = fs::OpenOptions::new()
                .read(true)
                // Directory sharing permits child creation but excludes rename
                // and deletion. Publication targets this directory HANDLE;
                // it never resolves the parent by pathname at the final step.
                .access_mode(
                    0x80000000
                        | if writable_parent { FILE_ADD_FILE } else { 0 }
                        | if for_apply { 0x10000 } else { 0 },
                )
                .share_mode(FILE_SHARE_READ | if directory { FILE_SHARE_WRITE } else { 0 })
                .custom_flags(
                    FILE_FLAG_OPEN_REPARSE_POINT
                        | if directory {
                            FILE_FLAG_BACKUP_SEMANTICS
                        } else {
                            0
                        },
                )
                .open(path)
                .map_err(|e| format!("Cannot protect {}: {e}", path.display()))?;
            let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
            // SAFETY: owned live handle, API initializes info on success.
            if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            // SAFETY: checked successful initialization above.
            let info = unsafe { info.assume_init() };
            if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
                || (!directory
                    && (info.nNumberOfLinks != 1
                        || info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0))
            {
                return Err("XMP writes refuse links, reparse points and file aliases".into());
            }
            Ok(file)
        }
        fn directories(
            path: &Path,
            handles: &mut Vec<fs::File>,
            seen: &mut std::collections::HashSet<PathBuf>,
            parent: &Path,
        ) -> Result<(), String> {
            if !path.is_absolute()
                || path.components().any(|c| {
                    matches!(
                        c,
                        std::path::Component::ParentDir | std::path::Component::CurDir
                    )
                })
            {
                return Err("Use an absolute folder path without traversal".into());
            }
            let mut chain = path.ancestors().collect::<Vec<_>>();
            chain.reverse();
            for dir in chain {
                if fs::symlink_metadata(dir)
                    .map_err(|e| e.to_string())?
                    .file_attributes()
                    & FILE_ATTRIBUTE_REPARSE_POINT
                    != 0
                {
                    return Err("XMP writes refuse directory reparse points".into());
                }
                let canonical = fs::canonicalize(dir).map_err(|e| e.to_string())?;
                if seen.insert(canonical.clone()) {
                    handles.push(open(dir, true, canonical == parent, false)?);
                }
            }
            Ok(())
        }
        let mut handles = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let parent = Path::new(path).parent().ok_or("Missing media parent")?;
        let canonical_parent = fs::canonicalize(parent).map_err(|e| e.to_string())?;
        directories(Path::new(root), &mut handles, &mut seen, &canonical_parent)?;
        let root_identity = identity(handles.last().ok_or("Missing root handle")?)?;
        let root_path = fs::canonicalize(root).map_err(|e| e.to_string())?;
        // Only fixed local drives are accepted; network and removable storage
        // need independent sharing/durability acceptance before enabling writes.
        let prefix = root_path.components().next().ok_or("Missing drive")?;
        let drive = match prefix {
            std::path::Component::Prefix(p) => match p.kind() {
                std::path::Prefix::Disk(d) | std::path::Prefix::VerbatimDisk(d) => d,
                _ => return Err("XMP writes require a local fixed NTFS drive".into()),
            },
            _ => return Err("Missing drive".into()),
        };
        let drive_path = [u16::from(drive), 58, 92, 0];
        // SAFETY: NUL-terminated drive path.
        if unsafe { GetDriveTypeW(drive_path.as_ptr()) } != 3 {
            return Err("XMP writes require a local fixed NTFS drive".into());
        }
        let mut filesystem = [0u16; 32];
        // SAFETY: all optional buffers are null; filesystem buffer is writable.
        let ok = unsafe {
            GetVolumeInformationByHandleW(
                handles.last().unwrap().as_raw_handle(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                filesystem.as_mut_ptr(),
                32,
            )
        };
        if ok == 0
            || String::from_utf16_lossy(
                &filesystem[..filesystem.iter().position(|c| *c == 0).unwrap_or(32)],
            ) != "NTFS"
        {
            return Err("XMP writes require verified NTFS semantics".into());
        }
        let parent = Path::new(path).parent().ok_or("Missing media parent")?;
        directories(parent, &mut handles, &mut seen, &canonical_parent)?;
        let source = open(Path::new(path), false, false, false)?;
        let source_identity = identity(&source)?;
        let media = crate::safe_media_path(root, path)?;
        if !media.starts_with(&root_path) {
            return Err("Media left the registered folder".into());
        }
        let sidecar_path = media.with_file_name(format!(
            "{}.xmp",
            media
                .file_name()
                .ok_or("Missing media name")?
                .to_string_lossy()
        ));
        let sidecar = match fs::symlink_metadata(&sidecar_path) {
            Ok(_) => Some(open(&sidecar_path, false, false, for_apply)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        let identity = sidecar.as_ref().map(identity).transpose()?;
        Ok(Self {
            _directories: handles,
            _source: source,
            sidecar,
            identity,
            root_identity,
            source_identity,
            path: sidecar_path,
        })
    }
    fn current(&mut self) -> Result<Option<String>, String> {
        use std::io::{Seek, SeekFrom};
        let Some(file) = self.sidecar.as_mut() else {
            return Ok(None);
        };
        if file.metadata().map_err(|e| e.to_string())?.len() > crate::MAX_XMP_SIDECAR_BYTES {
            return Err("XMP sidecar is too large".into());
        }
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut bytes = String::new();
        file.take(crate::MAX_XMP_SIDECAR_BYTES + 1)
            .read_to_string(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > crate::MAX_XMP_SIDECAR_BYTES {
            return Err("XMP sidecar is too large".into());
        }
        Ok(Some(bytes))
    }
    fn source_hash(&mut self) -> Result<String, String> {
        use std::io::{Seek, SeekFrom};
        // Hash the entire source through the retained read-only handle. Sharing
        // excludes ordinary writers/replacements until publication completes.
        // Bounded memory is important for large videos; never load media whole.
        let file = &mut self._source;
        let length = file.metadata().map_err(|e| e.to_string())?.len();
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut reader = file.take(length.saturating_add(1));
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        let mut read = 0u64;
        loop {
            let count = reader
                .read(&mut buffer)
                .map_err(|e| format!("Cannot verify original media: {e}"))?;
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
            read += count as u64;
        }
        if read != length {
            return Err("Original media length changed during verification; review again".into());
        }
        Ok(format!("{:x}", digest.finalize()))
    }
}
#[cfg(windows)]
fn identity(file: &fs::File) -> Result<String, String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: file is live; success initializes the supplied buffer.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    // SAFETY: initialization checked above.
    let info = unsafe { info.assume_init() };
    if info.dwVolumeSerialNumber == 0 || info.nFileIndexHigh == 0 && info.nFileIndexLow == 0 {
        return Err("File identity unavailable".into());
    }
    Ok(format!(
        "{}:{}:{}",
        info.dwVolumeSerialNumber, info.nFileIndexHigh, info.nFileIndexLow
    ))
}
#[cfg(not(windows))]
struct Guard {
    path: PathBuf,
    identity: Option<String>,
    root_identity: String,
    source_identity: String,
}
#[cfg(not(windows))]
impl Guard {
    fn open(_: &str, _: &str) -> Result<Self, String> {
        Err("XMP writes require verified Windows NTFS protection".into())
    }
    fn current(&mut self) -> Result<Option<String>, String> {
        Err("Unsupported filesystem".into())
    }
    fn open_mode(root: &str, path: &str, _: bool) -> Result<Self, String> {
        Self::open(root, path)
    }
    fn source_hash(&mut self) -> Result<String, String> {
        Err("Unsupported filesystem".into())
    }
}
fn authorize(db: &Connection, workspace: &str, root: &str) -> Result<(), String> {
    let roots = crate::user_data::workspace_roots_from(db, workspace)?;
    if !roots.iter().any(|r| r == root) {
        return Err("Folder access was removed from this workspace".into());
    }
    Ok(())
}
fn bind_root(db: &Connection, workspace: &str, root: &str, guard: &Guard) -> Result<(), String> {
    db.execute(
        "INSERT OR IGNORE INTO xmp_folder_bindings(workspace,root,identity) VALUES(?1,?2,?3)",
        params![workspace, root, guard.root_identity],
    )
    .map_err(|e| e.to_string())?;
    let expected: String = db
        .query_row(
            "SELECT identity FROM xmp_folder_bindings WHERE workspace=?1 AND root=?2",
            params![workspace, root],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if expected != guard.root_identity {
        return Err("The registered folder identity changed; XMP writes are blocked".into());
    }
    Ok(())
}
fn record(
    db: &Connection,
    workspace: &str,
    root: &str,
    path: &str,
    original: Option<&str>,
    candidate: &str,
    identity: Option<&str>,
) -> Result<i64, String> {
    db.execute("INSERT INTO xmp_operations(workspace,root,path,original,candidate,original_hash,candidate_hash,identity,state) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![workspace,root,path,original,candidate,original.map(|s|hash(s.as_bytes())),hash(candidate.as_bytes()),identity,if original.is_some(){"review_required"}else{"prepared"}]).map_err(|e|format!("A pending XMP review may already exist; review it before retrying: {e}"))?;
    Ok(db.last_insert_rowid())
}
fn review_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Review> {
    let mut row = Review {
        id: r.get(0)?,
        path: r.get(1)?,
        state: r.get(2)?,
        original: r.get(3)?,
        candidate: r.get(4)?,
        original_hash: r.get(5)?,
        candidate_hash: r.get(6)?,
        identity: r.get(7)?,
        source_identity: r.get(8)?,
        root_identity: r.get(9)?,
        backup_name: r.get(10)?,
        source_hash: r.get(11)?,
        integrity_error: None,
        root: r.get(12)?,
        decision_hash: r.get(13)?,
    };
    if row.candidate_hash != hash(row.candidate.as_bytes())
        || row.original_hash != row.original.as_ref().map(|s| hash(s.as_bytes()))
    {
        row.integrity_error=Some("Recovery snapshot failed its integrity check. It has been preserved; applying is blocked.".into());
    }
    Ok(row)
}
const REVIEW_COLUMNS:&str="id,path,state,original,candidate,original_hash,candidate_hash,identity,source_identity,root_identity,backup_name,source_hash,root,decision_hash";
fn load_review(db: &Connection, workspace: &str, id: i64) -> Result<Review, String> {
    db.query_row(
        &format!("SELECT {REVIEW_COLUMNS} FROM xmp_operations WHERE workspace=?1 AND id=?2"),
        params![workspace, id],
        review_row,
    )
    .map_err(|e| e.to_string())
}
fn list(db: &Connection, workspace: &str) -> Result<Vec<Review>, String> {
    list_history(db, workspace, false)
}
fn list_history(db: &Connection, workspace: &str, history: bool) -> Result<Vec<Review>, String> {
    let sql=format!("SELECT {REVIEW_COLUMNS} FROM xmp_operations WHERE workspace=?1 {} ORDER BY id DESC LIMIT 1000",if history{""}else{"AND state IN ('prepared','review_required','applying','backed_up','restoring')"});
    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([workspace], review_row)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub(crate) async fn xmp_reviews(
    workspace: String,
    history: Option<bool>,
) -> Result<Vec<Review>, String> {
    xmp_task(move || {
        let db = crate::user_data::database()?;
        schema(&db)?;
        // Journal ownership is scoped to a still-saved workspace; root availability
        // is deliberately not required to inspect/cancel its stored proposals.
        crate::user_data::workspace_roots_from(&db, &workspace)?;
        if history.unwrap_or(false) {
            list_history(&db, &workspace, true)
        } else {
            list(&db, &workspace)
        }
    })
    .await
}
const REVIEW_PAGE_SIZE: usize = 50;
#[derive(Serialize)]
pub(crate) struct ReviewSummary {
    id: i64,
    path: String,
    state: String,
    candidate_hash: String,
}
#[derive(Serialize)]
pub(crate) struct ReviewPage {
    items: Vec<ReviewSummary>,
    next_before: Option<i64>,
}
fn review_page_on(
    db: &Connection,
    workspace: &str,
    history: bool,
    before: Option<i64>,
) -> Result<ReviewPage, String> {
    crate::user_data::workspace_roots_from(db, workspace)?;
    if before.is_some_and(|id| id <= 0) {
        return Err("Invalid review page; refresh saved changes".into());
    }
    // Select metadata only. XML snapshots are loaded for one selected record,
    // never transported/hashed for every row of a large journal.
    let sql = format!("SELECT id,path,state,candidate_hash FROM xmp_operations WHERE workspace=?1 {} {} ORDER BY id DESC LIMIT {}",
        if history { "" } else { "AND state IN ('prepared','review_required','applying','backed_up','restoring')" },
        if before.is_some() { "AND id < ?2" } else { "" }, REVIEW_PAGE_SIZE + 1);
    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let mut rows = if let Some(id) = before {
        stmt.query(params![workspace, id])
    } else {
        stmt.query([workspace])
    }
    .map_err(|e| e.to_string())?;
    let mut items = Vec::with_capacity(REVIEW_PAGE_SIZE + 1);
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        items.push(ReviewSummary {
            id: row.get(0).map_err(|e| e.to_string())?,
            path: row.get(1).map_err(|e| e.to_string())?,
            state: row.get(2).map_err(|e| e.to_string())?,
            candidate_hash: row.get(3).map_err(|e| e.to_string())?,
        });
    }
    let has_more = items.len() > REVIEW_PAGE_SIZE;
    items.truncate(REVIEW_PAGE_SIZE);
    let next_before = if has_more {
        items.last().map(|row| row.id)
    } else {
        None
    };
    Ok(ReviewPage { items, next_before })
}
fn review_record_on(db: &Connection, workspace: &str, id: i64) -> Result<Review, String> {
    // Stored versions remain inspectable even if roots/media are unavailable.
    crate::user_data::workspace_roots_from(db, workspace)?;
    load_review(db, workspace, id)
}
#[tauri::command]
pub(crate) async fn xmp_review_page(
    workspace: String,
    history: Option<bool>,
    before: Option<i64>,
) -> Result<ReviewPage, String> {
    xmp_task(move || {
        let db = crate::user_data::database()?;
        schema(&db)?;
        review_page_on(&db, &workspace, history.unwrap_or(false), before)
    })
    .await
}
#[tauri::command]
pub(crate) async fn xmp_review_record(workspace: String, id: i64) -> Result<Review, String> {
    xmp_task(move || {
        let db = crate::user_data::database()?;
        schema(&db)?;
        review_record_on(&db, &workspace, id)
    })
    .await
}
#[derive(Serialize)]
pub(crate) struct Snapshot {
    current: Option<String>,
    fingerprint: String,
    can_apply: bool,
    reason: Option<String>,
    observation: String,
    backup_name: Option<String>,
    candidate_hash: Option<String>,
    operation_state: String,
    can_restore: bool,
    recovery_original: Option<RecoveryOriginal>,
    staging: Option<crate::xmp_staging_inventory::StagingInventory>,
}
fn snapshot(guard: &mut Guard) -> Result<Snapshot, String> {
    let current = guard.current()?;
    let fingerprint = hash(
        serde_json::to_string(&(&guard.identity, &current))
            .map_err(|e| e.to_string())?
            .as_bytes(),
    );
    Ok(Snapshot {
        current,
        fingerprint,
        can_apply: false,
        reason: None,
        observation: "observed".into(),
        backup_name: None,
        candidate_hash: None,
        operation_state: String::new(),
        can_restore: false,
        recovery_original: None,
        staging: None,
    })
}
fn bound_snapshot(workspace: &str, row: &Review, guard: &mut Guard) -> Result<Snapshot, String> {
    let mut out = snapshot(guard)?;
    let source_hash = guard.source_hash()?;
    out.operation_state = row.state.clone();
    if row.backup_name.is_some() {
        out.recovery_original = Some(recovery_original(row, guard, false).0);
    }
    out.fingerprint = hash(
        serde_json::to_string(&(
            workspace,
            row.id,
            &row.state,
            &row.candidate_hash,
            &row.original_hash,
            &row.source_identity,
            &row.root_identity,
            &row.identity,
            &row.backup_name,
            &guard.source_identity,
            &guard.root_identity,
            &out.fingerprint,
            &out.recovery_original,
            (&row.source_hash, &source_hash),
        ))
        .map_err(|e| e.to_string())?
        .as_bytes(),
    );
    let current_hash = out.current.as_ref().map(|s| hash(s.as_bytes()));
    out.observation = match current_hash.as_deref() {
        None => "missing",
        Some(h) if h == row.candidate_hash => "matches_saved",
        Some(_) => "changed",
    }
    .into();
    out.backup_name = row.backup_name.clone();
    out.candidate_hash = Some(row.candidate_hash.clone());
    out.reason = if row.integrity_error.is_some() {
        row.integrity_error.clone()
    } else if row.source_identity.as_deref() != Some(&guard.source_identity)
        || row.root_identity.as_deref() != Some(&guard.root_identity)
    {
        Some("The source photo or folder identity changed, or this older proposal lacks identity bindings. Cancel and create a fresh proposal.".into())
    } else if row.source_hash.as_deref() != Some(&source_hash) {
        Some(if matches!(row.state.as_str(), "prepared" | "review_required") {
            "The original media contents changed, or this older proposal lacks a source checksum. Cancel and create a fresh proposal."
        } else {
            "The original media contents changed, or this older record lacks a source checksum. Restoration is blocked; preserve the saved versions for manual inspection."
        }.into())
    } else if !matches!(row.state.as_str(), "prepared" | "review_required") {
        Some("This record is history or an interrupted Apply. Inspect the saved versions; no automatic restore is performed.".into())
    } else if row.original_hash != current_hash || row.identity != guard.identity {
        Some("The sidecar changed after this proposal was created. Keep its current version or cancel and create a fresh proposal.".into())
    } else {
        None
    };
    out.can_apply = out.reason.is_none();
    if out.can_apply && legacy_pending(&guard.path)? {
        out.reason=Some("Older temporary or backup files require manual review. They have been preserved; applying is blocked.".into());
        out.can_apply = false;
    }
    out.can_restore = matches!(row.state.as_str(), "applying" | "backed_up" | "restoring")
        && out.current.is_none()
        && row.integrity_error.is_none()
        && row.source_identity.as_deref() == Some(&guard.source_identity)
        && row.source_hash.as_deref() == Some(&source_hash)
        && row.root_identity.as_deref() == Some(&guard.root_identity)
        && out
            .recovery_original
            .as_ref()
            .is_some_and(|r| r.status == "verified")
        && !legacy_pending(&guard.path)?;
    Ok(out)
}
fn legacy_pending(path: &Path) -> Result<bool, String> {
    let name = path
        .file_name()
        .ok_or("Missing sidecar name")?
        .to_string_lossy();
    Ok(crate::xmp_recovery_inventory::inspect_recovery_pair(
        &path.with_file_name(format!("{name}.openphoto.tmp")),
        &path.with_file_name(format!("{name}.openphoto.bak")),
    )?
    .requires_review())
}
fn location(db: &Connection, workspace: &str, id: i64) -> Result<(String, String), String> {
    db.query_row(
        "SELECT root,path FROM xmp_operations WHERE workspace=?1 AND id=?2",
        params![workspace, id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .map_err(|e| e.to_string())
}
fn review_snapshot_on(db: &mut Connection, workspace: &str, id: i64) -> Result<Snapshot, String> {
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let row = load_review(&tx, workspace, id)?;
    let (root, path) = location(&tx, workspace, id)?;
    authorize(&tx, workspace, &root)?;
    match Guard::open(&root, &path) {
        Ok(mut guard) => {
            bind_root(&tx, workspace, &root, &guard)?;
            let mut observed = bound_snapshot(workspace, &row, &mut guard)?;
            // Advisory only: not part of approval fingerprints or write checks.
            observed.staging = Some(crate::xmp_staging_inventory::inspect(&guard.path, row.id));
            Ok(observed)
        }
        Err(error) => Ok(Snapshot {
            current: None,
            fingerprint: String::new(),
            can_apply: false,
            reason: Some(error),
            observation: "inaccessible".into(),
            backup_name: row.backup_name,
            candidate_hash: Some(row.candidate_hash),
            operation_state: row.state,
            can_restore: false,
            recovery_original: None,
            staging: None,
        }),
    }
}
#[tauri::command]
pub(crate) async fn xmp_review_snapshot(workspace: String, id: i64) -> Result<Snapshot, String> {
    xmp_task(move || {
        let mut db = crate::user_data::database()?;
        schema(&db)?;
        review_snapshot_on(&mut db, &workspace, id)
    })
    .await
}
fn decide(
    db: &Connection,
    workspace: &str,
    id: i64,
    decision: &str,
    fingerprint: &str,
) -> Result<(), String> {
    if !matches!(decision, "kept_external" | "cancelled") {
        return Err("Invalid recovery decision".into());
    }
    let (state, old): (String, Option<String>) = db
        .query_row(
            "SELECT state,decision_hash FROM xmp_operations WHERE workspace=?1 AND id=?2",
            params![workspace, id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    if state == decision && old.as_deref() == Some(fingerprint) {
        return Ok(());
    }
    if !matches!(state.as_str(), "prepared" | "review_required")
        && !(decision == "kept_external"
            && matches!(state.as_str(), "applying" | "backed_up" | "restoring"))
    {
        return Err("This operation is completed or interrupted. Use saved history; recovery files have not been changed.".into());
    }
    db.execute(
        "UPDATE xmp_operations SET state=?1,decision_hash=?2 WHERE workspace=?3 AND id=?4",
        params![decision, fingerprint, workspace, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn review_decide_on(
    db: &mut Connection,
    workspace: &str,
    id: i64,
    decision: &str,
    fingerprint: &str,
) -> Result<(), String> {
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    crate::user_data::workspace_roots_from(&tx, workspace)?;
    let row = load_review(&tx, workspace, id)?;
    let recorded: Option<String> = tx
        .query_row(
            "SELECT decision_hash FROM xmp_operations WHERE workspace=?1 AND id=?2",
            params![workspace, id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if row.state == decision && recorded.as_deref() == Some(fingerprint) {
        return tx.commit().map_err(|e| e.to_string());
    }
    if decision == "cancelled" {
        // A journal-only cancellation never opens or mutates filesystem paths.
        // Bind to the candidate the user selected, including corrupt records.
        if fingerprint != row.candidate_hash {
            return Err("The proposal changed; refresh before cancelling".into());
        }
        decide(&tx, workspace, id, decision, fingerprint)?;
    } else {
        let (root, path) = location(&tx, workspace, id)?;
        authorize(&tx, workspace, &root)?;
        let mut guard = Guard::open(&root, &path)?;
        bind_root(&tx, workspace, &root, &guard)?;
        let current = bound_snapshot(workspace, &row, &mut guard)?;
        if matches!(row.state.as_str(), "applying" | "backed_up" | "restoring")
            && current.current.is_none()
        {
            return Err("The sidecar is missing. Inspect the recorded recovery original; no version has been restored automatically.".into());
        }
        if current.fingerprint != fingerprint {
            return Err("The sidecar or proposal changed; review the current version again".into());
        }
        decide(&tx, workspace, id, decision, fingerprint)?;
    }
    tx.commit().map_err(|e| e.to_string())
}
#[tauri::command]
pub(crate) async fn xmp_review_decide(
    workspace: String,
    id: i64,
    decision: String,
    fingerprint: String,
) -> Result<(), String> {
    xmp_write_task(move || {
        let mut db = crate::user_data::database()?;
        schema(&db)?;
        review_decide_on(&mut db, &workspace, id, &decision, &fingerprint)
    })
    .await
}

#[derive(Serialize)]
pub(crate) struct Batch {
    created: usize,
    updated: usize,
    skipped: usize,
    review: usize,
    errors: Vec<String>,
}
#[tauri::command]
pub(crate) async fn bulk_create_tags(
    workspace: String,
    root: String,
    paths: Vec<String>,
    tags: Vec<String>,
) -> Result<Batch, String> {
    xmp_write_task(move || {
        let mut db = crate::user_data::database()?;
        schema(&db)?;
        batch(&mut db, workspace, root, paths, tags)
    })
    .await
}
fn batch(
    db: &mut Connection,
    workspace: String,
    root: String,
    paths: Vec<String>,
    tags: Vec<String>,
) -> Result<Batch, String> {
    batch_with_checkpoint(db, workspace, root, paths, tags, |_, _| Ok(()))
}
fn batch_with_checkpoint(
    db: &mut Connection,
    workspace: String,
    root: String,
    paths: Vec<String>,
    tags: Vec<String>,
    mut checkpoint: impl FnMut(&str, &Path) -> Result<(), String>,
) -> Result<Batch, String> {
    if paths.is_empty()
        || paths.len() > 1000
        || tags.is_empty()
        || tags.len() > 50
        || tags
            .iter()
            .any(|t| t.trim().is_empty() || t.len() > 128 || t.chars().any(char::is_control))
    {
        return Err("Select 1–1000 items and 1–50 valid tags".into());
    }
    let tags = tags
        .into_iter()
        .map(|t| t.trim().to_string())
        .collect::<Vec<_>>();
    let mut out = Batch {
        created: 0,
        updated: 0,
        skipped: 0,
        review: 0,
        errors: vec![],
    };
    for path in paths {
        let result = (|| -> Result<(), String> {
            let tx = db
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(|e| e.to_string())?;
            authorize(&tx, &workspace, &root)?;
            let mut guard = Guard::open(&root, &path)?;
            bind_root(&tx, &workspace, &root, &guard)?;
            let pending:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM xmp_operations WHERE workspace=?1 AND (path=?2 COLLATE NOCASE OR source_identity=?3) AND state IN ('prepared','review_required','applying','backed_up','restoring'))",params![workspace,path,guard.source_identity],|r|r.get(0)).map_err(|e|e.to_string())?;
            if pending {
                return Err("Review the pending XMP operation before retrying".into());
            }
            let original = guard.current()?;
            let candidate = match original.as_deref() {
                None => crate::render_openphoto_xmp(&tags),
                Some(xml) => {
                    let Some(existing) = crate::openphoto_xmp_tags(xml) else {
                        out.skipped += 1;
                        return Ok(());
                    };
                    if tags
                        .iter()
                        .all(|tag| existing.iter().any(|t| t.eq_ignore_ascii_case(tag)))
                    {
                        out.skipped += 1;
                        return Ok(());
                    }
                    let merged = crate::xmp::append_tags(xml, &tags)
                        .ok_or("Cannot safely preserve this XMP document")?;
                    if crate::openphoto_xmp_tags(&merged)
                        .ok_or("Invalid proposed tags")?
                        .len()
                        > 200
                    {
                        return Err("Managed sidecar would exceed 200 keywords".into());
                    }
                    merged
                }
            };
            if candidate.len() as u64 > crate::MAX_XMP_SIDECAR_BYTES {
                return Err("Proposed XMP exceeds the size limit".into());
            }
            let source_hash = guard.source_hash()?;
            let id = record(
                &tx,
                &workspace,
                &root,
                &path,
                original.as_deref(),
                &candidate,
                guard.identity.as_deref(),
            )?;
            tx.execute(
                "UPDATE xmp_operations SET source_identity=?1,root_identity=?2,source_hash=?3 WHERE id=?4",
                params![guard.source_identity, guard.root_identity, source_hash, id],
            )
            .map_err(|e| e.to_string())?;
            // Commit the complete recovery record before any publication attempt.
            let prepared = load_review(&tx, &workspace, id)?;
            checkpoint("before_record_commit", &guard.path)?;
            tx.commit().map_err(|e| e.to_string())?;
            checkpoint("after_record_commit", &guard.path)?;
            if original.is_some() {
                out.review += 1;
                return Ok(());
            }
            let tx = db
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(|e| e.to_string())?;
            authorize(&tx, &workspace, &root)?;
            bind_root(&tx, &workspace, &root, &guard)?;
            if load_review(&tx, &workspace, id)? != prepared {
                return Err("The prepared XMP record changed; no sidecar was published. Refresh saved changes.".into());
            }
            let name = guard
                .path
                .file_name()
                .ok_or("Missing sidecar name")?
                .to_string_lossy();
            if crate::xmp_recovery_inventory::inspect_recovery_pair(
                &guard.path.with_file_name(format!("{name}.openphoto.tmp")),
                &guard.path.with_file_name(format!("{name}.openphoto.bak")),
            )?
            .requires_review()
            {
                return Err(
                    "Legacy recovery files need manual review; all versions have been preserved"
                        .into(),
                );
            }
            // All ancestor/source handles remain pinned across both transactions.
            // create-new publication refuses any competing sidecar, never replaces it.
            publish_new(&guard, &candidate, id, |point| {
                checkpoint(point, &guard.path)
            })?;
            checkpoint("after_publish", &guard.path)?;
            tx.execute(
                "UPDATE xmp_operations SET state='published' WHERE id=?1",
                [id],
            )
            .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
            out.created += 1;
            Ok(())
        })();
        if let Err(error) = result {
            out.errors.push(format!("{path}: {error}"));
        }
    }
    Ok(out)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture {
        root: PathBuf,
        source: PathBuf,
        db: Connection,
    }
    impl Fixture {
        fn new() -> Self {
            let root = loop {
                let candidate = std::env::temp_dir().join(format!(
                    "openphoto-xmp-transactions-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&candidate) {
                    Ok(()) => break candidate,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("Could not create disposable fixture: {error}"),
                }
            };
            let source = root.join("source.jpg");
            fs::write(&source, b"original-media").unwrap();
            let db = Connection::open(root.join("user-data.sqlite")).unwrap();
            schema(&db).unwrap();
            db.execute_batch("CREATE TABLE user_data(key TEXT PRIMARY KEY,value TEXT NOT NULL)")
                .unwrap();
            db.execute(
                "INSERT INTO user_data VALUES('openphoto.workspaces',?1)",
                [serde_json::json!([{"id":"w","roots":[root.to_str().unwrap()]}]).to_string()],
            )
            .unwrap();
            Self { root, source, db }
        }
        fn sidecar(&self) -> PathBuf {
            self.root.join("source.jpg.xmp")
        }
        fn run(&mut self) -> Batch {
            batch(
                &mut self.db,
                "w".into(),
                self.root.to_string_lossy().into(),
                vec![self.source.to_string_lossy().into()],
                vec!["New".into()],
            )
            .unwrap()
        }
        fn reopen(&mut self) {
            self.db = Connection::open(self.root.join("user-data.sqlite")).unwrap();
            schema(&self.db).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            assert_eq!(fs::read(&self.source).unwrap(), b"original-media");
            self.db = Connection::open_in_memory().unwrap();
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
    #[test]
    fn existing_sidecar_is_journaled_without_publication_and_restarts_idempotently() {
        let mut f = Fixture::new();
        let old = crate::render_openphoto_xmp(&["Old".into()]);
        fs::write(f.sidecar(), &old).unwrap();
        let result = f.run();
        assert_eq!(result.review, 1);
        assert_eq!(result.updated, 0);
        assert!(result.errors.is_empty());
        assert_eq!(fs::read_to_string(f.sidecar()).unwrap(), old);
        f.reopen();
        let rows = list(&f.db, "w").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].original.as_deref(), Some(old.as_str()));
        assert!(rows[0].candidate.contains("New"));
        assert_eq!(f.run().errors.len(), 1);
        assert_eq!(list(&f.db, "w").unwrap().len(), 1);
        decide(&f.db, "w", rows[0].id, "kept_external", "preview").unwrap();
        decide(&f.db, "w", rows[0].id, "kept_external", "preview").unwrap();
        assert!(decide(&f.db, "w", rows[0].id, "cancelled", "preview").is_err());
        f.reopen();
        assert!(list(&f.db, "w").unwrap().is_empty());
        assert_eq!(fs::read_to_string(f.sidecar()).unwrap(), old);
        let count: i64 =
            f.db.query_row("SELECT COUNT(*) FROM xmp_operations", [], |r| r.get(0))
                .unwrap();
        assert_eq!(count, 1);
    }
    #[test]
    fn new_sidecar_publication_is_acknowledged_without_overwrite() {
        let mut f = Fixture::new();
        let result = f.run();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.created, 1);
        assert!(result.errors.is_empty());
        assert!(list(&f.db, "w").unwrap().is_empty());
        assert_eq!(f.run().skipped, 1);
    }
    #[test]
    fn interruption_at_each_boundary_keeps_candidate_and_never_auto_recovers() {
        for boundary in [
            "before_record_commit",
            "after_record_commit",
            "after_stage_create",
            "after_stage_write",
            "after_stage_flush",
            "before_stage_reopen",
            "before_publish",
            "after_publish",
        ] {
            let mut f = Fixture::new();
            let result = batch_with_checkpoint(
                &mut f.db,
                "w".into(),
                f.root.to_string_lossy().into(),
                vec![f.source.to_string_lossy().into()],
                vec!["New".into()],
                |point, _| {
                    if point == boundary {
                        Err("injected interruption".into())
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap();
            assert_eq!(result.errors.len(), 1);
            f.reopen();
            let rows = list(&f.db, "w").unwrap();
            assert_eq!(rows.len(), usize::from(boundary != "before_record_commit"));
            assert_eq!(f.sidecar().exists(), boundary == "after_publish");
            let first = serde_json::to_string(&rows).unwrap();
            f.reopen();
            assert_eq!(
                serde_json::to_string(&list(&f.db, "w").unwrap()).unwrap(),
                first
            );
            if boundary != "before_record_commit" {
                assert_eq!(f.run().errors.len(), 1);
            }
        }
    }
    #[test]
    fn external_file_winning_publication_race_is_preserved() {
        let mut f = Fixture::new();
        let result = batch_with_checkpoint(
            &mut f.db,
            "w".into(),
            f.root.to_string_lossy().into(),
            vec![f.source.to_string_lossy().into()],
            vec!["New".into()],
            |point, path| {
                if point == "before_publish" {
                    fs::write(path, b"external-winner").unwrap();
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result.errors.len(), 1);
        assert_eq!(fs::read(f.sidecar()).unwrap(), b"external-winner");
        assert_eq!(list(&f.db, "w").unwrap().len(), 1);
    }
    #[test]
    fn removed_root_and_hardlink_aliases_fail_closed() {
        let mut f = Fixture::new();
        f.db.execute("UPDATE user_data SET value='[]'", []).unwrap();
        assert_eq!(f.run().errors.len(), 1);
        assert!(!f.sidecar().exists());
        let mut f = Fixture::new();
        let alias = f.root.join("alias.jpg");
        fs::hard_link(&f.source, &alias).unwrap();
        assert_eq!(f.run().errors.len(), 1);
        assert!(!f.sidecar().exists());
    }
    #[test]
    fn legacy_artifacts_and_failed_journal_write_are_preserved() {
        let mut f = Fixture::new();
        let backup = f.root.join("source.jpg.xmp.openphoto.bak");
        fs::write(&backup, b"legacy-backup").unwrap();
        assert_eq!(f.run().errors.len(), 1);
        assert_eq!(fs::read(backup).unwrap(), b"legacy-backup");
        assert!(!f.sidecar().exists());
        assert_eq!(list(&f.db, "w").unwrap().len(), 1);
        let mut f = Fixture::new();
        f.db.execute_batch("CREATE TRIGGER fail_record BEFORE INSERT ON xmp_operations BEGIN SELECT RAISE(ABORT,'injected journal failure'); END").unwrap();
        assert_eq!(f.run().errors.len(), 1);
        assert!(!f.sidecar().exists());
        assert!(list(&f.db, "w").unwrap().is_empty());
    }
    #[test]
    fn snapshot_fingerprint_detects_same_length_edits_and_identity_replacement() {
        let f = Fixture::new();
        fs::write(f.sidecar(), b"one").unwrap();
        let before = snapshot(
            &mut Guard::open(f.root.to_str().unwrap(), f.source.to_str().unwrap()).unwrap(),
        )
        .unwrap()
        .fingerprint;
        fs::write(f.sidecar(), b"two").unwrap();
        let after = snapshot(
            &mut Guard::open(f.root.to_str().unwrap(), f.source.to_str().unwrap()).unwrap(),
        )
        .unwrap()
        .fingerprint;
        assert_ne!(before, after);
        fs::rename(f.sidecar(), f.root.join("retained-old.xmp")).unwrap();
        fs::write(f.sidecar(), b"one").unwrap();
        let replaced = snapshot(
            &mut Guard::open(f.root.to_str().unwrap(), f.source.to_str().unwrap()).unwrap(),
        )
        .unwrap()
        .fingerprint;
        assert_ne!(before, replaced);
    }
    #[test]
    fn stale_review_decision_and_revoked_access_never_change_files() {
        let mut f = Fixture::new();
        let old = crate::render_openphoto_xmp(&["Old".into()]);
        fs::write(f.sidecar(), &old).unwrap();
        assert_eq!(f.run().review, 1);
        let row = list(&f.db, "w").unwrap().remove(0);
        let fingerprint = review_snapshot_on(&mut f.db, "w", row.id)
            .unwrap()
            .fingerprint;
        fs::write(f.sidecar(), b"external-edit").unwrap();
        assert!(
            review_decide_on(&mut f.db, "w", row.id, "kept_external", &fingerprint)
                .unwrap_err()
                .contains("changed")
        );
        assert_eq!(list(&f.db, "w").unwrap().len(), 1);
        let fingerprint = review_snapshot_on(&mut f.db, "w", row.id)
            .unwrap()
            .fingerprint;
        review_decide_on(&mut f.db, "w", row.id, "kept_external", &fingerprint).unwrap();
        review_decide_on(&mut f.db, "w", row.id, "kept_external", &fingerprint).unwrap();
        f.db.execute("UPDATE user_data SET value='[]'", []).unwrap();
        assert!(review_decide_on(&mut f.db, "w", row.id, "cancelled", &fingerprint).is_err());
        assert_eq!(fs::read(f.sidecar()).unwrap(), b"external-edit");
    }
    #[test]
    fn corrupt_candidate_and_changed_folder_binding_are_refused() {
        let mut f = Fixture::new();
        fs::write(f.sidecar(), crate::render_openphoto_xmp(&["Old".into()])).unwrap();
        assert_eq!(f.run().review, 1);
        f.db.execute("UPDATE xmp_operations SET candidate='corrupt'", [])
            .unwrap();
        assert!(list(&f.db, "w").unwrap()[0].integrity_error.is_some());
        f.db.execute(
            "UPDATE xmp_folder_bindings SET identity='different-volume:file'",
            [],
        )
        .unwrap();
        assert!(f.run().errors[0].contains("identity changed"));
    }
    #[test]
    fn junction_to_another_directory_is_refused_without_touching_target() {
        let mut f = Fixture::new();
        let target = Fixture::new();
        let link = f.root.join("junction");
        let output = std::process::Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(&target.root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Disposable junction fixture unavailable: {output:?}"
        );
        let result = batch(
            &mut f.db,
            "w".into(),
            f.root.to_string_lossy().into(),
            vec![link.join("source.jpg").to_string_lossy().into()],
            vec!["New".into()],
        )
        .unwrap();
        assert_eq!(result.errors.len(), 1);
        assert!(!target.sidecar().exists());
        assert!(!f.sidecar().exists());
        fs::remove_dir(&link).unwrap();
    }
    #[test]
    fn abrupt_exit_child() {
        let Some(root) = std::env::var_os("OPENPHOTO_XMP_CRASH_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let boundary = std::env::var("OPENPHOTO_XMP_CRASH_BOUNDARY").unwrap();
        let mut db = Connection::open(root.join("user-data.sqlite")).unwrap();
        schema(&db).unwrap();
        let _ = batch_with_checkpoint(
            &mut db,
            "w".into(),
            root.to_string_lossy().into(),
            vec![root.join("source.jpg").to_string_lossy().into()],
            vec!["New".into()],
            |point, _| {
                if point == boundary {
                    std::process::exit(77)
                }
                Ok(())
            },
        );
        panic!("Crash boundary was not reached");
    }
    #[test]
    fn cold_restart_after_abrupt_process_exit_keeps_review_and_source() {
        for boundary in [
            "before_record_commit",
            "after_record_commit",
            "after_stage_create",
            "after_stage_write",
            "after_stage_flush",
            "before_stage_reopen",
            "before_publish",
            "after_publish",
        ] {
            let mut f = Fixture::new();
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "xmp_transactions::tests::abrupt_exit_child",
                    "--nocapture",
                ])
                .env("OPENPHOTO_XMP_CRASH_ROOT", &f.root)
                .env("OPENPHOTO_XMP_CRASH_BOUNDARY", boundary)
                .spawn()
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if std::time::Instant::now() >= deadline {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("Crash helper timed out")
                };
                std::thread::sleep(std::time::Duration::from_millis(10));
            };
            assert_eq!(status.code(), Some(77));
            f.reopen();
            assert_eq!(
                list(&f.db, "w").unwrap().len(),
                usize::from(boundary != "before_record_commit")
            );
            assert_eq!(f.sidecar().exists(), boundary == "after_publish");
            let snapshot = serde_json::to_string(&list(&f.db, "w").unwrap()).unwrap();
            f.reopen();
            assert_eq!(
                serde_json::to_string(&list(&f.db, "w").unwrap()).unwrap(),
                snapshot
            );
        }
    }
    #[test]
    fn staging_swap_or_edit_before_reopening_is_refused() {
        for replace in [false, true] {
            let mut f = Fixture::new();
            let result = batch_with_checkpoint(
                &mut f.db,
                "w".into(),
                f.root.to_string_lossy().into(),
                vec![f.source.to_string_lossy().into()],
                vec!["New".into()],
                |point, path| {
                    if point == "before_stage_reopen" {
                        let stage = fs::read_dir(path.parent().unwrap())
                            .unwrap()
                            .map(|e| e.unwrap().path())
                            .find(|p| {
                                p.file_name()
                                    .unwrap()
                                    .to_string_lossy()
                                    .starts_with(".openphoto-xmp-")
                            })
                            .unwrap();
                        if replace {
                            let bytes = fs::read(&stage).unwrap();
                            fs::rename(&stage, stage.with_extension("retained")).unwrap();
                            fs::write(stage, bytes).unwrap();
                        } else {
                            fs::write(stage, b"changed").unwrap();
                        }
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(result.errors.len(), 1);
            assert!(!f.sidecar().exists());
            assert_eq!(list(&f.db, "w").unwrap().len(), 1);
        }
    }
    #[test]
    fn alternate_streams_and_revocation_after_record_commit_are_refused() {
        let mut f = Fixture::new();
        let result = batch(
            &mut f.db,
            "w".into(),
            f.root.to_string_lossy().into(),
            vec![format!("{}:stream.jpg", f.source.display())],
            vec!["New".into()],
        )
        .unwrap();
        assert!(result.errors[0].contains("alternate data streams"));
        let database = f.root.join("user-data.sqlite");
        let result = batch_with_checkpoint(
            &mut f.db,
            "w".into(),
            f.root.to_string_lossy().into(),
            vec![f.source.to_string_lossy().into()],
            vec!["New".into()],
            |point, _| {
                if point == "after_record_commit" {
                    Connection::open(&database)
                        .unwrap()
                        .execute("UPDATE user_data SET value='[]'", [])
                        .unwrap();
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result.errors.len(), 1);
        assert!(!f.sidecar().exists());
        assert_eq!(list(&f.db, "w").unwrap().len(), 1);
    }
}

#[cfg(windows)]
fn publish_new(
    guard: &Guard,
    candidate: &str,
    id: i64,
    mut checkpoint: impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_DELETE, FILE_SHARE_READ};
    let path = &guard.path;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temp = path.with_file_name(format!(
        ".openphoto-xmp-{}-{id}-{stamp}.tmp",
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        // GENERIC_READ | GENERIC_WRITE | DELETE: rename uses our owned handle.
        .access_mode(0xC0010000)
        .create_new(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    // Exclusive creation and denied write sharing protect the complete bytes.
    // Rename uses the owned handle, never the possibly renamed staging path.
    let staging_identity = identity(&file)?;
    let result: Result<(), String> = (|| {
        checkpoint("after_stage_create")?;
        file.write_all(candidate.as_bytes())
            .map_err(|e| e.to_string())?;
        checkpoint("after_stage_write")?;
        file.sync_all().map_err(|e| e.to_string())?;
        checkpoint("after_stage_flush")?;
        drop(file);
        checkpoint("before_stage_reopen")?;
        // This is an owned new staging file, never the destination. After
        // reopening, verify identity AND bytes under denied-write sharing.
        // Any writer opened in this interval prevents guard acquisition.
        let mut immutable = fs::OpenOptions::new()
            .read(true)
            .access_mode(0x80010000)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        if identity(&immutable)? != staging_identity {
            return Err("Staging identity changed; publication refused".into());
        }
        let mut bytes = Vec::new();
        (&mut immutable)
            .take(crate::MAX_XMP_SIDECAR_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes != candidate.as_bytes() {
            return Err("Staging content changed; publication refused".into());
        }
        checkpoint("before_publish")?;
        rename_owned_no_replace(
            &immutable,
            path,
            guard._directories.last().ok_or("Missing pinned parent")?,
        )?;
        Ok(())
    })();
    // On success rename consumed the stage. On error retain the stage as well
    // as the complete journal candidate; never remove a pathname after unlock.
    result.map_err(|e| format!("{e}; staging and recovery record retained"))
}
#[cfg(not(windows))]
fn publish_new(
    _: &Guard,
    _: &str,
    _: i64,
    _: impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    Err("Unsupported filesystem".into())
}

#[cfg(windows)]
fn rename_owned_no_replace(file: &fs::File, path: &Path, parent: &fs::File) -> Result<(), String> {
    use std::os::windows::{ffi::OsStrExt, io::AsRawHandle};
    use windows_sys::Wdk::Storage::FileSystem::{
        FileRenameInformation, NtSetInformationFile, FILE_RENAME_INFORMATION,
    };
    use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;
    let mut io = std::mem::MaybeUninit::<IO_STATUS_BLOCK>::zeroed();
    let name = path
        .file_name()
        .ok_or("Missing destination name")?
        .encode_wide()
        .collect::<Vec<_>>();
    let offset = std::mem::offset_of!(FILE_RENAME_INFORMATION, FileName);
    let size = (offset + (name.len() + 1) * 2).max(std::mem::size_of::<FILE_RENAME_INFORMATION>());
    let mut buffer = vec![0u64; size.div_ceil(8)];
    // SAFETY: u64 storage supplies required struct alignment and sufficient
    // initialized bytes, including room for the full UTF-16 filename and NUL.
    let ok = unsafe {
        let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
        (*info).Anonymous.ReplaceIfExists = false;
        (*info).RootDirectory = parent.as_raw_handle();
        (*info).FileNameLength = (name.len() * 2) as u32;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            buffer.as_mut_ptr().cast::<u8>().add(offset).cast::<u16>(),
            name.len(),
        );
        NtSetInformationFile(
            file.as_raw_handle(),
            io.as_mut_ptr(),
            info.cast(),
            size as u32,
            FileRenameInformation,
        )
    };
    if ok < 0 {
        return Err(format!(
            "Create-new handle publication refused: NTSTATUS {ok:#x}"
        ));
    }
    Ok(())
}
include!("xmp_review_audit_tests.rs");

#[derive(Serialize, Debug)]
pub(crate) struct Applied {
    state: String,
    backup_name: Option<String>,
}
#[tauri::command]
pub(crate) async fn xmp_review_apply(
    workspace: String,
    id: i64,
    fingerprint: String,
    candidate_hash: String,
) -> Result<Applied, String> {
    xmp_write_task(move || {
        let mut db = crate::user_data::database()?;
        schema(&db)?;
        apply_on(
            &mut db,
            &workspace,
            id,
            &fingerprint,
            &candidate_hash,
            |_, _| Ok(()),
        )
    })
    .await
}
fn apply_on(
    db: &mut Connection,
    workspace: &str,
    id: i64,
    fingerprint: &str,
    candidate_hash: &str,
    mut checkpoint: impl FnMut(&str, &Path) -> Result<(), String>,
) -> Result<Applied, String> {
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let row = load_review(&tx, workspace, id)?;
    if row.integrity_error.is_some() || row.candidate_hash != candidate_hash {
        return Err(
            "The proposed version changed or failed integrity checks; refresh the review".into(),
        );
    }
    let (root, path) = location(&tx, workspace, id)?;
    authorize(&tx, workspace, &root)?;
    let mut guard = Guard::open_mode(&root, &path, true)?;
    bind_root(&tx, workspace, &root, &guard)?;
    let fresh = bound_snapshot(workspace, &row, &mut guard)?;
    if !fresh.can_apply || fresh.fingerprint != fingerprint {
        return Err(fresh
            .reason
            .unwrap_or("The source, proposal or current sidecar changed; review again".into()));
    }
    let backup_name = row
        .original
        .as_ref()
        .map(|_| format!(".openphoto-xmp-original-{id}-{}.bak", std::process::id()));
    tx.execute("UPDATE xmp_operations SET state='applying',backup_name=?1,decision_hash=?2 WHERE workspace=?3 AND id=?4",params![backup_name,fingerprint,workspace,id]).map_err(|e|e.to_string())?;
    let mut intended = row.clone();
    intended.state = "applying".into();
    intended.backup_name = backup_name.clone();
    intended.decision_hash = Some(fingerprint.into());
    checkpoint("before_apply_intent_commit", &guard.path)?;
    tx.commit().map_err(|e| e.to_string())?;
    checkpoint("after_apply_intent_commit", &guard.path)?;
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    authorize(&tx, workspace, &root)?;
    bind_root(&tx, workspace, &root, &guard)?;
    let authoritative = load_review(&tx, workspace, id)?;
    if authoritative != intended {
        return Err("Apply intent changed; all versions have been preserved".into());
    }
    let mut moved = false;
    let result = (|| -> Result<(), String> {
        if let Some(name) = &backup_name {
            checkpoint("before_original_move", &guard.path)?;
            let original = guard
                .sidecar
                .as_ref()
                .ok_or("The reviewed sidecar is missing")?;
            rename_owned_no_replace(
                original,
                &guard.path.with_file_name(name),
                guard._directories.last().ok_or("Missing pinned parent")?,
            )?;
            moved = true;
            checkpoint("after_original_move", &guard.path)?;
        }
        publish_new(&guard, &row.candidate, id, |point| {
            checkpoint(point, &guard.path)
        })?;
        checkpoint("after_apply_publish", &guard.path)?;
        tx.execute(
            "UPDATE xmp_operations SET state='applied' WHERE workspace=?1 AND id=?2",
            params![workspace, id],
        )
        .map_err(|e| e.to_string())?;
        checkpoint("before_apply_acknowledgement", &guard.path)?;
        Ok(())
    })();
    if let Err(error) = result {
        // Restore only via the retained ORIGINAL handle and no-overwrite rename.
        // A competing destination always wins. Never restore by copying to or
        // replacing a pathname. On abrupt process exit this code does not run.
        let restore = if moved {
            guard.sidecar.as_ref().map(|original| {
                rename_owned_no_replace(
                    original,
                    &guard.path,
                    guard._directories.last().expect("guard always owns parent"),
                )
            })
        } else {
            None
        };
        drop(tx);
        if row.original.is_some() && (!moved || matches!(restore, Some(Ok(())))) {
            // A handled refusal with the original intact/restored can be
            // reviewed again. Do not reset ambiguous or externally won saves.
            db.execute("UPDATE xmp_operations SET state='review_required',backup_name=NULL,decision_hash=NULL WHERE workspace=?1 AND id=?2 AND state='applying'",params![workspace,id]).map_err(|e|format!("Original preserved, but review state could not be updated: {e}"))?;
        }
        return Err(format!("Apply was not acknowledged: {error}. Original/candidate remain in the journal; recovery original: {}. {}",backup_name.as_deref().unwrap_or("not needed"),match restore{Some(Ok(()))=>"The original was restored without overwriting any file.",Some(Err(_))=>"A current destination or filesystem refusal prevented restore; the saved original was retained.",None=>"No original was moved."}));
    }
    tx.commit().map_err(|e|format!("Sidecar publication completed but acknowledgement failed; retain and inspect recovery records: {e}"))?;
    Ok(Applied {
        state: "applied".into(),
        backup_name,
    })
}

#[derive(Serialize, Debug)]
pub(crate) struct RecoveryOriginal {
    name: String,
    status: String,
    identity: Option<String>,
    hash: Option<String>,
    error: Option<String>,
}
#[cfg(windows)]
fn recovery_original(
    row: &Review,
    guard: &Guard,
    for_restore: bool,
) -> (RecoveryOriginal, Option<fs::File>) {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::*;
    let name = row.backup_name.clone().unwrap_or_default();
    let mut info = RecoveryOriginal {
        name: name.clone(),
        status: "unavailable".into(),
        identity: None,
        hash: None,
        error: None,
    };
    let result = (|| -> Result<fs::File, String> {
        let prefix = format!(".openphoto-xmp-original-{}-", row.id);
        let pid = name
            .strip_prefix(&prefix)
            .and_then(|s| s.strip_suffix(".bak"))
            .ok_or("Unrecognized recovery filename; it has not been opened")?;
        if pid.is_empty() || !pid.bytes().all(|b| b.is_ascii_digit()) || pid.len() > 20 {
            return Err("Unsafe recovery filename; it has not been opened".into());
        }
        let path = guard.path.with_file_name(&name);
        let mut file = fs::OpenOptions::new()
            .read(true)
            .access_mode(0x80000000 | if for_restore { 0x10000 } else { 0 })
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&path)
            .map_err(|e| e.to_string())?;
        let mut metadata = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
        // SAFETY: successful API call initializes the output for this live handle.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), metadata.as_mut_ptr()) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // SAFETY: successful initialization checked above.
        let metadata = unsafe { metadata.assume_init() };
        if metadata.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY)
            != 0
            || metadata.nNumberOfLinks != 1
        {
            return Err("Recovery original is a link, alias or directory; it was not read".into());
        }
        if file.metadata().map_err(|e| e.to_string())?.len() > crate::MAX_XMP_SIDECAR_BYTES {
            return Err("Recovery original is too large".into());
        }
        let id = identity(&file)?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(crate::MAX_XMP_SIDECAR_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let digest = hash(&bytes);
        info.identity = Some(id.clone());
        info.hash = Some(digest.clone());
        if bytes.len() as u64 > crate::MAX_XMP_SIDECAR_BYTES
            || row.identity.as_deref() != Some(&id)
            || row.original_hash.as_deref() != Some(&digest)
        {
            return Err("Recovery identity or contents changed; restoration refused".into());
        }
        info.status = "verified".into();
        Ok(file)
    })();
    match result {
        Ok(file) => (info, Some(file)),
        Err(error) => {
            info.error = Some(error);
            (info, None)
        }
    }
}
#[cfg(not(windows))]
fn recovery_original(row: &Review, _: &Guard, _: bool) -> (RecoveryOriginal, Option<fs::File>) {
    (
        RecoveryOriginal {
            name: row.backup_name.clone().unwrap_or_default(),
            status: "unavailable".into(),
            identity: None,
            hash: None,
            error: Some("Unsupported platform".into()),
        },
        None,
    )
}

#[tauri::command]
pub(crate) async fn xmp_review_restore(
    workspace: String,
    id: i64,
    fingerprint: String,
    candidate_hash: String,
) -> Result<Applied, String> {
    xmp_write_task(move || {
        let mut db = crate::user_data::database()?;
        schema(&db)?;
        restore_on(
            &mut db,
            &workspace,
            id,
            &fingerprint,
            &candidate_hash,
            |_, _| Ok(()),
        )
    })
    .await
}
fn restore_on(
    db: &mut Connection,
    workspace: &str,
    id: i64,
    fingerprint: &str,
    candidate_hash: &str,
    mut checkpoint: impl FnMut(&str, &Path) -> Result<(), String>,
) -> Result<Applied, String> {
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let row = load_review(&tx, workspace, id)?;
    let (root, path) = location(&tx, workspace, id)?;
    authorize(&tx, workspace, &root)?;
    let mut guard = Guard::open_mode(&root, &path, true)?;
    bind_root(&tx, workspace, &root, &guard)?;
    let fresh = bound_snapshot(workspace, &row, &mut guard)?;
    if !fresh.can_restore
        || fresh.fingerprint != fingerprint
        || row.candidate_hash != candidate_hash
    {
        return Err("The recovery versions changed or cannot be verified. Read them again before restoring.".into());
    }
    let (info, original) = recovery_original(&row, &guard, true);
    if info.status != "verified"
        || serde_json::to_string(&info).map_err(|e| e.to_string())?
            != serde_json::to_string(&fresh.recovery_original).map_err(|e| e.to_string())?
    {
        return Err("The saved original changed during review; restoration refused".into());
    }
    let original = original.ok_or("Recovery original could not be protected")?;
    tx.execute(
        "UPDATE xmp_operations SET state='restoring',decision_hash=?1 WHERE workspace=?2 AND id=?3",
        params![fingerprint, workspace, id],
    )
    .map_err(|e| e.to_string())?;
    let mut intended = row.clone();
    intended.state = "restoring".into();
    intended.decision_hash = Some(fingerprint.into());
    checkpoint("before_restore_intent_commit", &guard.path)?;
    tx.commit().map_err(|e| e.to_string())?;
    checkpoint("after_restore_intent_commit", &guard.path)?;
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    authorize(&tx, workspace, &root)?;
    bind_root(&tx, workspace, &root, &guard)?;
    let current = load_review(&tx, workspace, id)?;
    if current != intended {
        return Err("Recovery intent changed; all files were preserved".into());
    }
    checkpoint("before_restore_move", &guard.path)?;
    rename_owned_no_replace(
        &original,
        &guard.path,
        guard._directories.last().ok_or("Missing pinned parent")?,
    )?;
    checkpoint("after_restore_move", &guard.path)?;
    tx.execute(
        "UPDATE xmp_operations SET state='restored' WHERE workspace=?1 AND id=?2",
        params![workspace, id],
    )
    .map_err(|e| e.to_string())?;
    checkpoint("before_restore_acknowledgement", &guard.path)?;
    tx.commit().map_err(|e| {
        format!("Original restored but acknowledgement failed; review saved records: {e}")
    })?;
    Ok(Applied {
        state: "restored".into(),
        backup_name: row.backup_name,
    })
}
include!("xmp_step3_tests.rs");
include!("xmp_source_content_tests.rs");
include!("xmp_worker_tests.rs");
include!("xmp_paging_tests.rs");
include!("xmp_staging_observation_tests.rs");
include!("xmp_intent_binding_tests.rs");
