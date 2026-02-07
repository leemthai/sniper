
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use crate::config::{OptimizationStrategy, PhPct, StationId};
#[cfg(debug_assertions)]
use crate::config::DF;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct UIEngineSharedData {
    pub(crate) pairs: HashSet<String>,
    pub(crate) station_overrides: HashMap<String, StationId>,
    pub(crate) ph_overrides: HashMap<String, PhPct>,
    // Add other shared configurations here as needed in the future
    pub(crate) strategy: OptimizationStrategy,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SharedConfiguration {
    inner: Arc<RwLock<UIEngineSharedData>>,
}

impl SharedConfiguration {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(UIEngineSharedData::default())),
        }
    }

    pub(crate) fn get_strategy(&self) -> OptimizationStrategy {
        self.inner.read().unwrap().strategy
    }

    pub(crate) fn set_strategy(&self, strategy: OptimizationStrategy) {
        self.inner.write().unwrap().strategy = strategy;
    }

    // --- Pair Registry ---
    // i.e write a list of pairs
    pub(crate) fn register_pairs(&self, pairs: Vec<String>) {
        let mut lock = self.inner.write().unwrap();
        for p in pairs {
            lock.pairs.insert(p);
        }
    }

    pub(crate) fn get_all_pairs(&self) -> Vec<String> {
        self.inner.read().unwrap().pairs.iter().cloned().collect()
    }

    /// Iterates through all registered pairs and ensures they have a StationId.
    pub(crate) fn ensure_all_stations_initialized(&self) {
        let mut data = self.inner.write().unwrap();
        let keys: Vec<String> = data.pairs.iter().cloned().collect();
        for pair in keys {
            data.station_overrides
                .entry(pair)
                .or_insert(StationId::default());
        }
        #[cfg(debug_assertions)]
        if DF.log_station_overrides {
            log::info!(
                "LOG_STATION_OVERRIDES ensuring all station_overrides have at least default values: {:?}",
                data
            );
        }
    }

    /// Iterates through all registered pairs and ensures they have a PH value.
    pub(crate) fn ensure_all_phs_initialized(&self, default_ph: PhPct) {
        let mut data = self.inner.write().unwrap();
        let keys: Vec<String> = data.pairs.iter().cloned().collect();
        for pair in keys {
            data.ph_overrides.entry(pair).or_insert(default_ph);
        }
        #[cfg(debug_assertions)]
        if DF.log_ph_overrides {
            log::info!(
                "LOG_PH_OVERRIDES ensuring all ph_overrides have at least default values: {:?}",
                data
            );
        }
    }

    // --- Read Accessors ---
    pub(crate) fn get_station(&self, key: &str) -> Option<StationId> {
        self.inner
            .read()
            .unwrap()
            .station_overrides
            .get(key)
            .copied()
    }

    /// impl AsRef<str>: Most flexible -  allows a single function to conveniently accept both owned Strings and borrowed &str slices
    pub(crate) fn get_station_opt(&self, key: Option<impl AsRef<str>>) -> Option<StationId> {
        key.and_then(|k| {
            self.inner
                .read()
                .unwrap()
                .station_overrides
                .get(k.as_ref())
                .copied()
        })
    }

    pub(crate) fn get_ph(&self, key: &str) -> Option<PhPct> {
        self.inner.read().unwrap().ph_overrides.get(key).copied()
    }

    // --- Write Accessors ---
    pub(crate) fn insert_station(&self, key: String, value: StationId) {
        self.inner
            .write()
            .unwrap()
            .station_overrides
            .insert(key, value);
    }

    pub(crate) fn insert_ph(&self, key: String, value: PhPct) {
        self.inner.write().unwrap().ph_overrides.insert(key, value);
    }

    // Ensure default PH if needed
    // pub fn ensure_ph_default(&self, key: String, default_value: PhPct) {
    //     self.inner
    //         .write()
    //         .unwrap()
    //         .ph_overrides
    //         .entry(key)
    //         .or_insert(default_value);
    // }

    // // Utility to get all station overrides
    // pub fn get_all_stations(&self) -> HashMap<String, StationId> {
    //     self.inner.read().unwrap().station_overrides.clone()
    // }

    // // Utility to get all PH overrides
    // pub fn get_all_phs(&self) -> HashMap<String, PhPct> {
    //     self.inner.read().unwrap().ph_overrides.clone()
    // }
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
