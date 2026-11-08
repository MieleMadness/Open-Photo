use super::*;
const BACKUP: &[u8; 8] = b"OPVB001\0";
#[derive(Debug)]
pub(super) enum MediaFailure {
    Access,
    Range(u64),
}
impl From<String> for MediaFailure {
    fn from(_: String) -> Self {
        Self::Access
    }
}
impl From<&str> for MediaFailure {
    fn from(_: &str) -> Self {
        Self::Access
    }
}
type MediaPayload = (Zeroizing<Vec<u8>>, String, u64, u64, u64, bool);
impl VaultState {
    pub(super) fn backup(&self, token: &str, destination: &Path) -> Result<(), String> {
        let _operation = self.operation.lock().map_err(poisoned)?;
        let lease = self.lease(token, true)?;
        let store = self.store()?;
        if destination
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("opvault"))
        {
            return Err("Choose a new .opvault backup filename".into());
        }
        let parent = fs::canonicalize(
            destination
                .parent()
                .ok_or("Choose an existing backup folder")?,
        )
        .map_err(io_error)?;
        if parent.starts_with(fs::canonicalize(&store.path).map_err(io_error)?) {
            return Err("Save backups outside the vault directory".into());
        }
        let (header, raw) = store.state()?;
        let catalog = catalog(&header, &lease.master)?;
        let required = 12 + raw.len() as u64 + catalog.items.iter().map(cipher_len).sum::<u64>();
        if crate::video_export::available_space(&parent)?
            .is_some_and(|free| free < required + 8 * 1024 * 1024)
        {
            return Err("Not enough space for this encrypted backup".into());
        }
        let mut output = crate::safe_output::StagedOutput::new(destination)?;
        output.file().write_all(BACKUP).map_err(io_error)?;
        output
            .file()
            .write_all(&(raw.len() as u32).to_le_bytes())
            .map_err(io_error)?;
        output.file().write_all(&raw).map_err(io_error)?;
        #[cfg(test)]
        tests::crash::point("backup-partial-write");
        let mut buffer = vec![0; CHUNK as usize];
        for object in &catalog.items {
            let mut file = regular(&store.object_path(&object.id)?, false)?;
            if file.metadata().map_err(io_error)?.len() != cipher_len(object) {
                return Err("Vault media length failed verification".into());
            }
            let mut left = cipher_len(object);
            while left > 0 {
                if !self.current(&lease, true) {
                    return Err("Vault backup cancelled or locked".into());
                }
                let count = left.min(CHUNK) as usize;
                file.read_exact(&mut buffer[..count]).map_err(io_error)?;
                output
                    .file()
                    .write_all(&buffer[..count])
                    .map_err(io_error)?;
                left -= count as u64;
            }
        }
        if output.file().metadata().map_err(io_error)?.len() != required
            || !self.current(&lease, true)
        {
            return Err("Vault backup did not pass final validation".into());
        }
        output.verify_closed(|path| {
            verify_backup(path, &raw, &header.id, &catalog, &|| {
                !self.current(&lease, true)
            })
        })?;
        if !self.current(&lease, true) {
            return Err("Vault backup cancelled or locked".into());
        }
        #[cfg(test)]
        tests::crash::point("backup-before-commit");
        output.commit()?;
        #[cfg(test)]
        tests::crash::point("backup-after-commit");
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn restore(&self, path: &Path, pin: &str) -> Result<Grant, String> {
        self.restore_at(path, pin, self.epoch.load(Ordering::SeqCst))
    }
    pub(super) fn restore_at(&self, path: &Path, pin: &str, epoch: u64) -> Result<Grant, String> {
        let _operation = self.operation.lock().map_err(poisoned)?;
        if self.configured()? {
            return Err(
                "Restore requires an empty vault location. Existing vault data is never replaced."
                    .into(),
            );
        }
        if pin.is_empty() || pin.len() > 256 {
            return Err("Enter the backup's vault PIN or passphrase".into());
        }
        self.ensure_parent()?;
        let workspace = (self.authority)()?;
        self.admission(epoch, &workspace)?;
        let work_epoch = self.work_epoch.load(Ordering::SeqCst);
        let mut source = regular(path, false)?;
        let mut prefix = [0; 12];
        source.read_exact(&mut prefix).map_err(io_error)?;
        if &prefix[..8] != BACKUP {
            return Err("Unsupported vault backup. The original file was kept.".into());
        }
        let length = u32::from_le_bytes(prefix[8..].try_into().map_err(io_error)?) as u64;
        if length == 0 || length > MAX_HEADER {
            return Err("Vault backup header exceeds its format budget".into());
        }
        let mut raw = vec![0; length as usize];
        source.read_exact(&mut raw).map_err(io_error)?;
        let header = parse_header(&raw)?;
        let master = self.verify_pin(&header, pin)?;
        let catalog = catalog(&header, &master)?;
        let expected = 12 + length + catalog.items.iter().map(cipher_len).sum::<u64>();
        if source.metadata().map_err(io_error)?.len() != expected {
            return Err("Vault backup is incomplete or contains unexpected data".into());
        }
        if crate::video_export::available_space(self.parent()?)?
            .is_some_and(|free| free < expected + 8 * 1024 * 1024)
        {
            return Err("Not enough space to restore and verify this vault".into());
        }
        let staged = staged_directory(self.parent()?)?;
        let cancelled = || {
            self.admission(epoch, &workspace).is_err()
                || self.work_epoch.load(Ordering::SeqCst) != work_epoch
        };
        let mut buffer = vec![0; CHUNK as usize];
        for object in &catalog.items {
            let path = staged.0.join(format!("{}.opv", object.id));
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(io_error)?;
            let mut left = cipher_len(object);
            while left > 0 {
                if cancelled() {
                    return Err("Vault restore cancelled or locked".into());
                }
                let count = left.min(CHUNK) as usize;
                source.read_exact(&mut buffer[..count]).map_err(io_error)?;
                output.write_all(&buffer[..count]).map_err(io_error)?;
                #[cfg(test)]
                tests::crash::point("restore-partial-write");
                left -= count as u64;
            }
            output.sync_all().map_err(io_error)?;
            drop(output);
            verify_object(&mut regular(&path, false)?, &header.id, object, &cancelled)?;
        }
        let mut state = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(staged.0.join("state.opv"))
            .map_err(io_error)?;
        state.write_all(&raw).map_err(io_error)?;
        state.sync_all().map_err(io_error)?;
        drop(state);
        if cancelled() {
            return Err("Vault restore cancelled or locked".into());
        }
        self.success()?;
        #[cfg(test)]
        tests::crash::point("restore-before-publish");
        publish_directory(&staged, &self.root, cancelled)?;
        #[cfg(test)]
        tests::crash::point("restore-after-publish");
        self.store()?;
        self.grant(master, &catalog, workspace, epoch)
    }
    pub(super) fn media(
        &self,
        token: &str,
        id: &str,
        range: Option<&str>,
    ) -> Result<MediaPayload, MediaFailure> {
        let lease = self.lease(token, false)?;
        let store = self.store()?;
        let (header, _) = store.state()?;
        let catalog = catalog(&header, &lease.master)?;
        let object = catalog
            .items
            .iter()
            .find(|o| o.id == id)
            .ok_or("Vault item is unavailable")?;
        let (start, end, partial) =
            media_range(range, object.bytes, object.mime.starts_with("video/"))
                .map_err(|_| MediaFailure::Range(object.bytes))?;
        let mut file = regular(&store.object_path(id)?, false)?;
        let bytes = read_range(&mut file, &header.id, object, start, end, &|| {
            !self.current(&lease, false)
        })?;
        if !self.current(&lease, false) {
            return Err(LOCKED.into());
        }
        Ok((
            bytes,
            object.mime.clone(),
            start,
            end,
            object.bytes,
            partial,
        ))
    }
}
pub(super) fn media_range(
    range: Option<&str>,
    bytes: u64,
    video: bool,
) -> Result<(u64, u64, bool), String> {
    if bytes == 0 {
        return Err("Invalid media length".into());
    }
    let limit = if video { 8 * 1024 * 1024 } else { MAX_PHOTO };
    let Some(range) = range else {
        return Ok((0, (bytes - 1).min(limit - 1), video && bytes > limit));
    };
    let (start, end) = range
        .strip_prefix("bytes=")
        .and_then(|r| r.split_once('-'))
        .filter(|(a, b)| !a.contains(',') && !b.contains(','))
        .ok_or("Invalid media range")?;
    let (start, end) = if start.is_empty() {
        let suffix = end.parse::<u64>().map_err(io_error)?;
        if suffix == 0 {
            return Err("Invalid media range".into());
        }
        (bytes.saturating_sub(suffix), bytes - 1)
    } else {
        (
            start.parse::<u64>().map_err(io_error)?,
            if end.is_empty() {
                bytes - 1
            } else {
                end.parse::<u64>().map_err(io_error)?.min(bytes - 1)
            },
        )
    };
    if start >= bytes || end < start {
        return Err("Invalid media range".into());
    }
    Ok((start, end.min(start + limit - 1), true))
}
pub(crate) fn protocol(
    app: &tauri::AppHandle,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    use tauri::Manager;
    response(&app.state::<Arc<VaultState>>(), request)
}
pub(super) fn response(
    state: &VaultState,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{Response, StatusCode};
    let origin = request
        .headers()
        .get("origin")
        .and_then(|v| v.to_str().ok());
    let allowed = matches!(
        origin,
        None | Some("http://tauri.localhost" | "https://tauri.localhost" | "tauri://localhost")
    ) || (cfg!(debug_assertions) && origin == Some("http://localhost:1420"));
    let base = || {
        Response::builder()
            .header("Cache-Control", "no-store, max-age=0")
            .header("Pragma", "no-cache")
            .header("Expires", "0")
            .header("Referrer-Policy", "no-referrer")
            .header("X-Content-Type-Options", "nosniff")
            .header(
                "Access-Control-Allow-Origin",
                origin
                    .filter(|_| allowed)
                    .unwrap_or("http://tauri.localhost"),
            )
            .header("Accept-Ranges", "bytes")
    };
    if !allowed || !available() || !matches!(request.method().as_str(), "GET" | "HEAD") {
        return base()
            .status(StatusCode::NOT_FOUND)
            .body(vec![])
            .unwrap_or_default();
    }
    let parts = request
        .uri()
        .path()
        .trim_start_matches('/')
        .split('/')
        .collect::<Vec<_>>();
    if parts.len() != 2
        || parts[0].len() != 64
        || !parts[0].bytes().all(|c| c.is_ascii_hexdigit())
        || !valid_id(parts[1])
    {
        return base()
            .status(StatusCode::NOT_FOUND)
            .body(vec![])
            .unwrap_or_default();
    }
    let range = request.headers().get("range").and_then(|v| v.to_str().ok());
    match state.media(parts[0], parts[1], range) {
        Ok((bytes, mime, start, end, total, partial)) => {
            let mut response = base()
                .status(if partial {
                    StatusCode::PARTIAL_CONTENT
                } else {
                    StatusCode::OK
                })
                .header("Content-Type", mime)
                .header("Content-Length", bytes.len());
            if partial {
                response = response.header("Content-Range", format!("bytes {start}-{end}/{total}"));
            }
            response
                .body(if request.method() == "HEAD" {
                    vec![]
                } else {
                    bytes.to_vec()
                })
                .unwrap_or_default()
        }
        Err(MediaFailure::Range(total)) => base()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header("Content-Range", format!("bytes */{total}"))
            .body(vec![])
            .unwrap_or_default(),
        Err(_) => base()
            .status(StatusCode::NOT_FOUND)
            .body(vec![])
            .unwrap_or_default(),
    }
}

/// Verify the closed staged archive, not only its source objects or byte count.
pub(super) fn verify_backup(
    path: &Path,
    expected_header: &[u8],
    vault: &str,
    catalog: &Catalog,
    cancel: &impl Fn() -> bool,
) -> Result<(), String> {
    if cancel() {
        return Err("Vault backup cancelled or locked".into());
    }
    let mut file = regular(path, false)?;
    let required =
        12 + expected_header.len() as u64 + catalog.items.iter().map(cipher_len).sum::<u64>();
    if file.metadata().map_err(io_error)?.len() != required {
        return Err("Vault backup length failed verification".into());
    }
    let mut prefix = [0; 12];
    file.read_exact(&mut prefix).map_err(io_error)?;
    if &prefix[..8] != BACKUP
        || u32::from_le_bytes(prefix[8..].try_into().map_err(io_error)?) as usize
            != expected_header.len()
    {
        return Err("Vault backup header failed verification".into());
    }
    let mut header = vec![0; expected_header.len()];
    file.read_exact(&mut header).map_err(io_error)?;
    if header != expected_header {
        return Err("Vault backup header changed".into());
    }
    for object in &catalog.items {
        verify_object_stream(&mut file, vault, object, cancel)?;
    }
    let mut extra = [0; 1];
    if file.read(&mut extra).map_err(io_error)? != 0 || cancel() {
        return Err("Vault backup did not pass final validation".into());
    }
    Ok(())
}
