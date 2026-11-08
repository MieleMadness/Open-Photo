//! Account-bound local vault. Native authorization is required for every operation and byte range.
mod crypto;
mod storage;
mod transfer;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
pub(crate) use storage::CiphertextTarget;
use storage::*;
pub(crate) use transfer::protocol as transfer_protocol;
use zeroize::{Zeroize, Zeroizing};
const IDLE: Duration = Duration::from_secs(300);
const LOCKED: &str = "The vault is locked. Unlock it to continue.";
fn poisoned<T>(_: std::sync::PoisonError<T>) -> String {
    "The vault stopped unexpectedly. Restart Open-Photo before continuing.".into()
}
fn io_error(_: impl std::fmt::Display) -> String {
    "The vault operation could not finish. Existing encrypted data and original media were kept."
        .into()
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |t| t.as_secs())
}
fn available() -> bool {
    cfg!(windows) && (cfg!(debug_assertions) || cfg!(feature = "private-vault"))
}
fn require_available() -> Result<(), String> {
    if available() {
        Ok(())
    } else {
        Err("The private vault is unavailable in this build while security and installed-app checks are pending.".into())
    }
}
#[derive(Deserialize)]
#[serde(transparent)]
pub struct Credential(String);
impl Drop for Credential {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
fn validate_credential(value: &str) -> Result<(), String> {
    if value.len() > 256 {
        return Err("This PIN or passphrase is too long. Use a shorter one.".into());
    }
    if value.contains('\0')
        || if value.bytes().all(|c| c.is_ascii_digit()) {
            value.len() < 6
        } else {
            value.chars().count() < 12
        }
    {
        return Err(
            "Use at least six digits, or a passphrase of at least twelve characters.".into(),
        );
    }
    Ok(())
}
struct Session {
    token: Zeroizing<String>,
    master: Zeroizing<Vec<u8>>,
    workspace: String,
    last_activity: Instant,
    epoch: u64,
}
struct Lease {
    master: Zeroizing<Vec<u8>>,
    workspace: String,
    epoch: u64,
    work_epoch: u64,
}
pub struct VaultState {
    root: PathBuf,
    session: Mutex<Option<Session>>,
    storage: Mutex<Option<Arc<Storage>>>,
    operation: Mutex<()>,
    epoch: AtomicU64,
    work_epoch: AtomicU64,
    authority: Box<dyn Fn() -> Result<String, String> + Send + Sync>,
    screen: Box<dyn Fn() -> bool + Send + Sync>,
    clock: Box<dyn Fn() -> u64 + Send + Sync>,
    pub(crate) stream_gate: Arc<tokio::sync::Semaphore>,
}
impl Default for VaultState {
    fn default() -> Self {
        let root = dirs::data_local_dir()
            .filter(|p| p.is_absolute())
            .map(|p| p.join("Open-Photo/private-vault-v1"))
            .unwrap_or_default();
        Self::new(
            root,
            Box::new(|| {
                let workspace =
                    crate::user_data::setting("openphoto.activeWorkspace")?.unwrap_or_default();
                if !workspace.is_empty() {
                    crate::user_data::workspace_roots(&workspace)?;
                }
                Ok(workspace)
            }),
            crypto::session_unlocked,
        )
    }
}
#[derive(Serialize)]
pub struct Status {
    available: bool,
    configured: bool,
    retry_after_seconds: u64,
    reason: Option<&'static str>,
}
#[derive(Serialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub bytes: u64,
}
impl Drop for Item {
    fn drop(&mut self) {
        self.name.zeroize();
    }
}
#[derive(Serialize)]
pub struct Grant {
    token: String,
    epoch: u64,
    items: Vec<Item>,
}
impl Drop for Grant {
    fn drop(&mut self) {
        self.token.zeroize();
    }
}
#[derive(Clone, Copy, Serialize)]
pub(crate) struct LockNotice {
    pub(crate) through_epoch: u64,
}
#[derive(Serialize, Default)]
pub struct Reply {
    #[serde(skip_serializing_if = "Option::is_none")]
    lock: Option<LockNotice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    grant: Option<Grant>,
    #[serde(skip_serializing_if = "Option::is_none")]
    items: Option<Vec<Item>>,
}
#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Attempts {
    version: u32,
    failures: u32,
    until: u64,
}
fn summaries(catalog: &Catalog) -> Vec<Item> {
    catalog
        .items
        .iter()
        .map(|o| Item {
            id: o.id.clone(),
            name: o.name.clone(),
            mime: o.mime.clone(),
            bytes: o.bytes,
        })
        .collect()
}
impl VaultState {
    fn new(
        root: PathBuf,
        authority: Box<dyn Fn() -> Result<String, String> + Send + Sync>,
        screen: fn() -> bool,
    ) -> Self {
        Self {
            root,
            session: Mutex::new(None),
            storage: Mutex::new(None),
            operation: Mutex::new(()),
            epoch: AtomicU64::new(1),
            work_epoch: AtomicU64::new(1),
            authority,
            screen: Box::new(screen),
            clock: Box::new(now),
            stream_gate: Arc::new(tokio::sync::Semaphore::new(2)),
        }
    }
    fn time(&self) -> u64 {
        (self.clock)()
    }
    fn configured(&self) -> Result<bool, String> {
        if !self.root.is_absolute() {
            return Err("The Windows vault location is unavailable".into());
        }
        match fs::symlink_metadata(&self.root) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io_error(e)),
        }
    }
    fn verify_pin(&self, header: &Header, pin: &str) -> Result<Zeroizing<Vec<u8>>, String> {
        self.begin_attempt()?;
        match unwrap(header, pin) {
            Ok(master) => Ok(master),
            Err(error) => {
                let mut attempts = self.attempts()?;
                let delay = if attempts.failures >= 5 {
                    2u64.pow((attempts.failures - 5).min(9)).min(300)
                } else {
                    0
                };
                attempts.until = self.time() + delay;
                self.write_attempts(&attempts)?;
                Err(error)
            }
        }
    }
    fn parent(&self) -> Result<&Path, String> {
        self.root
            .parent()
            .ok_or_else(|| "Vault location is unavailable".into())
    }
    fn ensure_parent(&self) -> Result<(), String> {
        let parent = self.parent()?;
        if !parent.exists() {
            fs::create_dir(parent).map_err(io_error)?;
        }
        drop(directory(parent)?);
        Ok(())
    }
    fn store(&self) -> Result<Arc<Storage>, String> {
        let mut current = self.storage.lock().map_err(poisoned)?;
        if let Some(storage) = current.as_ref() {
            return Ok(storage.clone());
        }
        let storage = Arc::new(Storage::open(&self.root)?);
        *current = Some(storage.clone());
        Ok(storage)
    }
    fn attempts(&self) -> Result<Attempts, String> {
        let path = self.parent()?.join("vault-attempts-v1.json");
        if matches!(fs::symlink_metadata(&path),Err(e) if e.kind()==std::io::ErrorKind::NotFound) {
            return Ok(Attempts {
                version: 1,
                ..Default::default()
            });
        }
        let value: Attempts = serde_json::from_slice(&read_bounded(&path, 4096)?)
            .map_err(|_| "Vault attempt tracking needs repair; existing data was kept")?;
        if value.version != 1 || value.failures > 1_000_000 {
            return Err("Unsupported vault attempt tracking".into());
        }
        Ok(value)
    }
    fn write_attempts(&self, value: &Attempts) -> Result<(), String> {
        self.attempts()?;
        self.ensure_parent()?;
        replace(
            &self.parent()?.join("vault-attempts-v1.json"),
            &serde_json::to_vec(value).map_err(io_error)?,
        )
    }
    fn begin_attempt(&self) -> Result<(), String> {
        let mut attempts = self.attempts()?;
        if attempts.until > self.time() {
            return Err(format!(
                "Wait {} seconds before trying the vault again.",
                attempts.until - self.time()
            ));
        }
        attempts.failures = attempts.failures.saturating_add(1).min(1_000_000);
        let delay = if attempts.failures >= 5 {
            2u64.pow((attempts.failures - 5).min(9)).min(300)
        } else {
            0
        };
        attempts.until = self.time() + delay;
        self.write_attempts(&attempts)
    }
    fn success(&self) -> Result<(), String> {
        self.write_attempts(&Attempts {
            version: 1,
            ..Default::default()
        })
    }
    fn lock_inner(&self) -> (LockNotice, bool) {
        let through_epoch = self.epoch.fetch_add(1, Ordering::SeqCst);
        self.work_epoch.fetch_add(1, Ordering::SeqCst);
        let mut slot = self.session.lock().unwrap_or_else(|p| p.into_inner());
        let removed = slot.as_ref().is_some_and(|s| s.epoch <= through_epoch);
        if removed {
            slot.take();
        }
        (LockNotice { through_epoch }, removed)
    }
    pub(crate) fn lock_notice(&self) -> LockNotice {
        self.lock_inner().0
    }
    #[cfg(test)]
    pub(crate) fn lock(&self) -> bool {
        self.lock_inner().1
    }
    fn revoke_if(&self, expected: u64) -> Option<LockNotice> {
        let mut slot = self.session.lock().unwrap_or_else(|p| p.into_inner());
        if self
            .epoch
            .compare_exchange(expected, expected + 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return None;
        }
        self.work_epoch.fetch_add(1, Ordering::SeqCst);
        if slot.as_ref().is_some_and(|s| s.epoch == expected) {
            slot.take();
        }
        Some(LockNotice {
            through_epoch: expected,
        })
    }
    fn revoke_token(&self, token: &str) -> Option<LockNotice> {
        let expected = {
            let slot = self.session.lock().unwrap_or_else(|p| p.into_inner());
            slot.as_ref()
                .filter(|s| {
                    token.len() == s.token.len()
                        && token
                            .bytes()
                            .zip(s.token.bytes())
                            .fold(0, |d, (a, b)| d | (a ^ b))
                            == 0
                })
                .map(|s| s.epoch)
        }?;
        self.revoke_if(expected)
    }
    fn lease(&self, token: &str, touch: bool) -> Result<Lease, String> {
        let observed = self.epoch.load(Ordering::SeqCst);
        let workspace = (self.authority)()?;
        if !(self.screen)() {
            self.revoke_if(observed);
            return Err(LOCKED.into());
        }
        let mut slot = self.session.lock().map_err(poisoned)?;
        if self.epoch.load(Ordering::SeqCst) != observed {
            return Err(LOCKED.into());
        }
        let Some(session) = slot.as_mut() else {
            return Err(LOCKED.into());
        };
        if session.last_activity.elapsed() >= IDLE
            || session.workspace != workspace
            || session.epoch != self.epoch.load(Ordering::SeqCst)
        {
            slot.take();
            self.epoch.fetch_add(1, Ordering::SeqCst);
            return Err(LOCKED.into());
        }
        if token.len() != session.token.len()
            || token
                .bytes()
                .zip(session.token.bytes())
                .fold(0, |diff, (a, b)| diff | (a ^ b))
                != 0
        {
            return Err(LOCKED.into());
        }
        if touch {
            session.last_activity = Instant::now();
        }
        Ok(Lease {
            master: Zeroizing::new(session.master.to_vec()),
            workspace: session.workspace.clone(),
            epoch: session.epoch,
            work_epoch: self.work_epoch.load(Ordering::SeqCst),
        })
    }
    fn current(&self, lease: &Lease, work: bool) -> bool {
        if self.epoch.load(Ordering::SeqCst) != lease.epoch
            || (work && self.work_epoch.load(Ordering::SeqCst) != lease.work_epoch)
        {
            return false;
        }
        if !(self.screen)() || !(self.authority)().is_ok_and(|w| w == lease.workspace) {
            self.revoke_if(lease.epoch);
            return false;
        }
        let Ok(slot) = self.session.lock() else {
            return false;
        };
        let current = slot
            .as_ref()
            .is_some_and(|s| s.epoch == lease.epoch && s.last_activity.elapsed() < IDLE);
        drop(slot);
        if !current {
            self.revoke_if(lease.epoch);
        }
        current
    }
    fn admission(&self, epoch: u64, workspace: &str) -> Result<(), String> {
        if self.epoch.load(Ordering::SeqCst) != epoch
            || !(self.screen)()
            || (self.authority)()? != workspace
        {
            return Err("The vault request was cancelled or the workspace changed".into());
        }
        Ok(())
    }
    fn grant(
        &self,
        master: Zeroizing<Vec<u8>>,
        catalog: &Catalog,
        workspace: String,
        expected_epoch: u64,
    ) -> Result<Grant, String> {
        self.admission(expected_epoch, &workspace)?;
        let token = crypto::random::<32>()?
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let mut slot = self.session.lock().map_err(poisoned)?;
        if self
            .epoch
            .compare_exchange(
                expected_epoch,
                expected_epoch + 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_err()
        {
            return Err(LOCKED.into());
        }
        *slot = Some(Session {
            token: Zeroizing::new(token.clone()),
            master,
            workspace,
            last_activity: Instant::now(),
            epoch: expected_epoch + 1,
        });
        Ok(Grant {
            epoch: expected_epoch + 1,
            token,
            items: summaries(catalog),
        })
    }
    fn status(&self) -> Result<Status, String> {
        let configured = self.configured()?;
        if configured {
            self.store()?.state()?;
        }
        Ok(Status {
            available: available(),
            configured,
            retry_after_seconds: self.attempts()?.until.saturating_sub(self.time()),
            reason: if available() {
                None
            } else {
                Some("The private vault is unavailable in this build while security and installed-app checks are pending.")
            },
        })
    }
    #[cfg(test)]
    fn setup(&self, pin: &str, confirmation: &str) -> Result<Grant, String> {
        self.setup_at(pin, confirmation, self.epoch.load(Ordering::SeqCst))
    }
    fn setup_at(&self, pin: &str, confirmation: &str, epoch: u64) -> Result<Grant, String> {
        let _operation = self.operation.lock().map_err(poisoned)?;
        validate_credential(pin)?;
        if pin != confirmation {
            return Err("The PIN/passphrase confirmation does not match".into());
        }
        if self.configured()? {
            return Err(
                "A vault already exists. Unlock it; setup never replaces stored media.".into(),
            );
        }
        self.ensure_parent()?;
        let workspace = (self.authority)()?;
        self.admission(epoch, &workspace)?;
        let master = Zeroizing::new(crypto::random::<32>()?.to_vec());
        let mut header = Header {
            version: 1,
            id: crypto::id()?,
            iterations: crypto::ITERATIONS,
            salt: String::new(),
            wrapped: String::new(),
            catalog: String::new(),
        };
        wrap(&mut header, &master, pin)?;
        let catalog = Catalog {
            version: 1,
            items: vec![],
        };
        set_catalog(&mut header, &master, &catalog)?;
        let staged = staged_directory(self.parent()?)?;
        let data = serde_json::to_vec(&header).map_err(io_error)?;
        let mut output = crate::safe_output::StagedOutput::new(&staged.0.join("state.opv"))?;
        output.file().write_all(&data).map_err(io_error)?;
        output.commit()?;
        let verified = parse_header(&read_bounded(&staged.0.join("state.opv"), MAX_HEADER)?)?;
        let verified_master = unwrap(&verified, pin)?;
        storage::catalog(&verified, &verified_master)?;
        self.success()?;
        self.admission(epoch, &workspace)?;
        publish_directory(&staged, &self.root, || {
            self.admission(epoch, &workspace).is_err()
        })?;
        self.store()?;
        self.grant(master, &catalog, workspace, epoch)
    }
    #[cfg(test)]
    fn unlock(&self, pin: &str) -> Result<Grant, String> {
        self.unlock_at(pin, self.epoch.load(Ordering::SeqCst))
    }
    fn unlock_at(&self, pin: &str, epoch: u64) -> Result<Grant, String> {
        let _operation = self.operation.lock().map_err(poisoned)?;
        if pin.len() > 256 || pin.is_empty() {
            return Err("Enter your vault PIN or passphrase".into());
        }
        let workspace = (self.authority)()?;
        self.admission(epoch, &workspace)?;
        let (header, _) = self.store()?.state()?;
        let master = self.verify_pin(&header, pin)?;
        let catalog = catalog(&header, &master)?;
        self.success()?;
        self.grant(master, &catalog, workspace, epoch)
    }
    fn list(&self, token: &str) -> Result<Vec<Item>, String> {
        let lease = self.lease(token, true)?;
        let (header, _) = self.store()?.state()?;
        let catalog = catalog(&header, &lease.master)?;
        if !self.current(&lease, false) {
            return Err(LOCKED.into());
        }
        Ok(summaries(&catalog))
    }
    fn change_pin(
        &self,
        token: &str,
        current: &str,
        new: &str,
        confirm: &str,
    ) -> Result<Grant, String> {
        let _operation = self.operation.lock().map_err(poisoned)?;
        validate_credential(new)?;
        if new != confirm {
            return Err("The new PIN/passphrase confirmation does not match".into());
        }
        let lease = self.lease(token, true)?;
        let store = self.store()?;
        let (mut header, old) = store.state()?;
        let master = self.verify_pin(&header, current)?;
        if master.as_slice() != lease.master.as_slice() {
            return Err("Vault key changed; lock and reopen it".into());
        }
        let catalog = catalog(&header, &master)?;
        wrap(&mut header, &master, new)?;
        if !self.current(&lease, true) {
            return Err(LOCKED.into());
        }
        #[cfg(test)]
        tests::crash::point("pin-before-publish");
        store.publish(&old, &header, || !self.current(&lease, true))?;
        #[cfg(test)]
        tests::crash::point("pin-after-publish");
        self.success()?;
        if self.admission(lease.epoch, &lease.workspace).is_err() {
            self.revoke_if(lease.epoch);
            return Err(
                "The new PIN was saved, but the vault was locked. Unlock it with the new PIN."
                    .into(),
            );
        }
        let token = crypto::random::<32>()?
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let mut slot = self.session.lock().map_err(poisoned)?;
        if self
            .epoch
            .compare_exchange(
                lease.epoch,
                lease.epoch + 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_err()
        {
            return Err(
                "The new PIN was saved, but the vault was locked. Unlock it with the new PIN."
                    .into(),
            );
        }
        *slot = Some(Session {
            token: Zeroizing::new(token.clone()),
            master,
            workspace: lease.workspace.clone(),
            last_activity: Instant::now(),
            epoch: lease.epoch + 1,
        });
        Ok(Grant {
            epoch: lease.epoch + 1,
            token,
            items: summaries(&catalog),
        })
    }

    fn import(&self, token: &str, path: &Path) -> Result<Vec<Item>, String> {
        let _operation = self.operation.lock().map_err(poisoned)?;
        let lease = self.lease(token, true)?;
        let store = self.store()?;
        let source_path = fs::canonicalize(path).map_err(io_error)?;
        if source_path.starts_with(fs::canonicalize(&store.path).map_err(io_error)?) {
            return Err("Vault files cannot be imported as ordinary source media".into());
        }
        let mut source = regular(path, false)?;
        let bytes = source.metadata().map_err(io_error)?.len();
        let (mut mime, limit) = match path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "jpg" | "jpeg" => ("image/jpeg", MAX_PHOTO),
            "png" => ("image/png", MAX_PHOTO),
            "webp" => ("image/webp", MAX_PHOTO),
            "gif" => ("image/gif", MAX_PHOTO),
            "bmp" => ("image/bmp", MAX_PHOTO),
            "mp4" => ("video/mp4", MAX_VIDEO),
            "webm" => ("video/webm", MAX_VIDEO),
            _ => return Err("Choose a JPEG, PNG, WebP, GIF, BMP, MP4 or WebM file".into()),
        };
        if bytes == 0 || bytes > limit {
            return Err("Vault imports support photos up to 64 MB and videos up to 512 MB".into());
        }
        if mime.starts_with("image/") {
            let reader = image::ImageReader::new(std::io::BufReader::new(
                source.try_clone().map_err(io_error)?,
            ))
            .with_guessed_format()
            .map_err(io_error)?;
            mime = match reader.format() {
                Some(image::ImageFormat::Jpeg) => "image/jpeg",
                Some(image::ImageFormat::Png) => "image/png",
                Some(image::ImageFormat::WebP) => "image/webp",
                Some(image::ImageFormat::Gif) => "image/gif",
                Some(image::ImageFormat::Bmp) => "image/bmp",
                _ => return Err("This image format is not supported by the private vault".into()),
            };
            let (w, h) = reader
                .into_dimensions()
                .map_err(|_| "This image cannot be verified for the vault")?;
            if w == 0 || h == 0 || u64::from(w) * u64::from(h) > 50_000_000 {
                return Err("Vault photos must be at most 50 megapixels".into());
            }
            source.seek(SeekFrom::Start(0)).map_err(io_error)?;
        }
        let (mut header, old) = store.state()?;
        let mut catalog = catalog(&header, &lease.master)?;
        if catalog.items.len() >= 500
            || catalog.items.iter().map(|o| o.bytes).sum::<u64>() + bytes > MAX_TOTAL
        {
            return Err("The vault supports up to 500 items and 5 GB of media".into());
        }
        if crate::video_export::available_space(&store.path)?
            .is_some_and(|free| free < bytes + bytes.div_ceil(CHUNK) * 16 + 8 * 1024 * 1024)
        {
            return Err("Not enough space to copy and verify this vault item".into());
        }
        let mut hash = Sha256::new();
        let mut buffer = Zeroizing::new(vec![0; CHUNK as usize]);
        loop {
            if !self.current(&lease, true) {
                return Err("Vault import cancelled or locked".into());
            }
            let n = source.read(&mut buffer).map_err(io_error)?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
        source.seek(SeekFrom::Start(0)).map_err(io_error)?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| !n.is_empty() && n.len() <= 512)
            .ok_or("The filename is unsupported")?
            .to_string();
        let object = Object {
            id: crypto::id()?,
            name,
            mime: mime.into(),
            bytes,
            digest: format!("{:x}", hash.finalize()),
            key: crypto::random::<32>()?.to_vec(),
        };
        let destination = store.object_path(&object.id)?;
        let target = store.ciphertext_target(&object.id)?;
        let mut output = crate::safe_output::StagedOutput::new_ciphertext(&target)?;
        encrypt_file(&mut source, output.file(), &header.id, &object, &|| {
            !self.current(&lease, true)
        })?;
        output.verify_closed(|path| {
            verify_object(&mut regular(path, false)?, &header.id, &object, &|| {
                !self.current(&lease, true)
            })
        })?;
        if !self.current(&lease, true) {
            return Err("Vault import cancelled or locked".into());
        }
        output.commit()?;
        #[cfg(test)]
        tests::crash::point("import-object-published");
        let cleanup = TemporaryFile(destination.clone());
        catalog.items.push(object);
        set_catalog(&mut header, &lease.master, &catalog)?;
        if !self.current(&lease, true) {
            return Err("Vault import cancelled or locked".into());
        }
        store.publish(&old, &header, || !self.current(&lease, true))?;
        #[cfg(test)]
        tests::crash::point("import-catalog-published");
        std::mem::forget(cleanup);
        if !self.current(&lease, false) {
            return Err("The vault locked after the encrypted import was saved. Unlock it to see the saved item.".into());
        }
        Ok(summaries(&catalog))
    }
    #[cfg(test)]
    fn watchdog(&self) -> bool {
        self.watchdog_notice().is_some()
    }
    pub(crate) fn watchdog_notice(&self) -> Option<LockNotice> {
        let expired = match self.session.lock() {
            Ok(slot) => slot.as_ref().map(|s| {
                (
                    s.epoch,
                    s.last_activity.elapsed() >= IDLE,
                    s.workspace.clone(),
                )
            }),
            Err(poison) => {
                drop(poison);
                return Some(self.lock_notice());
            }
        };
        if let Some((epoch, idle, workspace)) = expired {
            if idle || !(self.screen)() || !(self.authority)().is_ok_and(|w| w == workspace) {
                return self.revoke_if(epoch);
            }
        }
        None
    }
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Status,
    Setup {
        credential: Credential,
        confirmation: Credential,
    },
    Unlock {
        credential: Credential,
    },
    Lock,
    Revoke {
        token: String,
    },
    Touch {
        token: String,
    },
    Check {
        token: String,
    },
    List {
        token: String,
    },
    Import {
        token: String,
        path: String,
    },
    Cancel {
        token: String,
    },
    ChangePin {
        token: String,
        current: Credential,
        credential: Credential,
        confirmation: Credential,
    },
    Backup {
        token: String,
        destination: String,
    },
    Restore {
        path: String,
        credential: Credential,
    },
}
#[tauri::command]
pub async fn vault_command(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<VaultState>>,
    action: Action,
) -> Result<Reply, String> {
    let state = state.inner().clone();
    if matches!(action, Action::Lock) {
        let notice = state.lock_notice();
        use tauri::Emitter;
        let _ = app.emit("vault-locked", notice);
        return Ok(Reply {
            lock: Some(notice),
            ..Default::default()
        });
    }
    if let Action::Revoke { token } = &action {
        let notice = state.revoke_token(token);
        if let Some(notice) = notice {
            use tauri::Emitter;
            let _ = app.emit("vault-locked", notice);
        }
        return Ok(Reply {
            lock: Some(notice.unwrap_or(LockNotice { through_epoch: 0 })),
            ..Default::default()
        });
    }
    // Capture before entering the blocking/operation queues. Lock cancels queued authentication too.
    let request_epoch = state.epoch.load(Ordering::SeqCst);
    let failed_state = state.clone();
    let failed_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut reply = Reply::default();
        match action {
            Action::Status => reply.status = Some(state.status()?),
            Action::Lock|Action::Revoke{..} => unreachable!(),
            action => {
                require_available()?;
                match action {
                    Action::Setup {
                        credential,
                        confirmation,
                    } => reply.grant = Some(state.setup_at(&credential.0, &confirmation.0,request_epoch)?),
                    Action::Unlock { credential } => {
                        reply.grant = Some(state.unlock_at(&credential.0,request_epoch)?)
                    }
                    Action::Touch { token } => {
                        state.lease(&token, true)?;
                    }
                    Action::Check { token } => {
                        state.lease(&token, false)?;
                    }
                    Action::List { token } => reply.items = Some(state.list(&token)?),
                    Action::Import { token, path } => {
                        reply.items = Some(state.import(&token, Path::new(&path))?)
                    }
                    Action::Cancel { token } => {
                        state.lease(&token, false)?;
                        state.work_epoch.fetch_add(1, Ordering::SeqCst);
                    }
                    Action::ChangePin {
                        token,
                        current,
                        credential,
                        confirmation,
                    } => {
                        reply.grant = Some(state.change_pin(
                            &token,
                            &current.0,
                            &credential.0,
                            &confirmation.0,
                        )?)
                    }
                    Action::Backup { token, destination } => {
                        state.backup(&token, Path::new(&destination))?
                    }
                    Action::Restore { path, credential } => {
                        reply.grant = Some(state.restore_at(Path::new(&path), &credential.0,request_epoch)?)
                    }
                    Action::Status | Action::Lock|Action::Revoke{..} => unreachable!(),
                }
            }
        }
        Ok(reply)
    })
    .await
    .map_err(move |_| {let notice=failed_state.lock_notice();use tauri::Emitter;let _=failed_app.emit("vault-locked",notice);"The vault worker stopped unexpectedly and access was revoked. Restart Open-Photo before continuing.".to_string()})?
}
pub(crate) fn excluded_path(path: &Path) -> bool {
    let Some(base) = dirs::data_local_dir() else {
        return false;
    };
    let root = base.join("Open-Photo/private-vault-v1");
    let absolute = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let root = fs::canonicalize(&root).unwrap_or(root);
    absolute.starts_with(root)
}
#[cfg(test)]
mod tests;
