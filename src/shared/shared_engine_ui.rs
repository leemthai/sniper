use crate::config::{PhPct, StationId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UIEngineSharedData {
    pub pairs: HashSet<String>,
    pub station_overrides: HashMap<String, StationId>,
    pub ph_overrides: HashMap<String, PhPct>,
    // Add other shared configurations here as needed in the future
}

#[derive(Debug, Clone, Default)]
pub struct SharedConfiguration {
    inner: Arc<RwLock<UIEngineSharedData>>,
}

impl SharedConfiguration {
    pub fn new() -> Self {
        Self { inner: Arc::new(RwLock::new(UIEngineSharedData::default())) }
    }

    // --- Pair Registry ---
    // i.e write a list of pairs
    pub fn register_pairs(&self, pairs: Vec<String>) {
        let mut lock = self.inner.write().unwrap();
        for p in pairs {
            lock.pairs.insert(p);
        }
    }

    pub fn get_pair_count(&self) -> usize {
        self.inner.read().unwrap().pairs.len()
    }

    pub fn get_all_pairs(&self) -> Vec<String> {
        self.inner.read().unwrap().pairs.iter().cloned().collect()
    }

    /// Iterates through all registered pairs and ensures they have a StationId.
    pub fn ensure_all_stations_initialized(&self) {
        let mut data = self.inner.write().unwrap();
        let keys: Vec<String> = data.pairs.iter().cloned().collect();
        for pair in keys {
            data.station_overrides
                .entry(pair)
                .or_insert(StationId::default());
        }
    }

    /// Iterates through all registered pairs and ensures they have a PH value.
    pub fn ensure_all_phs_initialized(&self, default_ph: PhPct) {
        let mut data = self.inner.write().unwrap();
        let keys: Vec<String> = data.pairs.iter().cloned().collect();
        for pair in keys {
            data.ph_overrides.entry(pair).or_insert(default_ph);
        }
    }

    // --- Read Accessors ---
    pub fn get_station(&self, key: &str) -> Option<StationId> {
        self.inner.read().unwrap().station_overrides.get(key).copied()
    }

    pub fn get_ph(&self, key: &str) -> Option<PhPct> {
        self.inner.read().unwrap().ph_overrides.get(key).copied()
    }

    // --- Write Accessors ---
    pub fn insert_station(&self, key: String, value: StationId) {
        self.inner.write().unwrap().station_overrides.insert(key, value);
    }

    pub fn insert_ph(&self, key: String, value: PhPct) {
        self.inner.write().unwrap().ph_overrides.insert(key, value);
    }

    // Ensure default for station
    pub fn ensure_station_default(&self, key: String) {
        self.inner
            .write()
            .unwrap()
            .station_overrides
            .entry(key)
            .or_insert(StationId::default());
    }
    
    // Ensure default PH if needed
    pub fn ensure_ph_default(&self, key: String, default_value: PhPct) {
        self.inner.write().unwrap().ph_overrides.entry(key).or_insert(default_value);
    }
    
    // Utility to get all station overrides
    pub fn get_all_stations(&self) -> HashMap<String, StationId> {
        self.inner.read().unwrap().station_overrides.clone()
    }
    
    // Utility to get all PH overrides
    pub fn get_all_phs(&self) -> HashMap<String, PhPct> {
        self.inner.read().unwrap().ph_overrides.clone()
    }
}

// --- SERDE MAGIC ---
impl Serialize for SharedConfiguration {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.inner.read().unwrap().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SharedConfiguration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = UIEngineSharedData::deserialize(deserializer)?;
        Ok(Self {
            inner: Arc::new(RwLock::new(data)),
        })
    }
}
