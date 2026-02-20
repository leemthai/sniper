use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::models::OhlcvTimeSeries;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct TimeSeriesCollection {
    pub name: String,
    pub version: f64,
    pub series_data: Vec<OhlcvTimeSeries>,
}

impl TimeSeriesCollection {
    pub fn unique_pair_names(&self) -> Vec<String> {
        self.series_data
            .iter()
            .map(|ts| ts.pair_interval.name().to_string())
            .collect::<BTreeSet<_>>() // Sorts and deduplicates
            .into_iter()
            .collect()
    }
}
