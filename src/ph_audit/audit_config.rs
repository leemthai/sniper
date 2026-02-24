pub const AUDIT_PAIRS: &[&str] = &[
    "BTCUSDT",
    "DOGEUSDT",
    "SOLUSDT",
    "ETHUSDT",
    "XRPUSDT",
    "BNBFDUSD",
    "PAXGUSDT",
    "ZECUSDT",
    "KITEUSDT",
    "LUNCUSDT",
    "PEPEUSDT",
    "PENGUUSDT",
];

// The Spectrum: Micro (0.5%) to Macro
pub const PH_LEVELS: &[f64] = &[
    // --- Micro (< 1%) ---
    // 0.005, // Can't find any results here yet.....
    0.008, // --- Short (1% to 10%, step 1%) ---
    0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.07, 0.08, 0.09, 0.10,
    // --- Medium (10% to 20%, step 2%) ---
    0.12, 0.14, 0.16, 0.18, 0.20, // --- Macro (20% to 50%, step 4%) ---
    0.24, 0.28, 0.32, 0.36, 0.40, 0.44, 0.48,
    // --- Ultra Macro (50% to 100%, step 8%) ---
    0.56, 0.64, 0.72, 0.80,
];
