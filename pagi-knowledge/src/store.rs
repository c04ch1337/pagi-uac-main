//! Sled-backed store with one tree per KB slot (kb1–kb8).

use sled::Db;
use std::path::Path;

const DEFAULT_PATH: &str = "./data/pagi_knowledge";
const TREE_NAMES: [&str; 8] = [
    "kb1_marketing",
    "kb2_sales",
    "kb3_finance",
    "kb4_operations",
    "kb5_community",
    "kb6_products",
    "kb7_policies",
    "kb8_custom",
];

/// Store with 8 Sled trees, one per knowledge base slot.
pub struct KnowledgeStore {
    db: Db,
}

impl KnowledgeStore {
    /// Opens or creates the knowledge DB at `./data/pagi_knowledge`.
    pub fn new() -> Result<Self, sled::Error> {
        Self::open_path(DEFAULT_PATH)
    }

    /// Opens or creates the knowledge DB at the given path.
    pub fn open_path<P: AsRef<Path>>(path: P) -> Result<Self, sled::Error> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    fn tree_name(slot_id: u8) -> &'static str {
        if (1..=8).contains(&slot_id) {
            TREE_NAMES[slot_id as usize - 1]
        } else {
            TREE_NAMES[0]
        }
    }

    /// Returns the value at `key` in the tree for `slot_id` (1–8).
    pub fn get(&self, slot_id: u8, key: &str) -> Result<Option<Vec<u8>>, sled::Error> {
        let tree = self.db.open_tree(Self::tree_name(slot_id))?;
        let v = tree.get(key.as_bytes())?;
        Ok(v.map(|iv| iv.to_vec()))
    }

    /// Inserts `value` at `key` in the tree for `slot_id` (1–8).
    pub fn insert(
        &self,
        slot_id: u8,
        key: &str,
        value: &[u8],
    ) -> Result<Option<Vec<u8>>, sled::Error> {
        let tree = self.db.open_tree(Self::tree_name(slot_id))?;
        let prev = tree.insert(key.as_bytes(), value)?;
        Ok(prev.map(|iv| iv.to_vec()))
    }

    /// Removes the key in the tree for `slot_id` (1–8). Returns the previous value if present.
    pub fn remove(&self, slot_id: u8, key: &str) -> Result<Option<Vec<u8>>, sled::Error> {
        let tree = self.db.open_tree(Self::tree_name(slot_id))?;
        let prev = tree.remove(key.as_bytes())?;
        Ok(prev.map(|iv| iv.to_vec()))
    }

    /// Returns all keys in the tree for `slot_id` (1–8). Order is not guaranteed.
    pub fn scan_keys(&self, slot_id: u8) -> Result<Vec<String>, sled::Error> {
        let tree = self.db.open_tree(Self::tree_name(slot_id))?;
        let keys: Vec<String> = tree
            .iter()
            .keys()
            .filter_map(|k| k.ok())
            .filter_map(|k| String::from_utf8(k.to_vec()).ok())
            .collect();
        Ok(keys)
    }
}
