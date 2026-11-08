//! Windows CNG/DPAPI adapters. No home-grown cipher, PRNG, KDF or PIN verifier.
use std::{mem::size_of, ptr};
use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::*};
use zeroize::{Zeroize, Zeroizing};
pub(super) const ITERATIONS: u64 = 600_000;
fn check(status: i32) -> Result<(), String> {
    if status >= 0 {
        Ok(())
    } else {
        Err("Vault cryptographic operation failed".into())
    }
}
struct Algorithm(BCRYPT_ALG_HANDLE);
impl Drop for Algorithm {
    fn drop(&mut self) {
        unsafe {
            BCryptCloseAlgorithmProvider(self.0, 0);
        }
    }
}
struct Key(BCRYPT_KEY_HANDLE);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            BCryptDestroyKey(self.0);
        }
    }
}
fn algorithm(name: windows_sys::core::PCWSTR, flags: u32) -> Result<Algorithm, String> {
    let mut handle = ptr::null_mut();
    unsafe {
        check(BCryptOpenAlgorithmProvider(
            &mut handle,
            name,
            ptr::null(),
            flags,
        ))?;
    }
    Ok(Algorithm(handle))
}
pub(super) fn random<const N: usize>() -> Result<Zeroizing<[u8; N]>, String> {
    let mut data = Zeroizing::new([0; N]);
    unsafe {
        check(BCryptGenRandom(
            ptr::null_mut(),
            data.as_mut_ptr(),
            N as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        ))?;
    }
    Ok(data)
}
pub(super) fn id() -> Result<String, String> {
    Ok(random::<16>()?.iter().map(|b| format!("{b:02x}")).collect())
}
pub(super) fn pin_key(
    pin: &str,
    salt: &[u8],
    iterations: u64,
) -> Result<Zeroizing<Vec<u8>>, String> {
    let alg = algorithm(BCRYPT_SHA256_ALGORITHM, BCRYPT_ALG_HANDLE_HMAC_FLAG)?;
    let mut key = Zeroizing::new(vec![0; 32]);
    unsafe {
        check(BCryptDeriveKeyPBKDF2(
            alg.0,
            pin.as_ptr(),
            pin.len() as u32,
            salt.as_ptr(),
            salt.len() as u32,
            iterations,
            key.as_mut_ptr(),
            32,
            0,
        ))?;
    }
    Ok(key)
}
fn crypt(
    key: &[u8],
    nonce: &[u8],
    aad: &[u8],
    input: &[u8],
    tag: &mut [u8; 16],
    encrypt: bool,
) -> Result<Zeroizing<Vec<u8>>, String> {
    if key.len() != 32 || nonce.len() != 12 || input.len() > 2 * 1024 * 1024 || aad.len() > 4096 {
        return Err("Invalid vault cipher input".into());
    }
    let alg = algorithm(BCRYPT_AES_ALGORITHM, 0)?;
    let mode: Vec<u16> = "ChainingModeGCM\0".encode_utf16().collect();
    unsafe {
        check(BCryptSetProperty(
            alg.0,
            BCRYPT_CHAINING_MODE,
            mode.as_ptr().cast(),
            (mode.len() * 2) as u32,
            0,
        ))?;
    }
    let mut handle = ptr::null_mut();
    unsafe {
        check(BCryptGenerateSymmetricKey(
            alg.0,
            &mut handle,
            ptr::null_mut(),
            0,
            key.as_ptr(),
            32,
            0,
        ))?;
    }
    let key = Key(handle);
    let mut output = Zeroizing::new(vec![0; input.len()]);
    let mut written = 0;
    let mut auth = BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO {
        cbSize: size_of::<BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO>() as u32,
        dwInfoVersion: 1,
        pbNonce: nonce.as_ptr() as *mut u8,
        cbNonce: 12,
        pbAuthData: if aad.is_empty() {
            ptr::null_mut()
        } else {
            aad.as_ptr() as *mut u8
        },
        cbAuthData: aad.len() as u32,
        pbTag: tag.as_mut_ptr(),
        cbTag: 16,
        pbMacContext: ptr::null_mut(),
        cbMacContext: 0,
        cbAAD: 0,
        cbData: 0,
        dwFlags: 0,
    };
    // SAFETY: input buffers stay live and read-only; the tag and provider information
    // have mutable storage for the documented output/in-out fields. Lengths are bounded.
    let status = unsafe {
        if encrypt {
            BCryptEncrypt(
                key.0,
                input.as_ptr(),
                input.len() as u32,
                (&mut auth as *mut BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO).cast(),
                ptr::null_mut(),
                0,
                output.as_mut_ptr(),
                output.len() as u32,
                &mut written,
                0,
            )
        } else {
            BCryptDecrypt(
                key.0,
                input.as_ptr(),
                input.len() as u32,
                (&mut auth as *mut BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO).cast(),
                ptr::null_mut(),
                0,
                output.as_mut_ptr(),
                output.len() as u32,
                &mut written,
                0,
            )
        }
    };
    check(status)?;
    if written as usize != input.len() {
        return Err("Vault cipher length mismatch".into());
    }
    Ok(output)
}
pub(super) fn encrypt(
    key: &[u8],
    nonce: &[u8],
    aad: &[u8],
    plain: &[u8],
) -> Result<Vec<u8>, String> {
    let mut tag = [0; 16];
    let cipher = crypt(key, nonce, aad, plain, &mut tag, true)?;
    let mut result = cipher.to_vec();
    result.extend_from_slice(&tag);
    Ok(result)
}
pub(super) fn decrypt(
    key: &[u8],
    nonce: &[u8],
    aad: &[u8],
    cipher: &[u8],
) -> Result<Zeroizing<Vec<u8>>, String> {
    if cipher.len() < 16 {
        return Err("Vault ciphertext is truncated".into());
    }
    let mut tag = [0; 16];
    tag.copy_from_slice(&cipher[cipher.len() - 16..]);
    crypt(
        key,
        nonce,
        aad,
        &cipher[..cipher.len() - 16],
        &mut tag,
        false,
    )
}
pub(super) fn seal(key: &[u8], aad: &[u8], plain: &[u8]) -> Result<Vec<u8>, String> {
    let nonce = random::<12>()?;
    let mut result = nonce.to_vec();
    result.extend(encrypt(key, &nonce[..], aad, plain)?);
    Ok(result)
}
pub(super) fn open(key: &[u8], aad: &[u8], cipher: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
    if cipher.len() < 28 {
        return Err("Vault ciphertext is truncated".into());
    }
    decrypt(key, &cipher[..12], aad, &cipher[12..])
}
pub(super) fn dpapi(input: &[u8], protect: bool) -> Result<Zeroizing<Vec<u8>>, String> {
    if input.is_empty() || input.len() > 16384 {
        return Err("Invalid vault key envelope".into());
    }
    let source = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    let ok = unsafe {
        if protect {
            CryptProtectData(
                &source,
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &source,
                ptr::null_mut(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if ok == 0 {
        return Err("This vault key cannot be opened by the current Windows account".into());
    }
    // DPAPI owns this allocation; copy into a zeroizing owner, clear it, then LocalFree.
    let result = unsafe {
        let bytes = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let result = Zeroizing::new(bytes.to_vec());
        bytes.zeroize();
        LocalFree(output.pbData.cast());
        result
    };
    Ok(result)
}
pub(super) fn session_unlocked() -> bool {
    use windows_sys::Win32::System::RemoteDesktop::*;
    let mut buffer = ptr::null_mut();
    let mut bytes = 0;
    unsafe {
        if WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            WTS_CURRENT_SESSION,
            WTSSessionInfoEx,
            &mut buffer,
            &mut bytes,
        ) == 0
        {
            return false;
        }
        let unlocked = if bytes as usize >= size_of::<WTSINFOEXW>() {
            let info = &*(buffer as *const WTSINFOEXW);
            info.Level == 1
                && info.Data.WTSInfoExLevel1.SessionState == WTSActive
                && info.Data.WTSInfoExLevel1.SessionFlags == WTS_SESSIONSTATE_UNLOCK as i32
        } else {
            false
        };
        WTSFreeMemory(buffer.cast());
        unlocked
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aes256_gcm_nist_vector_and_context_tampering() {
        let key = [0; 32];
        let nonce = [0; 12];
        let encrypted = encrypt(&key, &nonce, b"", &[0; 16]).unwrap();
        let hex: String = encrypted.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "cea7403d4d606b6e074ec5d3baf39d18d0d1c8a799996bf0265b98b5d48ab919"
        );
        assert_eq!(&*decrypt(&key, &nonce, b"", &encrypted).unwrap(), &[0; 16]);
        assert!(decrypt(&key, &nonce, b"wrong context", &encrypted).is_err());
        let mut changed = encrypted;
        changed[0] ^= 1;
        assert!(decrypt(&key, &nonce, b"", &changed).is_err());
    }
    #[test]
    fn pbkdf2_sha256_known_answer() {
        let key = pin_key("password", b"salt", 1).unwrap();
        let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
        );
    }
    #[test]
    fn dpapi_roundtrip_and_tamper() {
        let protected = dpapi(b"test master key", true).unwrap();
        assert_ne!(&*protected, b"test master key");
        assert_eq!(&*dpapi(&protected, false).unwrap(), b"test master key");
        let mut changed = protected;
        let index = changed.len() - 1;
        changed[index] ^= 1;
        assert!(dpapi(&changed, false).is_err());
    }
}
