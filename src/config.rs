//! config.json next to the program (database connection). The file content is encrypted.
//!
//! Note: the key is built into the program, so this keeps the password away from casual readers
//! (text editors, backups, screenshots) but is not a defence against someone who has the program.
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

const PREFIX: &str = "RMENC1:";

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
}

impl Default for Config {
    fn default() -> Self {
        Config { host: "127.0.0.1".into(), port: 3307, user: "root".into(), password: String::new(), database: "radiomoon".into() }
    }
}

impl Config {
    pub fn is_local(&self) -> bool {
        self.host == "127.0.0.1" || self.host == "localhost"
    }
}

fn cipher() -> Aes256Gcm {
    let mut h = Sha256::new();
    h.update(b"RadioMoon::config::v1::7f3a91c4-5d2e-4b8a-9e61-0c1d77a2b3f9");
    let key = h.finalize();
    Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key))
}

pub fn encrypt(plain: &str) -> String {
    let mut nonce = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce);
    let ct = cipher().encrypt(Nonce::from_slice(&nonce), plain.as_bytes()).unwrap_or_default();
    let mut blob = nonce.to_vec();
    blob.extend_from_slice(&ct);
    format!("{PREFIX}{}", B64.encode(blob))
}

pub fn decrypt(text: &str) -> Option<String> {
    let b = B64.decode(text.trim().strip_prefix(PREFIX)?).ok()?;
    if b.len() < 13 {
        return None;
    }
    let (nonce, ct) = b.split_at(12);
    let plain = cipher().decrypt(Nonce::from_slice(nonce), ct).ok()?;
    String::from_utf8(plain).ok()
}

pub fn save(base: &Path, c: &Config) {
    if let Ok(js) = serde_json::to_string(c) {
        let _ = std::fs::write(base.join("config.json"), encrypt(&js));
    }
}

pub fn load(base: &Path) -> Config {
    let p = base.join("config.json");
    if let Ok(s) = std::fs::read_to_string(&p) {
        if s.trim_start().starts_with(PREFIX) {
            if let Some(js) = decrypt(&s) {
                if let Ok(c) = serde_json::from_str::<Config>(&js) {
                    return c;
                }
            }
        } else if let Ok(c) = serde_json::from_str::<Config>(&s) {
            // an old plain-text file: encrypt it now
            save(base, &c);
            return c;
        }
    }
    let c = Config::default();
    save(base, &c);
    c
}
