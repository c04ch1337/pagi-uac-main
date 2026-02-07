//! **Shadow_KB Secure Vault** — AES-256-GCM encrypted storage for Slot 9 (Shadow).
//!
//! Sensitive emotional data (trauma anchors, private journaling, grief weight) is
//! encrypted at rest using AES-256-GCM. Data is only decrypted in memory when a
//! session key (master passphrase) is provided. Even if the sled files are stolen,
//! the ciphertext is unreadable without the 32-byte key.
//!
//! ## Wire Format
//!
//! Each encrypted blob is stored as: `[12-byte nonce][ciphertext+tag]`.
//! The nonce is randomly generated per write via `OsRng`.
//!
//! ## Key Derivation
//!
//! The master key is read from `PAGI_SHADOW_KEY` (64 hex chars = 32 bytes).
//! If the env var is absent or malformed, the vault remains **locked** — all
//! reads/writes to Slot 9 return `Err(VaultLocked)`.
//!
//! Decrypted blobs are returned in a **memory-locked** buffer (`LockedVec`) so
//! the OS cannot swap them to disk (mlock/VirtualLock).

use crate::secure_memory::LockedVec;
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use serde::{Deserialize, Serialize};

/// AES-256-GCM nonce length (96 bits).
const NONCE_LEN: usize = 12;

/// Environment variable holding the 64-hex-char master key.
const ENV_SHADOW_KEY: &str = "PAGI_SHADOW_KEY";

/// Errors specific to the Shadow Vault.
#[derive(Debug, Clone)]
pub enum VaultError {
    /// No master key was provided — Slot 9 is locked.
    Locked,
    /// Encryption failed (should never happen with valid key + data).
    EncryptionFailed(String),
    /// Decryption failed — wrong key or corrupted blob.
    DecryptionFailed(String),
    /// The stored blob is too short to contain a valid nonce + ciphertext.
    CorruptBlob,
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Locked => write!(f, "Shadow Vault is locked (no master key provided)"),
            Self::EncryptionFailed(e) => write!(f, "Shadow Vault encryption failed: {}", e),
            Self::DecryptionFailed(e) => write!(f, "Shadow Vault decryption failed: {}", e),
            Self::CorruptBlob => write!(f, "Shadow Vault: corrupt blob (too short)"),
        }
    }
}

impl std::error::Error for VaultError {}

/// Emotional anchor record stored encrypted in the Shadow_KB (Slot 9).
///
/// These represent sensitive personal data: trauma markers, grief weight,
/// high-stress indicators, private journal entries. **Never log decrypted content.**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalAnchor {
    /// Anchor type (e.g. "high_stress", "grief", "conflict", "burnout", "private_note").
    pub anchor_type: String,
    /// Intensity on a 0.0–1.0 scale. Used by the Cognitive Governor to modulate tone.
    #[serde(default)]
    pub intensity: f32,
    /// Whether this anchor is currently active (affects compassionate routing).
    #[serde(default = "default_true")]
    pub active: bool,
    /// Anonymized label (e.g. "family_situation", "work_pressure"). No real names.
    #[serde(default)]
    pub label: String,
    /// Optional private note content. Encrypted at rest; never sent to external APIs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Unix timestamp (ms) when this anchor was created/updated.
    pub timestamp_ms: i64,
}

fn default_true() -> bool {
    true
}

impl EmotionalAnchor {
    /// Creates a new active anchor with the current timestamp.
    pub fn new(anchor_type: impl Into<String>, intensity: f32) -> Self {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        Self {
            anchor_type: anchor_type.into(),
            intensity: intensity.clamp(0.0, 1.0),
            active: true,
            label: String::new(),
            note: None,
            timestamp_ms: ts,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Serializes to JSON bytes (plaintext — will be encrypted by SecretVault).
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Deserializes from JSON bytes (after decryption).
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

/// The Secret Vault: AES-256-GCM encryption wrapper for Slot 9 (Shadow_KB).
///
/// Constructed with an optional master key. If `None`, the vault is **locked**
/// and all encrypt/decrypt operations return `VaultError::Locked`.
pub struct SecretVault {
    cipher: Option<Aes256Gcm>,
}

impl SecretVault {
    /// Creates a new vault from a 32-byte key. Pass `None` to create a locked vault.
    pub fn new(master_key: Option<&[u8; 32]>) -> Self {
        let cipher = master_key.map(|k| Aes256Gcm::new_from_slice(k).expect("key length is 32"));
        Self { cipher }
    }

    /// Attempts to create a vault from the `PAGI_SHADOW_KEY` environment variable.
    /// Returns a locked vault if the env var is missing or malformed.
    pub fn from_env() -> Self {
        let key_bytes = std::env::var(ENV_SHADOW_KEY).ok().and_then(|hex| {
            let hex = hex.trim().replace([' ', '\n'], "");
            if hex.len() != 64 {
                tracing::warn!(
                    target: "pagi::vault",
                    "PAGI_SHADOW_KEY must be 64 hex chars (32 bytes); Shadow Vault will be LOCKED"
                );
                return None;
            }
            (0..32)
                .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok())
                .collect::<Option<Vec<u8>>>()
        });
        let cipher = key_bytes.and_then(|k| {
            let arr: [u8; 32] = k.try_into().ok()?;
            Some(Aes256Gcm::new_from_slice(&arr).expect("key length is 32"))
        });
        if cipher.is_some() {
            tracing::info!(
                target: "pagi::vault",
                "🔐 Shadow Vault UNLOCKED — Slot 9 (Shadow_KB) is accessible"
            );
        } else {
            tracing::info!(
                target: "pagi::vault",
                "🔒 Shadow Vault LOCKED — Slot 9 (Shadow_KB) is inaccessible (no valid PAGI_SHADOW_KEY)"
            );
        }
        Self { cipher }
    }

    /// Returns `true` if the vault has a valid master key and can encrypt/decrypt.
    #[inline]
    pub fn is_unlocked(&self) -> bool {
        self.cipher.is_some()
    }

    /// Encrypts plaintext data into `[nonce || ciphertext]`.
    ///
    /// Returns `VaultError::Locked` if no master key is set.
    pub fn encrypt_blob(&self, data: &[u8]) -> Result<Vec<u8>, VaultError> {
        let cipher = self.cipher.as_ref().ok_or(VaultError::Locked)?;
        let nonce = Aes256Gcm::generate_nonce(OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, data)
            .map_err(|e| VaultError::EncryptionFailed(e.to_string()))?;
        let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        out.extend_from_slice(nonce.as_slice());
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    /// Decrypts a blob previously produced by `encrypt_blob`.
    /// Returns a **memory-locked** buffer (mlock/VirtualLock) so it is never swapped to disk.
    ///
    /// Returns `VaultError::Locked` if no master key, `VaultError::CorruptBlob` if too short,
    /// or `VaultError::DecryptionFailed` if the key is wrong or data is tampered.
    pub fn decrypt_blob(&self, encrypted_data: &[u8]) -> Result<LockedVec, VaultError> {
        let cipher = self.cipher.as_ref().ok_or(VaultError::Locked)?;
        if encrypted_data.len() < NONCE_LEN {
            return Err(VaultError::CorruptBlob);
        }
        let (nonce_bytes, ct) = encrypted_data.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, ct)
            .map_err(|e| VaultError::DecryptionFailed(e.to_string()))?;
        Ok(LockedVec::new(plaintext))
    }

    /// Convenience: encrypt a string (UTF-8) into a blob.
    pub fn encrypt_str(&self, data: &str) -> Result<Vec<u8>, VaultError> {
        self.encrypt_blob(data.as_bytes())
    }

    /// Convenience: decrypt a blob back to a UTF-8 string.
    /// The decrypted bytes are held in a locked buffer only during the copy; then zeroed and unlocked.
    pub fn decrypt_str(&self, encrypted_data: &[u8]) -> Result<String, VaultError> {
        let locked = self.decrypt_blob(encrypted_data)?;
        String::from_utf8(locked.as_slice().to_vec())
            .map_err(|e| VaultError::DecryptionFailed(e.to_string()))
    }

    /// Encrypts an `EmotionalAnchor` for storage in Slot 9.
    pub fn encrypt_anchor(&self, anchor: &EmotionalAnchor) -> Result<Vec<u8>, VaultError> {
        self.encrypt_blob(&anchor.to_bytes())
    }

    /// Decrypts an `EmotionalAnchor` from Slot 9 storage.
    /// Decrypted bytes are memory-locked until deserialization completes.
    pub fn decrypt_anchor(&self, encrypted_data: &[u8]) -> Result<EmotionalAnchor, VaultError> {
        let locked = self.decrypt_blob(encrypted_data)?;
        EmotionalAnchor::from_bytes(locked.as_slice()).ok_or_else(|| {
            VaultError::DecryptionFailed("failed to deserialize EmotionalAnchor".to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        // Deterministic test key (NOT for production)
        let mut key = [0u8; 32];
        for (i, b) in key.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(7).wrapping_add(42);
        }
        key
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = test_key();
        let vault = SecretVault::new(Some(&key));
        assert!(vault.is_unlocked());

        let plaintext = "This is deeply personal and sensitive data";
        let encrypted = vault.encrypt_str(plaintext).unwrap();

        // Encrypted data should NOT contain the plaintext
        let encrypted_str = String::from_utf8_lossy(&encrypted);
        assert!(!encrypted_str.contains(plaintext));

        // Decrypt should recover the original
        let decrypted = vault.decrypt_str(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn anchor_roundtrip() {
        let key = test_key();
        let vault = SecretVault::new(Some(&key));

        let anchor = EmotionalAnchor::new("high_stress", 0.85)
            .with_label("work_deadline")
            .with_note("Feeling overwhelmed with the Q4 deadline");

        let encrypted = vault.encrypt_anchor(&anchor).unwrap();
        let decrypted = vault.decrypt_anchor(&encrypted).unwrap();

        assert_eq!(decrypted.anchor_type, "high_stress");
        assert!((decrypted.intensity - 0.85).abs() < f32::EPSILON);
        assert_eq!(decrypted.label, "work_deadline");
        assert!(decrypted.active);
    }

    #[test]
    fn locked_vault_rejects_operations() {
        let vault = SecretVault::new(None);
        assert!(!vault.is_unlocked());

        assert!(matches!(
            vault.encrypt_str("test"),
            Err(VaultError::Locked)
        ));
        assert!(matches!(
            vault.decrypt_str(&[0u8; 32]),
            Err(VaultError::Locked)
        ));
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let key1 = test_key();
        let mut key2 = test_key();
        key2[0] ^= 0xFF; // flip one byte

        let vault1 = SecretVault::new(Some(&key1));
        let vault2 = SecretVault::new(Some(&key2));

        let encrypted = vault1.encrypt_str("secret data").unwrap();
        assert!(matches!(
            vault2.decrypt_str(&encrypted),
            Err(VaultError::DecryptionFailed(_))
        ));
    }

    #[test]
    fn corrupt_blob_detected() {
        let key = test_key();
        let vault = SecretVault::new(Some(&key));

        // Too short to contain nonce
        assert!(matches!(
            vault.decrypt_blob(&[1, 2, 3]),
            Err(VaultError::CorruptBlob)
        ));
    }
}
