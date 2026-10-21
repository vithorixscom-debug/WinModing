use std::fs::File;
use std::io::Read;
use std::path::Path;

use rayon::prelude::*;
use sha1::{Digest, Sha1};
use sha2::{Sha256, Sha512};

use crate::domain::{ContentHash, HashAlgo};
use crate::error::LauncherError;

pub fn digest_bytes(bytes: &[u8], algo: HashAlgo) -> String {
    match algo {
        HashAlgo::Sha1 => hex::encode(Sha1::digest(bytes)),
        HashAlgo::Sha256 => hex::encode(Sha256::digest(bytes)),
        HashAlgo::Sha512 => hex::encode(Sha512::digest(bytes)),
    }
}

pub fn verify_bytes(
    bytes: &[u8],
    expected: &ContentHash,
    label: &str,
) -> Result<(), LauncherError> {
    let actual = digest_bytes(bytes, expected.algo);
    if actual != expected.hex {
        return Err(LauncherError::HashMismatch {
            path: label.to_string(),
            expected: expected.hex.clone(),
            actual,
        });
    }
    Ok(())
}

pub fn verify_file(path: &Path, expected: &ContentHash) -> Result<(), LauncherError> {
    let mut file = File::open(path).map_err(|err| crate::error::io_at(path, err))?;
    let mut hasher_sha1 = Sha1::new();
    let mut hasher_sha256 = Sha256::new();
    let mut hasher_sha512 = Sha512::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| crate::error::io_at(path, err))?;
        if read == 0 {
            break;
        }
        let chunk = &buffer[..read];
        match expected.algo {
            HashAlgo::Sha1 => hasher_sha1.update(chunk),
            HashAlgo::Sha256 => hasher_sha256.update(chunk),
            HashAlgo::Sha512 => hasher_sha512.update(chunk),
        }
    }
    let actual = match expected.algo {
        HashAlgo::Sha1 => hex::encode(hasher_sha1.finalize()),
        HashAlgo::Sha256 => hex::encode(hasher_sha256.finalize()),
        HashAlgo::Sha512 => hex::encode(hasher_sha512.finalize()),
    };
    if actual != expected.hex {
        return Err(LauncherError::HashMismatch {
            path: path.display().to_string(),
            expected: expected.hex.clone(),
            actual,
        });
    }
    Ok(())
}

/// Hash many files on a blocking rayon pool. Call from `spawn_blocking`.
pub fn verify_files_parallel(
    items: &[(std::path::PathBuf, ContentHash)],
) -> Result<(), LauncherError> {
    let first_error = items
        .par_iter()
        .find_map_any(|(path, hash)| match verify_file(path, hash) {
            Ok(()) => None,
            Err(err) => Some(err),
        });
    match first_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_of_abc_matches_known_vector() {
        let hash = digest_bytes(b"abc", HashAlgo::Sha1);
        assert_eq!(hash, "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn verify_bytes_accepts_matching_sha1() {
        let expected = ContentHash::sha1("a9993e364706816aba3e25717850c26c9cd0d89d");
        assert!(verify_bytes(b"abc", &expected, "demo.jar").is_ok());
    }

    #[test]
    fn mismatch_names_both_digests() {
        let expected = ContentHash::sha1("deadbeef");
        let err = verify_bytes(b"abc", &expected, "demo.jar").unwrap_err();
        assert_eq!(err.code(), "hash_mismatch");
    }

    #[test]
    fn sha256_mismatch_is_rejected() {
        let expected = ContentHash::sha256("00");
        let err = verify_bytes(b"abc", &expected, "demo.jar").unwrap_err();
        assert_eq!(err.code(), "hash_mismatch");
    }

    #[test]
    fn verify_file_rejects_wrong_hash() {
        let dir = std::env::temp_dir().join(format!("winmoding-hash-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.bin");
        std::fs::write(&path, b"abc").unwrap();
        let expected = ContentHash::sha1("deadbeef");
        let err = verify_file(&path, &expected).unwrap_err();
        assert_eq!(err.code(), "hash_mismatch");
        let _ = std::fs::remove_dir_all(dir);
    }
}
