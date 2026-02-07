//! Shadow KB (KB_PNEUMA_EXT): encrypted storage for "Heavy Stuff" (journal entries, emotional anchors).
//! Data is only readable when the session key is provided; never written to stdout or logs in decrypted form.
//! Decrypted buffers are memory-locked (mlock/VirtualLock) so they are never swapped to disk.

use crate::secure_memory::LockedVec;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

const KEY_LEN: usize = 32;
const ENV_SHADOW_KEY: &str = "PAGI_SHADOW_KEY";

/// Single journal or "anchor" record. Stored encrypted in the ShadowStore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalHistoryEntry {
    /// Anonymized label only (e.g. "conflict_with_person", "work_deadline"). Never store raw names here.
    pub label: String,
    /// Optional intensity 0.0–1.0 for this entry (used to update MentalState).
    #[serde(default)]
    pub intensity: f32,
    /// Unix timestamp (ms) when this entry was created.
    pub timestamp_ms: i64,
    /// Optional raw journal content. Encrypted at rest; never logged or sent to external APIs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_content: Option<String>,
}

/// Result of decrypting and reading; never log or send to external API.
pub struct DecryptedEntry(pub PersonalHistoryEntry);

/// Shadow store: encrypts before write, decrypts after read. Key from env `PAGI_SHADOW_KEY` (32 bytes hex).
/// If the key is not set, get/put are no-ops (safe degradation).
pub struct ShadowStore {
    db: sled::Db,
    cipher: Option<Aes256Gcm>,
}

impl ShadowStore {
    /// Opens the shadow DB at `path` (e.g. `./data/pagi_shadow`). Uses `PAGI_SHADOW_KEY` (64 hex chars = 32 bytes).
    pub fn open_path(path: &Path) -> Result<Self, String> {
        let db = sled::open(path).map_err(|e| format!("shadow store open: {}", e))?;
        let key_bytes = std::env::var(ENV_SHADOW_KEY).ok().and_then(|hex| {
            let hex = hex.trim().replace([' ', '\n'], "");
            if hex.len() != 64 {
                return None;
            }
            (0..32).map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()).collect::<Option<Vec<u8>>>()
        });
        let cipher = key_bytes.and_then(|k| {
            let arr: [u8; KEY_LEN] = k.try_into().ok()?;
            Some(Aes256Gcm::new_from_slice(&arr).expect("key length is 32"))
        });
        Ok(Self { db, cipher })
    }

    /// Stores a personal history entry encrypted under the tree `journal` with key `record_id`.
    /// If no key is configured, does nothing (returns Ok).
    pub fn put_journal(&self, record_id: &str, entry: &PersonalHistoryEntry) -> Result<(), String> {
        let Some(ref cipher) = self.cipher else {
            return Ok(());
        };
        let plain = serde_json::to_vec(entry).map_err(|e| format!("serialize: {}", e))?;
        let nonce = Aes256Gcm::generate_nonce(OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plain.as_ref())
            .map_err(|e| format!("encrypt: {}", e))?;
        let mut out = Vec::with_capacity(nonce.len() + ciphertext.len());
        out.extend_from_slice(nonce.as_slice());
        out.extend_from_slice(&ciphertext);
        self.db
            .open_tree("journal")
            .map_err(|e| format!("tree: {}", e))?
            .insert(record_id.as_bytes(), out)
            .map_err(|e| format!("insert: {}", e))?;
        Ok(())
    }

    /// Decrypts and returns the entry. Only call when session key is available; never log the result.
    pub fn get_journal(&self, record_id: &str) -> Result<Option<DecryptedEntry>, String> {
        let Some(ref cipher) = self.cipher else {
            return Ok(None);
        };
        let tree = self.db.open_tree("journal").map_err(|e| format!("tree: {}", e))?;
        let Some(data) = tree.get(record_id.as_bytes()).map_err(|e| format!("get: {}", e))? else {
            return Ok(None);
        };
        const NONCE_LEN: usize = 12;
        if data.len() < NONCE_LEN {
            return Err("corrupt blob".to_string());
        }
        let (nonce_slice, ct) = data.split_at(NONCE_LEN);
        let nonce = aes_gcm::Nonce::from_slice(nonce_slice);
        let plain = cipher.decrypt(nonce, ct).map_err(|e| format!("decrypt: {}", e))?;
        let locked = LockedVec::new(plain);
        let entry: PersonalHistoryEntry =
            serde_json::from_slice(locked.as_slice()).map_err(|e| format!("deserialize: {}", e))?;
        Ok(Some(DecryptedEntry(entry)))
    }
}

/// Thread-safe handle for the shadow store (optional in gateway).
pub type ShadowStoreHandle = Arc<RwLock<Option<ShadowStore>>>;
