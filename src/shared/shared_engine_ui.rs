use crate::config::StationId;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Default)]
pub struct SharedStationMap {
    // Arc lets us share ownership. RwLock lets us read/write safely.
    inner: Arc<RwLock<HashMap<String, StationId>>>,
}

impl SharedStationMap {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // Helper to get a value without manually locking everywhere
    pub fn get(&self, key: &str) -> Option<StationId> {
        self.inner.read().unwrap().get(key).copied()
    }

    // Helper to write
    pub fn insert(&self, key: String, value: StationId) {
        self.inner.write().unwrap().insert(key, value);
    }

    // For the loop logic you asked about earlier
    pub fn ensure_default(&self, key: String) {
        self.inner
            .write()
            .unwrap()
            .entry(key)
            .or_insert(StationId::default());
    }
}

// --- SERDE MAGIC ---
// This makes the Arc<RwLock> invisible to the save file.
// It saves as a plain HashMap.
impl Serialize for SharedStationMap {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Lock for reading, then serialize the inner map directly
        self.inner.read().unwrap().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SharedStationMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize into a plain HashMap, then wrap it
        let map = HashMap::<String, StationId>::deserialize(deserializer)?;
        Ok(Self {
            inner: Arc::new(RwLock::new(map)),
        })
    }
}
