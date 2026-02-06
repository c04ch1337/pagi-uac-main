//! Multi-layer memory: short-term cache (DashMap) and long-term Sled DB.

use crate::shared::TenantContext;
use dashmap::DashMap;
use sled::Db;
use std::path::Path;
use std::sync::Arc;

const DEFAULT_VAULT_PATH: &str = "./data/pagi_vault";

fn cache_key(ctx: &TenantContext, path: &str) -> String {
    format!("{}:{}", ctx.tenant_id, path)
}

/// Manages short-term (in-memory) cache and long-term Sled storage.
pub struct MemoryManager {
    db: Db,
    /// Hot cache: tenant-scoped path -> value. Checked before Sled.
    cache: Arc<DashMap<String, Vec<u8>>>,
}

impl MemoryManager {
    /// Opens or creates a Sled database at `./data/pagi_vault` with an in-memory cache.
    pub fn new() -> Result<Self, sled::Error> {
        Self::open_path(DEFAULT_VAULT_PATH)
    }

    /// Opens or creates a Sled database at the given path with an in-memory cache.
    pub fn open_path<P: AsRef<Path>>(path: P) -> Result<Self, sled::Error> {
        let db = sled::open(path)?;
        Ok(Self {
            db,
            cache: Arc::new(DashMap::new()),
        })
    }

    /// Persists a value at the given path. Writes to both the hot cache and Sled (long-term).
    pub fn save_path(
        &self,
        ctx: &TenantContext,
        path: &str,
        value: &[u8],
    ) -> Result<(), sled::Error> {
        let key = path.as_bytes();
        self.db.insert(key, value)?;
        self.cache.insert(cache_key(ctx, path), value.to_vec());
        Ok(())
    }

    /// Retrieves a value at the given path. Checks the hot cache first, then Sled.
    pub fn get_path(&self, ctx: &TenantContext, path: &str) -> Result<Option<Vec<u8>>, sled::Error> {
        let ck = cache_key(ctx, path);
        if let Some(v) = self.cache.get(&ck) {
            return Ok(Some(v.clone()));
        }
        let v = self.db.get(path.as_bytes())?;
        let out = v.map(|iv| iv.to_vec());
        if let Some(ref vec) = out {
            self.cache.insert(ck, vec.clone());
        }
        Ok(out)
    }
}
