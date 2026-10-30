//! Documents kept next to the database: letters, scans, photos, recordings.
//! Each file is encrypted on its own with a random key (XChaCha20-Poly1305 in
//! the STREAM construction, 64 KiB chunks, so large videos never sit whole
//! in memory). The key lives in the `files` table of the encrypted database,
//! so whatever opens the database, a backup included, also opens its files.
//!
//! File layout: `SWF1`, a 19-byte nonce prefix, then the sealed chunks.

use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::stream::{DecryptorBE32, EncryptorBE32};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use sha2::{Digest, Sha256};

use crate::keys::{Key, new_key};
use crate::store::{Error, Result};

const MAGIC: &[u8; 4] = b"SWF1";
const NONCE: usize = 19;
const CHUNK: usize = 64 * 1024;
const TAG: usize = 16;

/// A file just sealed into the vault. The caller stores it in the database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed {
    pub id: String,
    pub key: Key,
    pub size: u64,
    /// SHA-256 of the plain file, hex.
    pub sha256: String,
}

/// Where a file with this id is kept. Ids are made here (UUID v7), but are
/// checked again because they come back from the UI.
pub fn path(dir: &Path, id: &str) -> Result<PathBuf> {
    if !(1..=64).contains(&id.len()) || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(Error::Io(io::Error::other(format!("odd file id {id:?}"))));
    }
    Ok(dir.join(format!("{id}.swf")))
}

/// Encrypt everything `src` yields into a new file in `dir`.
pub fn seal(dir: &Path, mut src: impl Read) -> Result<Sealed> {
    std::fs::create_dir_all(dir)?;
    let id = uuid::Uuid::now_v7().to_string();
    let key = new_key();
    let mut nonce = [0u8; NONCE];
    getrandom::fill(&mut nonce).map_err(|e| Error::Io(io::Error::other(e.to_string())))?;
    let dest = path(dir, &id)?;
    let tmp = dest.with_extension("partial");
    let result = (|| -> Result<(u64, String)> {
        let mut out = BufWriter::new(File::create(&tmp)?);
        out.write_all(MAGIC)?;
        out.write_all(&nonce)?;
        let mut enc = EncryptorBE32::from_aead(XChaCha20Poly1305::new(&key.into()), &nonce.into());
        let mut hash = Sha256::new();
        let mut size = 0u64;
        // Read one chunk ahead so the last chunk is sealed as the last.
        let mut cur = read_full(&mut src, CHUNK)?;
        loop {
            let next = if cur.len() == CHUNK {
                read_full(&mut src, CHUNK)?
            } else {
                Vec::new()
            };
            hash.update(&cur);
            size += cur.len() as u64;
            if next.is_empty() {
                let sealed = enc.encrypt_last(cur.as_slice()).map_err(|_| sealing())?;
                out.write_all(&sealed)?;
                break;
            }
            let sealed = enc.encrypt_next(cur.as_slice()).map_err(|_| sealing())?;
            out.write_all(&sealed)?;
            cur = next;
        }
        let file = out.into_inner().map_err(|e| e.into_error())?;
        file.sync_all()?;
        Ok((size, hex(&hash.finalize())))
    })();
    match result {
        Ok((size, sha256)) => {
            std::fs::rename(&tmp, &dest)?;
            Ok(Sealed {
                id,
                key,
                size,
                sha256,
            })
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Decrypt the file `id` into `out`. Fails, writing nothing more, at the
/// first chunk that does not check out. Returns the plain size.
pub fn open(dir: &Path, id: &str, key: &Key, mut out: impl Write) -> Result<u64> {
    let mut src = io::BufReader::new(File::open(path(dir, id)?)?);
    let mut head = [0u8; 4 + NONCE];
    src.read_exact(&mut head).map_err(|_| damaged())?;
    if &head[..4] != MAGIC {
        return Err(damaged());
    }
    let nonce: [u8; NONCE] = head[4..].try_into().expect("19 bytes");
    let mut dec = DecryptorBE32::from_aead(XChaCha20Poly1305::new(key.into()), &nonce.into());
    let mut size = 0u64;
    let mut cur = read_full(&mut src, CHUNK + TAG)?;
    loop {
        let next = if cur.len() == CHUNK + TAG {
            read_full(&mut src, CHUNK + TAG)?
        } else {
            Vec::new()
        };
        if next.is_empty() {
            let plain = dec.decrypt_last(cur.as_slice()).map_err(|_| damaged())?;
            out.write_all(&plain)?;
            return Ok(size + plain.len() as u64);
        }
        let plain = dec.decrypt_next(cur.as_slice()).map_err(|_| damaged())?;
        out.write_all(&plain)?;
        size += plain.len() as u64;
        cur = next;
    }
}

/// Decrypt the whole file into memory (for previews).
pub fn read(dir: &Path, id: &str, key: &Key) -> Result<Vec<u8>> {
    let mut v = Vec::new();
    open(dir, id, key, &mut v)?;
    Ok(v)
}

/// Remove a file. A file that is already gone is fine.
pub fn remove(dir: &Path, id: &str) -> Result<()> {
    match std::fs::remove_file(path(dir, id)?) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

/// Copy the sealed file `id` into `to` unless it is there already. Files
/// never change once sealed, so a copy that exists is the same file.
pub fn mirror(from: &Path, to: &Path, id: &str) -> Result<bool> {
    let dest = path(to, id)?;
    if dest.exists() {
        return Ok(false);
    }
    std::fs::create_dir_all(to)?;
    let tmp = dest.with_extension("partial");
    std::fs::copy(path(from, id)?, &tmp)?;
    std::fs::rename(&tmp, &dest)?;
    Ok(true)
}

fn read_full(src: &mut impl Read, n: usize) -> io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(n);
    src.take(n as u64).read_to_end(&mut buf)?;
    Ok(buf)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sealing() -> Error {
    Error::Io(io::Error::other("could not encrypt the file"))
}

fn damaged() -> Error {
    Error::Io(io::Error::other(
        "the file is damaged or the key does not open it",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i * 31 % 251) as u8).collect()
    }

    #[test]
    fn round_trips_every_size_around_a_chunk() {
        let dir = tempfile::tempdir().unwrap();
        for n in [0, 1, CHUNK - 1, CHUNK, CHUNK + 1, 3 * CHUNK] {
            let plain = bytes(n);
            let s = seal(dir.path(), plain.as_slice()).unwrap();
            assert_eq!(s.size, n as u64);
            assert_eq!(s.sha256, hex(&Sha256::digest(&plain)));
            assert_eq!(read(dir.path(), &s.id, &s.key).unwrap(), plain, "size {n}");
            let raw = std::fs::read(path(dir.path(), &s.id).unwrap()).unwrap();
            assert!(
                n < 16 || !raw.windows(16).any(|w| w == &plain[..16]),
                "plain text on disk"
            );
        }
    }

    #[test]
    fn refuses_a_wrong_key_a_changed_byte_and_a_cut_file() {
        let dir = tempfile::tempdir().unwrap();
        let s = seal(dir.path(), bytes(2 * CHUNK + 10).as_slice()).unwrap();
        assert!(read(dir.path(), &s.id, &new_key()).is_err());
        let p = path(dir.path(), &s.id).unwrap();
        let good = std::fs::read(&p).unwrap();
        let mut bad = good.clone();
        bad[4 + NONCE + 100] ^= 1;
        std::fs::write(&p, &bad).unwrap();
        assert!(read(dir.path(), &s.id, &s.key).is_err());
        // Dropping the last chunk must not pass as a shorter file.
        std::fs::write(&p, &good[..4 + NONCE + 2 * (CHUNK + TAG)]).unwrap();
        assert!(read(dir.path(), &s.id, &s.key).is_err());
    }

    #[test]
    fn ids_cannot_leave_the_folder_and_mirror_copies_once() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        assert!(path(a.path(), "../x").is_err() && path(a.path(), "").is_err());
        let s = seal(a.path(), &b"letter"[..]).unwrap();
        assert!(mirror(a.path(), b.path(), &s.id).unwrap());
        assert!(!mirror(a.path(), b.path(), &s.id).unwrap());
        assert_eq!(read(b.path(), &s.id, &s.key).unwrap(), b"letter");
        remove(a.path(), &s.id).unwrap();
        remove(a.path(), &s.id).unwrap();
    }
}
