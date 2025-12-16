// Define the structure to hold the data, specifically for f64
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt;

use crate::models::cva::{CVACore, ScoreType};
use crate::utils::maths_utils;

#[derive(Debug)]
pub struct SelectionResults {
    pub indices: Vec<usize>,
    // You could add metadata here if needed to store data source info, e.g., source_id: u32
}

// Implement Display for a cleaner, comma-separated list of indices
impl fmt::Display for SelectionResults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Indices: [")?;
        for (i, index) in self.indices.iter().enumerate() {
            write!(f, "{}", index)?;
            if i < self.indices.len() - 1 {
                write!(f, ", ")?;
            }
        }
        write!(f, "]")
    }
}
pub struct DataSelector<'a> {
    data: &'a [f64],
}

impl<'a> DataSelector<'a> {
    pub fn new(data: &'a [f64]) -> Self {
        DataSelector { data }
    }

    /// Selects indices of data points within a specified percentile range.
    /// Selects indices of data points within a specified percentile range.
    /// `lower_proportion`: The inclusive lower bound of the percentile range (0.0 to 1.0).
    /// `upper_proportion`: The exclusive upper bound of the percentile range (0.0 to 1.0).
    /// For example:
    /// - To pick the bottom 5%: `lower_proportion = 0.0`, `upper_proportion = 0.05`
    /// - To pick the middle 10%: `lower_proportion = 0.45`, `upper_proportion = 0.55`
    /// - To pick the top 10%: `lower_proportion = 0.90`, `upper_proportion = 1.0`
    pub fn select_percentile_range(
        &self,
        lower_proportion: f64,
        upper_proportion: f64,
    ) -> Vec<usize> {
        // Ensure valid percentile range
        let lower_proportion = lower_proportion.clamp(0.0, 1.0);
        let upper_proportion = upper_proportion.clamp(0.0, 1.0);

        if lower_proportion >= upper_proportion {
            return Vec::new(); // Invalid or empty range
        }

        let total_count = self.data.len();
        if total_count == 0 {
            return Vec::new();
        }

        // 1. Create a sorted list of (original_index, value) pairs
        let mut indexed_data: Vec<(usize, f64)> =
            self.data.iter().enumerate().map(|(i, &v)| (i, v)).collect();

        // Sort in ascending order (a.1 vs b.1) to easily pick from any percentile
        indexed_data.sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        // 2. Calculate the start and end indices based on proportions
        // The index for a proportion P is P * total_count
        let start_index = ((total_count as f64) * lower_proportion) as usize;
        let end_index = ((total_count as f64) * upper_proportion) as usize;

        // Ensure indices are within bounds
        let start_index = start_index.min(total_count);
        let end_index = end_index.min(total_count);

        // 3. Take the slice of items within the calculated index range and collect their original indices
        indexed_data[start_index..end_index]
            .iter()
            .map(|(i, _v)| *i)
            .collect()
    }

    // Helper method to find the max value
    fn get_max_val(&self) -> Option<f64> {
        self.data
            .iter()
            .copied()
            .filter(|x| !x.is_nan())
            .max_by(|a, b| a.partial_cmp(b).unwrap())
    }

    fn select_above_value_threshold(&self, threshold: f64) -> Vec<usize> {
        self.data
            .iter()
            .enumerate()
            .filter(|&(_, &val)| val > threshold)
            .map(|(idx, _)| idx)
            .collect()
    }

    fn select_below_value_threshold(&self, threshold: f64) -> Vec<usize> {
        self.data
            .iter()
            .enumerate()
            .filter(|&(_, &val)| val < threshold)
            .map(|(idx, _)| idx)
            .collect()
    }

    fn select_relative_to_max_proportion(&self, proportion: f64) -> Vec<usize> {
        if let Some(max_val) = self.get_max_val() {
            // Ensure proportion is within 0.0 and 1.0
            let clamped_proportion = proportion.clamp(0.0, 1.0);
            let lower_bound = max_val * (1.0 - clamped_proportion);
            return self
                .data
                .iter()
                .enumerate()
                .filter(|&(_, &val)| val >= lower_bound && val <= max_val)
                .map(|(idx, _)| idx)
                .collect();
        }
        Vec::new()
    }

    // Select top n of indices (not top n %)
    pub fn select_top_n(&self, top_n: usize) -> Vec<usize> {
        let total_count = self.data.len();
        let actual_top_n = std::cmp::min(top_n, total_count);

        let mut indexed_data: Vec<(usize, f64)> =
            self.data.iter().enumerate().map(|(i, &v)| (i, v)).collect();

        indexed_data.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

        indexed_data
            .into_iter()
            .take(actual_top_n)
            .map(|(i, _v)| i)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
#[allow(dead_code)]
pub enum ZoneSelectionCriteria {
    AboveValueThreshold(f64),
    BelowValueThreshold(f64),
    RelativeToMaxProportion(f64),
    TopN(usize),
    PercentileRange(f64, f64),
}

impl ZoneSelectionCriteria {
    // A single method on the ENUM itself to perform the selection
    // It takes the data container as an argument.
    pub fn select(&self, data_source: &[f64]) -> SelectionResults {
        let selection_helper = DataSelector::new(data_source);
        let indices = match self {
            ZoneSelectionCriteria::AboveValueThreshold(threshold) => {
                selection_helper.select_above_value_threshold(*threshold)
            }
            ZoneSelectionCriteria::BelowValueThreshold(threshold) => {
                selection_helper.select_below_value_threshold(*threshold)
            }
            ZoneSelectionCriteria::RelativeToMaxProportion(proportion) => {
                selection_helper.select_relative_to_max_proportion(*proportion)
            }
            ZoneSelectionCriteria::PercentileRange(lower_proportion, upper_proportion) => {
                selection_helper.select_percentile_range(*lower_proportion, *upper_proportion)
            }
            ZoneSelectionCriteria::TopN(n) => selection_helper.select_top_n(*n),
        };

        SelectionResults { indices }
    }
}

/// Specifies which data source to select from
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DataSource {
    pub score_type: ScoreType,
}

impl DataSource {
    pub fn new(score_type: ScoreType) -> Self {
        Self { score_type }
    }

    /// Extract and normalize data from CVACore
    /// Returns None if data is invalid (empty)
    pub fn extract(&self, cva_results: &CVACore) -> Option<Vec<f64>> {
        let raw_data = cva_results.get_scores_ref(self.score_type);

        // Validate data
        if raw_data.is_empty() {
            return None;
        }

        // Max-normalize by default
        Some(maths_utils::normalize_max(raw_data))
    }
}

/// A complete filter: data source + selection criteria
#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    pub source: DataSource,
    pub criteria: ZoneSelectionCriteria,
}

impl Filter {
    pub fn new(score_type: ScoreType, criteria: ZoneSelectionCriteria) -> Self {
        Self {
            source: DataSource::new(score_type),
            criteria,
        }
    }

    /// Evaluate this filter against CVA results
    /// Returns None if data extraction fails
    pub fn evaluate(&self, cva_results: &CVACore) -> Option<HashSet<usize>> {
        let data = self.source.extract(cva_results)?;
        let results = self.criteria.select(&data);
        Some(results.indices.into_iter().collect())
    }
}

/// Composable filter chain with logical operators
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum FilterChain {
    Single(Filter),
    And(Box<FilterChain>, Box<FilterChain>),
}

impl FilterChain {
    /// Create a single-filter chain
    pub fn new(score_type: ScoreType, criteria: ZoneSelectionCriteria) -> Self {
        FilterChain::Single(Filter::new(score_type, criteria))
    }

    /// Combine with another chain using AND logic
    #[allow(dead_code)]
    pub fn and(self, other: FilterChain) -> Self {
        FilterChain::And(Box::new(self), Box::new(other))
    }

    /// Evaluate the entire chain, returning selected zone indices
    /// Returns None if any filter evaluation fails
    pub fn evaluate(&self, cva_results: &CVACore) -> Option<HashSet<usize>> {
        match self {
            FilterChain::Single(filter) => filter.evaluate(cva_results),
            FilterChain::And(left, right) => {
                let left_set = left.evaluate(cva_results)?;
                let right_set = right.evaluate(cva_results)?;
                Some(left_set.intersection(&right_set).copied().collect())
            }
        }
    }

    /// Get all unique data sources (ScoreTypes) used in this filter chain
    #[allow(dead_code)] // Part of FilterChain API
    pub fn data_sources(&self) -> HashSet<ScoreType> {
        match self {
            FilterChain::Single(filter) => {
                let mut set = HashSet::new();
                set.insert(filter.source.score_type);
                set
            }
            FilterChain::And(left, right) => {
                let mut set = left.data_sources();
                for source in right.data_sources() {
                    set.insert(source);
                }
                set
            }
        }
    }

    /// Check if this chain uses multiple data sources
    #[allow(dead_code)] // Part of FilterChain API
    pub fn is_multi_source(&self) -> bool {
        self.data_sources().len() > 1
    }
}
