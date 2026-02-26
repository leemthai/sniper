use {
    crate::config::{PhPct, StationId},
    crate::models::OptimizationStrategy,
    serde::{Deserialize, Deserializer, Serialize, Serializer},
    std::{
        collections::HashMap,
        sync::{Arc, RwLock},
    },
};

#[cfg(debug_assertions)]
use crate::config::DF;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct UIEngineSharedData {
    pub(crate) station_overrides: HashMap<String, StationId>,
    pub(crate) ph_overrides: HashMap<String, PhPct>,
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

    pub(crate) fn ensure_all_stations_initialized(&self, pairs: &[String]) {
        let mut data = self.inner.write().unwrap();
        for pair in pairs {
            data.station_overrides.entry(pair.clone()).or_default();
        }
        #[cfg(debug_assertions)]
        if DF.log_station_overrides {
            log::info!(
                "LOG_STATION_OVERRIDES ensuring all station_overrides have at least default values: {:?}",
                data
            );
        }
    }

    pub(crate) fn ensure_all_phs_initialized(&self, pairs: &[String], default_ph: PhPct) {
        let mut data = self.inner.write().unwrap();
        for pair in pairs {
            data.ph_overrides.entry(pair.clone()).or_insert(default_ph);
        }
        #[cfg(debug_assertions)]
        if DF.log_ph_overrides {
            log::info!(
                "LOG_PH_OVERRIDES ensuring all ph_overrides have at least default values: {:?}",
                data
            );
        }
    }

    pub(crate) fn get_station(&self, key: &str) -> Option<StationId> {
        self.inner
            .read()
            .unwrap()
            .station_overrides
            .get(key)
            .copied()
    }

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
}

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
