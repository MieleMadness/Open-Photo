//! Opaque encrypted storage. All paths are native-owned, never supplied as vault destinations.
use super::*;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use windows_sys::Win32::Storage::FileSystem::{
    MoveFileExW, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE, MOVEFILE_REPLACE_EXISTING,
    MOVEFILE_WRITE_THROUGH,
};
static METADATA_GATE: Mutex<()> = Mutex::new(());
pub(super) const CHUNK: u64 = 1024 * 1024;
const MAGIC: &[u8; 8] = b"OPVM001\0";
pub(super) const MAX_PHOTO: u64 = 64 * 1024 * 1024;
pub(super) const MAX_VIDEO: u64 = 512 * 1024 * 1024;
pub(super) const MAX_TOTAL: u64 = 5 * 1024 * 1024 * 1024;
pub(super) const MAX_HEADER: u64 = 2 * 1024 * 1024;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Header {
    pub version: u32,
    pub id: String,
    pub iterations: u64,
    pub salt: String,
    pub wrapped: String,
    pub catalog: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Object {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub bytes: u64,
    pub digest: String,
    pub key: Vec<u8>,
}
impl Drop for Object {
    fn drop(&mut self) {
        self.name.zeroize();
        self.key.zeroize();
        self.digest.zeroize();
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Catalog {
    pub version: u32,
    pub items: Vec<Object>,
}
/// Only the native vault store can construct a target for encrypted object publication.
pub(crate) struct CiphertextTarget {
    path: PathBuf,
}
impl CiphertextTarget {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}
pub(super) struct Storage {
    pub path: PathBuf,
    _directory: File,
    _lease: File,
    _parent: File,
}
fn err(error: impl std::error::Error + 'static) -> String {
    if (&error as &dyn std::error::Error)
        .downcast_ref::<std::io::Error>()
        .is_some_and(|e| e.kind() == std::io::ErrorKind::Interrupted)
    {
        return "Vault operation cancelled or locked. Existing data was kept.".into();
    }
    let code = (&error as &dyn std::error::Error)
        .downcast_ref::<std::io::Error>()
        .and_then(std::io::Error::raw_os_error)
        .unwrap_or(0);
    format!("Vault storage is unavailable or could not be verified (Windows error {code}). Existing data was kept.")
}
pub(super) fn regular(path: &Path, write: bool) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(write)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    let file = options.open(path).map_err(err)?;
    verify_regular(&file)?;
    Ok(file)
}
fn verify_regular(file: &File) -> Result<(), String> {
    let m = file.metadata().map_err(err)?;
    if !m.is_file() || m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err("Vault files must be regular files, not links".into());
    }
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    unsafe {
        if GetFileInformationByHandle(file.as_raw_handle() as _, info.as_mut_ptr()) == 0
            || info.assume_init().nNumberOfLinks != 1
        {
            return Err("Vault files must not be hard-linked".into());
        }
    }
    Ok(())
}
pub(super) fn directory(path: &Path) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(err)?;
    let m = file.metadata().map_err(err)?;
    if !m.is_dir() || m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err("Vault directories must not be links".into());
    }
    Ok(file)
}
impl Storage {
    pub fn open(path: &Path) -> Result<Self, String> {
        let parent = directory(path.parent().ok_or("Vault location unavailable")?)?;
        let dir = directory(path)?;
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path.join("lease.opv"))
            .map_err(|_| {
                "The vault is in use by another Open-Photo window or its storage is unavailable"
            })?;
        verify_regular(&lease)?;
        Ok(Self {
            path: path.into(),
            _directory: dir,
            _lease: lease,
            _parent: parent,
        })
    }
    pub fn state(&self) -> Result<(Header, Vec<u8>), String> {
        let data = read_bounded(&self.path.join("state.opv"), MAX_HEADER)?;
        let header = parse_header(&data)?;
        Ok((header, data))
    }
    pub fn publish(
        &self,
        old: &[u8],
        header: &Header,
        cancel: impl Fn() -> bool,
    ) -> Result<(), String> {
        if read_bounded(&self.path.join("state.opv"), MAX_HEADER)? != old {
            return Err("Vault state changed. Lock and reopen it before continuing.".into());
        }
        replace_until(
            &self.path.join("state.opv"),
            &serde_json::to_vec(header).map_err(err)?,
            cancel,
        )
    }
    pub(super) fn ciphertext_target(&self, id: &str) -> Result<CiphertextTarget, String> {
        Ok(CiphertextTarget {
            path: self.object_path(id)?,
        })
    }
    pub fn object_path(&self, id: &str) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err("Invalid vault object identity".into());
        }
        Ok(self.path.join(format!("{id}.opv")))
    }
}
pub(super) fn valid_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
/// Copy a complete bounded metadata snapshot while excluding atomic publishers.
/// Release the file handle before releasing the gate; crypto/decoding happen outside it.
pub(super) fn read_bounded(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    let _guard = METADATA_GATE.lock().map_err(poisoned)?;
    read_bounded_locked(path, max)
}
fn read_bounded_locked(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    let mut file = regular(path, false)?;
    if file.metadata().map_err(err)?.len() > max {
        return Err("Vault metadata exceeds its format budget".into());
    }
    let mut data = Vec::new();
    Read::by_ref(&mut file)
        .take(max + 1)
        .read_to_end(&mut data)
        .map_err(err)?;
    if data.len() as u64 > max {
        return Err("Vault metadata exceeds its format budget".into());
    }
    Ok(data)
}
pub(super) fn parse_header(data: &[u8]) -> Result<Header, String> {
    let h: Header = serde_json::from_slice(data)
        .map_err(|_| "Unsupported or damaged vault header. Preserve the vault and its backup.")?;
    if h.version != 1
        || h.iterations != crypto::ITERATIONS
        || !valid_id(&h.id)
        || decode(&h.salt)?.len() != 32
        || decode(&h.wrapped)?.len() > 16384
        || h.catalog.len() > 1_500_000
    {
        return Err("Unsupported vault format. Existing data was kept.".into());
    }
    Ok(h)
}
fn encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
fn decode(text: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|_| "Invalid vault envelope".into())
}
fn context(id: &str, kind: &str) -> Vec<u8> {
    format!("openphoto-vault-v1:{id}:{kind}").into_bytes()
}
pub(super) fn wrap(header: &mut Header, master: &[u8], pin: &str) -> Result<(), String> {
    let salt = crypto::random::<32>()?;
    let key = crypto::pin_key(pin, &salt[..], crypto::ITERATIONS)?;
    let protected = crypto::dpapi(master, true)?;
    header.salt = encode(&salt[..]);
    header.wrapped = encode(&crypto::seal(
        &key,
        &context(&header.id, "master"),
        &protected,
    )?);
    Ok(())
}
pub(super) fn unwrap(header: &Header, pin: &str) -> Result<Zeroizing<Vec<u8>>, String> {
    let key = crypto::pin_key(pin, &decode(&header.salt)?, header.iterations)?;
    let protected = crypto::open(
        &key,
        &context(&header.id, "master"),
        &decode(&header.wrapped)?,
    )
    .map_err(|_| "The PIN/passphrase is incorrect or the vault key is damaged")?;
    let master = crypto::dpapi(&protected, false)?;
    if master.len() != 32 {
        return Err("Invalid vault master key".into());
    }
    Ok(master)
}
pub(super) fn catalog(header: &Header, master: &[u8]) -> Result<Catalog, String> {
    let bytes = crypto::open(
        master,
        &context(&header.id, "catalog"),
        &decode(&header.catalog)?,
    )?;
    let catalog: Catalog =
        serde_json::from_slice(&bytes).map_err(|_| "Vault catalog could not be verified")?;
    let mut ids = std::collections::HashSet::new();
    if catalog.version != 1
        || catalog.items.len() > 500
        || catalog.items.iter().any(|o| {
            !valid_id(&o.id)
                || !ids.insert(&o.id)
                || o.name.is_empty()
                || o.name.len() > 512
                || o.key.len() != 32
                || o.bytes == 0
                || o.bytes > MAX_VIDEO
                || o.digest.len() != 64
                || !o.digest.bytes().all(|c| c.is_ascii_hexdigit())
                || !matches!(
                    o.mime.as_str(),
                    "image/jpeg"
                        | "image/png"
                        | "image/webp"
                        | "image/gif"
                        | "image/bmp"
                        | "video/mp4"
                        | "video/webm"
                )
                || (o.mime.starts_with("image/") && o.bytes > MAX_PHOTO)
        })
        || catalog.items.iter().map(|o| o.bytes).sum::<u64>() > MAX_TOTAL
    {
        return Err("Unsupported or inconsistent vault catalog".into());
    }
    Ok(catalog)
}
pub(super) fn set_catalog(
    header: &mut Header,
    master: &[u8],
    catalog: &Catalog,
) -> Result<(), String> {
    let plain = Zeroizing::new(serde_json::to_vec(catalog).map_err(err)?);
    if plain.len() > 1024 * 1024 {
        return Err("Vault catalog is full".into());
    }
    header.catalog = encode(&crypto::seal(
        master,
        &context(&header.id, "catalog"),
        &plain,
    )?);
    Ok(())
}
pub(super) fn replace(path: &Path, data: &[u8]) -> Result<(), String> {
    replace_until(path, data, || false)
}
fn replace_until(path: &Path, data: &[u8], cancel: impl Fn() -> bool) -> Result<(), String> {
    let _guard = METADATA_GATE.lock().map_err(poisoned)?;
    let parent = path.parent().ok_or("Vault location unavailable")?;
    let _parent = directory(parent)?;
    let temp = parent.join(format!(".state-{}.opv", crypto::id()?));
    let cleanup = TemporaryFile(temp.clone());
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(err)?;
    file.write_all(data).map_err(err)?;
    file.sync_all().map_err(err)?;
    drop(file);
    if read_bounded_locked(&temp, MAX_HEADER)? != data {
        return Err("Vault staged metadata failed verification".into());
    }
    // Destination is a fixed native-owned filename, under a directory held against rename/delete.
    use std::os::windows::ffi::OsStrExt;
    let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    if fs::symlink_metadata(path).is_ok() {
        drop(regular(path, false)?);
    }
    atomic_until(&cancel, || {
        if unsafe {
            MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } != 0
        {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    })
    .map_err(err)?;
    drop(cleanup);
    Ok(())
}
pub(super) struct TemporaryFile(pub PathBuf);
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
pub(super) struct TemporaryDirectory(pub PathBuf);
impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if self
            .0
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with(".vault-stage-"))
        {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
pub(super) fn staged_directory(parent: &Path) -> Result<TemporaryDirectory, String> {
    let _parent = directory(parent)?;
    let path = parent.join(format!(".vault-stage-{}", crypto::id()?));
    fs::create_dir(&path).map_err(err)?;
    Ok(TemporaryDirectory(path))
}
pub(super) fn cipher_len(object: &Object) -> u64 {
    8 + object.bytes + object.bytes.div_ceil(CHUNK) * 16
}
fn chunk_aad(vault: &str, object: &Object, index: u64) -> Vec<u8> {
    format!(
        "openphoto-vault-object-v1:{vault}:{}:{}:{index}",
        object.id, object.bytes
    )
    .into_bytes()
}
fn nonce(index: u64) -> [u8; 12] {
    let mut nonce = [0; 12];
    nonce[8..].copy_from_slice(&(index as u32).to_be_bytes());
    nonce
}
pub(super) fn encrypt_file(
    source: &mut File,
    destination: &mut File,
    vault: &str,
    object: &Object,
    cancel: &impl Fn() -> bool,
) -> Result<(), String> {
    destination.write_all(MAGIC).map_err(err)?;
    #[cfg(test)]
    tests::crash::point("import-partial-write");
    let mut buffer = Zeroizing::new(vec![0; CHUNK as usize]);
    let mut read = 0;
    while read < object.bytes {
        if cancel() {
            return Err("Vault import cancelled or locked".into());
        }
        let count = (object.bytes - read).min(CHUNK) as usize;
        source.read_exact(&mut buffer[..count]).map_err(err)?;
        let index = read / CHUNK;
        let cipher = crypto::encrypt(
            &object.key,
            &nonce(index),
            &chunk_aad(vault, object, index),
            &buffer[..count],
        )?;
        destination.write_all(&cipher).map_err(err)?;
        read += count as u64;
    }
    destination.sync_all().map_err(err)?;
    Ok(())
}
pub(super) fn read_range(
    file: &mut File,
    vault: &str,
    object: &Object,
    start: u64,
    end: u64,
    cancel: &impl Fn() -> bool,
) -> Result<Zeroizing<Vec<u8>>, String> {
    if start > end || end >= object.bytes || end - start + 1 > MAX_PHOTO {
        return Err("Vault media range exceeds its bounds".into());
    }
    if file.metadata().map_err(err)?.len() != cipher_len(object) {
        return Err("Vault media length failed verification".into());
    }
    let mut magic = [0; 8];
    file.seek(SeekFrom::Start(0)).map_err(err)?;
    file.read_exact(&mut magic).map_err(err)?;
    if &magic != MAGIC {
        return Err("Unsupported vault media format".into());
    }
    let mut result = Zeroizing::new(Vec::with_capacity((end - start + 1) as usize));
    for index in start / CHUNK..=end / CHUNK {
        if cancel() {
            return Err("Vault media request cancelled or locked".into());
        }
        let offset = index * CHUNK;
        let count = (object.bytes - offset).min(CHUNK);
        file.seek(SeekFrom::Start(8 + index * (CHUNK + 16)))
            .map_err(err)?;
        let mut cipher = vec![0; (count + 16) as usize];
        file.read_exact(&mut cipher).map_err(err)?;
        let plain = crypto::decrypt(
            &object.key,
            &nonce(index),
            &chunk_aad(vault, object, index),
            &cipher,
        )?;
        let from = start.saturating_sub(offset) as usize;
        let to = (end - offset + 1).min(count) as usize;
        result.extend_from_slice(&plain[from..to]);
    }
    if cancel() {
        return Err("Vault media request cancelled or locked".into());
    }
    Ok(result)
}
pub(super) fn verify_object(
    file: &mut File,
    vault: &str,
    object: &Object,
    cancel: &impl Fn() -> bool,
) -> Result<(), String> {
    if file.metadata().map_err(err)?.len() != cipher_len(object) {
        return Err("Vault media length failed verification".into());
    }
    file.seek(SeekFrom::Start(0)).map_err(err)?;
    verify_object_stream(file, vault, object, cancel)
}
/// Authenticate one sequential encrypted object, including its complete plaintext digest.
/// The enclosing file/archive reader owns the exact-length and EOF checks.
pub(super) fn verify_object_stream(
    source: &mut impl Read,
    vault: &str,
    object: &Object,
    cancel: &impl Fn() -> bool,
) -> Result<(), String> {
    if cancel() {
        return Err("Vault verification cancelled or locked".into());
    }
    let mut magic = [0; 8];
    source.read_exact(&mut magic).map_err(err)?;
    if &magic != MAGIC {
        return Err("Unsupported vault media format".into());
    }
    let mut hash = Sha256::new();
    let mut cipher = vec![0; (CHUNK + 16) as usize];
    for start in (0..object.bytes).step_by(CHUNK as usize) {
        if cancel() {
            return Err("Vault verification cancelled or locked".into());
        }
        let count = (object.bytes - start).min(CHUNK) as usize + 16;
        source.read_exact(&mut cipher[..count]).map_err(err)?;
        let index = start / CHUNK;
        let plain = crypto::decrypt(
            &object.key,
            &nonce(index),
            &chunk_aad(vault, object, index),
            &cipher[..count],
        )?;
        hash.update(&*plain);
    }
    if cancel() {
        return Err("Vault verification cancelled or locked".into());
    }
    if format!("{:x}", hash.finalize()) != object.digest {
        return Err("Vault media digest failed verification".into());
    }
    Ok(())
}

/// Publish an owned staged directory without replacing any file, junction or vault.
pub(super) fn publish_directory(
    staged: &TemporaryDirectory,
    destination: &Path,
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    if staged.0.parent() != destination.parent()
        || !staged
            .0
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with(".vault-stage-"))
    {
        return Err("Invalid staged vault location".into());
    }
    let _parent = directory(destination.parent().ok_or("Vault location unavailable")?)?;
    drop(directory(&staged.0)?);
    use std::os::windows::ffi::OsStrExt;
    let source: Vec<u16> = staged.0.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // No MOVEFILE_REPLACE_EXISTING: a racing destination is always preserved.
    atomic_until(&cancel,||{if unsafe{MoveFileExW(source.as_ptr(),target.as_ptr(),MOVEFILE_WRITE_THROUGH)}!=0{Ok(())}else{Err(std::io::Error::last_os_error())}}).map_err(|error|format!("The encrypted vault directory could not be published (Windows error {}). Existing vault data was not replaced.",error.raw_os_error().unwrap_or(0)))?;
    Ok(())
}
/// Retry only the same atomic syscall for bounded Windows busy/denied conditions.
/// Never retry integrity failures, overwrite collisions, crypto, or source copying.
pub(super) fn atomic_until<T>(
    cancel: &impl Fn() -> bool,
    mut operation: impl FnMut() -> std::io::Result<T>,
) -> std::io::Result<T> {
    for attempt in 0..=6 {
        if cancel() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Vault operation cancelled or locked",
            ));
        }
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) => {
                if attempt == 6 || !matches!(error.raw_os_error(), Some(5 | 32 | 33)) {
                    return Err(error);
                }
                std::thread::sleep(Duration::from_millis(5u64 << attempt));
            }
        }
    }
    unreachable!()
}
