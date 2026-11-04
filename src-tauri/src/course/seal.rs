//! Sealed files: answer keys, and tests/exams before their release date, are stored AES-256-GCM encrypted.
//! The key lives in the app database, so this stops casual peeking (opening the folder, searching files);
//! it is not protection against someone determined to cheat on their own course.

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rusqlite::Connection;
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

const NONCE_LEN: usize = 12;
const MAGIC: &[u8; 4] = b"SSv1";

#[derive(Clone)]
pub struct Sealer {
    cipher: Aes256Gcm,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(s: &str) -> Option<Vec<u8>> {
    (0..s.len()).step_by(2).map(|i| s.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok())).collect()
}

impl Sealer {
    /// Load the key from settings, creating one on first use.
    pub fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let key = match crate::db::get_setting(conn, "seal_key")?.and_then(|h| unhex(&h)).filter(|k| k.len() == 32) {
            Some(k) => k,
            None => {
                let k = Aes256Gcm::generate_key(OsRng).to_vec();
                crate::db::set_setting(conn, "seal_key", &hex(&k))?;
                k
            }
        };
        Ok(Self { cipher: Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key)) })
    }

    pub fn seal(&self, plain: &[u8]) -> Vec<u8> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ct = self.cipher.encrypt(&nonce, plain).expect("encryption cannot fail for in-memory data");
        [MAGIC.as_slice(), nonce.as_slice(), &ct].concat()
    }

    pub fn open(&self, data: &[u8]) -> std::io::Result<Vec<u8>> {
        if data.len() < MAGIC.len() + NONCE_LEN || &data[..4] != MAGIC {
            return Err(std::io::Error::other("not a sealed file"));
        }
        let nonce = Nonce::from_slice(&data[4..4 + NONCE_LEN]);
        self.cipher
            .decrypt(nonce, &data[4 + NONCE_LEN..])
            .map_err(|_| std::io::Error::other("sealed file is damaged or was sealed with another key"))
    }

    pub fn write_json<T: Serialize>(&self, path: &Path, value: &T, sealed: bool) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let bytes = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
        std::fs::write(path, if sealed { self.seal(&bytes) } else { bytes })
    }

    /// Reads a sealed or plain JSON file (sealed files start with a magic header).
    pub fn read_json<T: DeserializeOwned>(&self, path: &Path) -> std::io::Result<T> {
        let data = std::fs::read(path)?;
        let bytes = if data.starts_with(MAGIC) { self.open(&data)? } else { data };
        serde_json::from_slice(&bytes).map_err(std::io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_key_persists() {
        let conn = crate::db::open_in_memory().unwrap();
        let a = Sealer::load(&conn).unwrap();
        let sealed = a.seal(b"answer is 42");
        assert!(!sealed.windows(2).any(|w| w == b"42"));
        let b = Sealer::load(&conn).unwrap(); // same key from settings
        assert_eq!(b.open(&sealed).unwrap(), b"answer is 42");

        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("k.json");
        b.write_json(&p, &serde_json::json!({"x": 1}), true).unwrap();
        let v: serde_json::Value = a.read_json(&p).unwrap();
        assert_eq!(v["x"], 1);
        b.write_json(&p, &serde_json::json!({"x": 2}), false).unwrap();
        let v: serde_json::Value = a.read_json(&p).unwrap();
        assert_eq!(v["x"], 2);
    }

    #[test]
    fn rejects_tampering() {
        let conn = crate::db::open_in_memory().unwrap();
        let s = Sealer::load(&conn).unwrap();
        let mut sealed = s.seal(b"hello");
        let last = sealed.len() - 1;
        sealed[last] ^= 1;
        assert!(s.open(&sealed).is_err());
    }
}
