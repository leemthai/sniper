use {
    crate::ui::UI_TEXT,
    serde::{Deserialize, Serialize},
    strum_macros::{Display, EnumIter},
};
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Display,
    EnumIter,
    Ord,
    PartialOrd,
    Default,
)]
pub(crate) enum OptimizationStrategy {
    #[strum(to_string = "Max ROI")]
    MaxROI,

    #[strum(to_string = "Max AROI")]
    MaxAROI,

    #[strum(to_string = "Balanced")]
    Balanced,

    /// Log-Growth Confidence Score
    /// **Goal:** Maximize long-term capital via geometric growth ($E[\log(1+fR)]$).
    /// **Math:** Approximates **Mean Return − ½ Variance**.
    /// **Result:** Auto penalizes volatility and tail risk, ensuring stable, growth-optimal performance aligned with your architecture.
    #[strum(to_string = "Log Growth (Confidence)")]
    #[default]
    LogGrowthConfidence,
}

impl OptimizationStrategy {
    pub(crate) fn icon(&self) -> String {
        match self {
            OptimizationStrategy::MaxROI => UI_TEXT.icon_strategy_roi.to_string(),
            OptimizationStrategy::MaxAROI => UI_TEXT.icon_strategy_aroi.to_string(),
            OptimizationStrategy::Balanced => UI_TEXT.icon_strategy_balanced.to_string(),
            OptimizationStrategy::LogGrowthConfidence => {
                UI_TEXT.icon_strategy_log_growth.to_string()
            }
        }
    }
}
