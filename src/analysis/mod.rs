// Analysis algorithms and zone scoring
pub mod multi_pair_monitor;
pub mod pair_analysis;
pub mod selection_criteria;
pub mod zone_scoring;
pub mod horizon_profiler;
pub mod range_gap_finder;
pub mod market_state;
pub mod scenario_simulator;
pub mod adaptive;

// Re-export commonly used types
pub use multi_pair_monitor::MultiPairMonitor;
