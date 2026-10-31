use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_DISK_ITEMS: usize = 256;
const MAX_CACHED_BYTES: usize = 8 * 1024 * 1024;

fn directory() -> Option<PathBuf> {
    let path = dirs::cache_dir()?.join("Open-Photo").join("previews-v1");
    fs::create_dir_all(&path).ok()?;
    Some(path)
}

fn location(key: &str) -> Option<PathBuf> {
    let digest = Sha256::digest(key.as_bytes());
    let name: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    Some(directory()?.join(format!("{name}.png")))
}

pub fn read(key: &str) -> Option<Vec<u8>> {
    let path = location(key)?;
    let metadata = fs::metadata(&path).ok()?;
    if metadata.len() > MAX_CACHED_BYTES as u64 {
        return None;
    }
    fs::read(path).ok()
}

pub fn write(key: &str, bytes: &[u8]) {
    if bytes.len() > MAX_CACHED_BYTES {
        return;
    }
    let Some(path) = location(key) else {
        return;
    };
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_nanos());
    let temporary = path.with_extension(format!("{}.{}.tmp", std::process::id(), timestamp));
    if fs::write(&temporary, bytes).is_err() {
        return;
    }
    if fs::rename(&temporary, &path).is_err() {
        let _ = fs::remove_file(&temporary);
    }
    prune();
}

fn prune() {
    let Some(directory) = directory() else {
        return;
    };
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut files: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("png") {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect();
    if files.len() <= MAX_DISK_ITEMS {
        return;
    }
    files.sort_by_key(|(modified, _)| *modified);
    let excess = files.len() - MAX_DISK_ITEMS;
    for (_, path) in files.into_iter().take(excess) {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_previews_are_not_cached() {
        let key = format!("too-large-{}", std::process::id());
        write(&key, &vec![0_u8; MAX_CACHED_BYTES + 1]);
        assert!(read(&key).is_none());
    }
}
