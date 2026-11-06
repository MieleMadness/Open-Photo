//! Test-only, bounded staging copy primitive for future crossfade publication.
use std::io::{Read, Write};

/// Copy to a pre-owned temporary file with bounded buffers and cooperative
/// cancellation; never copy beyond the already-verified byte budget.
pub(crate) fn copy_verified_bytes(
    mut source: impl Read,
    mut destination: impl Write,
    expected_bytes: u64,
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    if expected_bytes == 0 || expected_bytes > crate::video_render::MAX_EXPORT_BYTES {
        return Err("Crossfade staging size exceeds the export limit.".into());
    }
    let mut copied = 0_u64;
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        if cancel() {
            return Err("Crossfade staging cancelled.".into());
        }
        let count = source.read(&mut chunk).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        copied = copied
            .checked_add(count as u64)
            .ok_or("Crossfade staging byte count overflow.")?;
        if copied > expected_bytes {
            return Err("Crossfade scratch changed or exceeded verified size.".into());
        }
        destination
            .write_all(&chunk[..count])
            .map_err(|e| e.to_string())?;
    }
    if cancel() {
        return Err("Crossfade staging cancelled.".into());
    }
    if copied != expected_bytes {
        return Err("Crossfade staged bytes did not match verified size.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        io::{self, Cursor},
    };

    #[test]
    fn copies_exact_bytes_and_rejects_truncated_or_growing_sources() {
        let mut out = Vec::new();
        copy_verified_bytes(Cursor::new(b"original"), &mut out, 8, || false).unwrap();
        assert_eq!(out, b"original");
        assert!(copy_verified_bytes(Cursor::new(b"original"), Vec::new(), 9, || false).is_err());
        assert!(copy_verified_bytes(Cursor::new(b"original"), Vec::new(), 7, || false).is_err());
        assert!(copy_verified_bytes(Cursor::new(b"original"), Vec::new(), 0, || false).is_err());
    }

    #[test]
    fn cancellation_interrupts_large_staging_before_full_copy() {
        let bytes = vec![42_u8; 256 * 1024];
        let checks = Cell::new(0);
        let mut out = Vec::new();
        let result = copy_verified_bytes(Cursor::new(&bytes), &mut out, bytes.len() as u64, || {
            checks.set(checks.get() + 1);
            checks.get() >= 3
        });
        assert!(result.is_err());
        assert!(out.len() < bytes.len());
    }

    #[test]
    fn read_failures_do_not_look_like_successful_copies() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("forced read failure"))
            }
        }
        assert!(copy_verified_bytes(Broken, Vec::new(), 10, || false).is_err());
    }
}
