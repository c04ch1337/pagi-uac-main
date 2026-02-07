//! Integration test: Shadow_KB (Slot 9) — AES-256-GCM encrypted storage.
//!
//! Verifies that:
//! 1. Data written to Slot 9 is automatically encrypted (ciphertext ≠ plaintext).
//! 2. Data read back from Slot 9 can be decrypted to the original.
//! 3. A locked vault rejects Slot 9 writes.
//! 4. Compassionate routing (`check_mental_load`) detects active anchors.

use pagi_core::{EmotionalAnchor, KnowledgeStore, KbType};

/// Deterministic test key (32 bytes). NOT for production.
fn test_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    for (i, b) in key.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(7).wrapping_add(42);
    }
    key
}

#[test]
fn slot9_encrypt_decrypt_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let key = test_key();
    let store = KnowledgeStore::open_with_key(dir.path(), Some(&key)).unwrap();

    assert!(store.is_shadow_unlocked(), "vault should be unlocked with key");

    // Write a sensitive note to Slot 9
    let sensitive = "I am feeling overwhelmed by the Q4 deadline and family issues";
    store
        .insert(KbType::Shadow.slot_id(), "test/note", sensitive.as_bytes())
        .expect("insert to Slot 9 should succeed");

    // Read back the RAW bytes from sled (bypassing decryption)
    let raw = store
        .get(KbType::Shadow.slot_id(), "test/note")
        .expect("get should succeed")
        .expect("key should exist");

    // The raw bytes should NOT contain the plaintext (it's encrypted)
    let raw_str = String::from_utf8_lossy(&raw);
    assert!(
        !raw_str.contains("overwhelmed"),
        "Raw sled data should be encrypted ciphertext, not plaintext"
    );
    assert!(
        raw.len() > sensitive.len(),
        "Encrypted data should be larger than plaintext (nonce + tag overhead)"
    );

    // Decrypt via the vault
    let decrypted = store
        .get_shadow_decrypted("test/note")
        .expect("decrypt should succeed")
        .expect("key should exist");
    assert_eq!(decrypted, sensitive, "Decrypted text should match original");
}

#[test]
fn slot9_anchor_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let key = test_key();
    let store = KnowledgeStore::open_with_key(dir.path(), Some(&key)).unwrap();

    let anchor = EmotionalAnchor::new("high_stress", 0.85)
        .with_label("work_deadline")
        .with_note("Feeling overwhelmed with the Q4 deadline");

    store
        .insert_shadow_anchor("anchor/work_deadline", &anchor)
        .expect("insert anchor should succeed");

    let retrieved = store
        .get_shadow_anchor("anchor/work_deadline")
        .expect("get anchor should succeed")
        .expect("anchor should exist");

    assert_eq!(retrieved.anchor_type, "high_stress");
    assert!((retrieved.intensity - 0.85).abs() < f32::EPSILON);
    assert_eq!(retrieved.label, "work_deadline");
    assert!(retrieved.active);
    assert_eq!(
        retrieved.note.as_deref(),
        Some("Feeling overwhelmed with the Q4 deadline")
    );
}

#[test]
fn slot9_locked_vault_rejects_writes() {
    let dir = tempfile::tempdir().unwrap();
    // No key = locked vault
    let store = KnowledgeStore::open_with_key(dir.path(), None).unwrap();

    assert!(!store.is_shadow_unlocked(), "vault should be locked without key");

    let result = store.insert(KbType::Shadow.slot_id(), "test/locked", b"secret");
    assert!(result.is_err(), "Slot 9 write should fail when vault is locked");
}

#[test]
fn compassionate_routing_detects_anchors() {
    let dir = tempfile::tempdir().unwrap();
    let key = test_key();
    let store = KnowledgeStore::open_with_key(dir.path(), Some(&key)).unwrap();

    // No anchors → no compassionate instruction
    assert!(
        store.check_mental_load().is_none(),
        "No anchors should mean no compassionate routing"
    );

    // Add a high-intensity anchor
    let anchor = EmotionalAnchor::new("high_stress", 0.9)
        .with_label("family_situation");
    store
        .insert_shadow_anchor("anchor/family_situation", &anchor)
        .unwrap();

    // Now check_mental_load should return a compassionate instruction
    let instruction = store.check_mental_load();
    assert!(
        instruction.is_some(),
        "Active high-intensity anchor should trigger compassionate routing"
    );
    let text = instruction.unwrap();
    assert!(
        text.contains("supportive"),
        "Instruction should mention being supportive: {}",
        text
    );
    assert!(
        text.contains("high_stress"),
        "Instruction should mention the anchor type: {}",
        text
    );
}

#[test]
fn compassionate_routing_ignores_low_intensity() {
    let dir = tempfile::tempdir().unwrap();
    let key = test_key();
    let store = KnowledgeStore::open_with_key(dir.path(), Some(&key)).unwrap();

    // Add a low-intensity anchor (below 0.5 threshold)
    let anchor = EmotionalAnchor::new("mild_concern", 0.3)
        .with_label("minor_issue");
    store
        .insert_shadow_anchor("anchor/minor_issue", &anchor)
        .unwrap();

    // Low intensity should NOT trigger compassionate routing
    assert!(
        store.check_mental_load().is_none(),
        "Low-intensity anchor should not trigger compassionate routing"
    );
}

#[test]
fn slot9_data_unreadable_without_key() {
    let dir = tempfile::tempdir().unwrap();
    let key = test_key();

    // Write with key
    {
        let store = KnowledgeStore::open_with_key(dir.path(), Some(&key)).unwrap();
        store
            .insert(KbType::Shadow.slot_id(), "secret/data", b"top secret info")
            .unwrap();
    }

    // Open without key — vault is locked
    {
        let store = KnowledgeStore::open_with_key(dir.path(), None).unwrap();
        // Raw get still works (returns encrypted bytes)
        let raw = store
            .get(KbType::Shadow.slot_id(), "secret/data")
            .unwrap()
            .expect("key should exist in sled");
        let raw_str = String::from_utf8_lossy(&raw);
        assert!(
            !raw_str.contains("top secret"),
            "Raw data should be encrypted"
        );

        // But decryption should fail
        let result = store.get_shadow_decrypted("secret/data");
        assert!(result.is_err(), "Decryption should fail without key");
    }

    // Open with wrong key — decryption should fail
    {
        let mut wrong_key = test_key();
        wrong_key[0] ^= 0xFF;
        let store = KnowledgeStore::open_with_key(dir.path(), Some(&wrong_key)).unwrap();
        let result = store.get_shadow_decrypted("secret/data");
        assert!(result.is_err(), "Decryption should fail with wrong key");
    }
}
