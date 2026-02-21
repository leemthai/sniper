use {
    crate::{app::App, models::TradeOpportunity},
    eframe::egui::Context,
    serde::{Deserialize, Serialize},
    std::fmt,
};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    Pending,
    Syncing,
    Completed(usize), // number of new candles
    Failed(String),   // Error
}

#[derive(Debug, Clone)]
pub struct ProgressEvent {
    pub index: usize,
    pub pair: String,
    pub status: SyncStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum SortDirection {
    Ascending,
    #[default]
    Descending,
}

impl SortDirection {
    pub fn toggle(&self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Default)]
pub(crate) enum Selection {
    #[default]
    None,
    Pair(String),
    Opportunity(TradeOpportunity),
}

impl fmt::Display for Selection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Selection::None => write!(f, "Selection::None"),
            Selection::Pair(pair_name) => write!(f, "Selection::Pair({})", pair_name),
            Selection::Opportunity(op) => {
                write!(f, "Selection::Opportunity({}, id={})", op.pair_name, op.id)
            }
        }
    }
}

impl Selection {
    /// owned String
    #[inline]
    pub(crate) fn pair_owned(&self) -> Option<String> {
        match self {
            Selection::Pair(p) => Some(p.clone()),
            Selection::Opportunity(op) => Some(op.pair_name.clone()),
            Selection::None => None,
        }
    }

    /// borrowed view
    #[inline]
    pub(crate) fn pair(&self) -> Option<&str> {
        match self {
            Selection::Pair(p) => Some(p),
            Selection::Opportunity(op) => Some(&op.pair_name),
            Selection::None => None,
        }
    }

    pub(crate) fn opportunity(&self) -> Option<&TradeOpportunity> {
        match self {
            Selection::Opportunity(op) => Some(op),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum PersistedSelection {
    None,
    Pair(String),
    Opportunity {
        pair: String,
        opportunity_id: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AutoScaleY(pub bool);

impl AutoScaleY {
    pub fn value(&self) -> bool {
        self.0
    }
}

impl Default for AutoScaleY {
    fn default() -> Self {
        Self(true)
    }
}

pub(crate) trait PhaseView {
    fn tick(&mut self, app: &mut App, ctx: &Context) -> AppState;
}

#[derive(Clone)]
pub(crate) struct RunningState;

impl PhaseView for RunningState {
    fn tick(&mut self, app: &mut App, ctx: &Context) -> AppState {
        #[cfg(feature = "ph_audit")]
        app.try_run_audit(ctx);

        app.tick_running_state(ctx);

        AppState::Running(RunningState)
    }
}

#[derive(Clone, Default)]
pub(crate) struct TuningState {
    pub(crate) todo_list: Vec<String>,
    pub(crate) total: usize,
    pub(crate) completed: usize,
}

impl PhaseView for TuningState {
    fn tick(&mut self, app: &mut App, ctx: &Context) -> AppState {
        app.tick_tuning_state(ctx, self)
    }
}

#[derive(Default, Clone)]
pub(crate) struct BootstrapState {
    pub(crate) pairs: std::collections::BTreeMap<usize, (String, SyncStatus)>,
    pub(crate) total_pairs: usize,
    pub(crate) completed: usize,
    pub(crate) failed: usize,
}
impl PhaseView for BootstrapState {
    fn tick(&mut self, app: &mut App, ctx: &Context) -> AppState {
        app.tick_bootstrap_state(ctx, self)
    }
}

pub(crate) enum AppState {
    Bootstrapping(BootstrapState),
    Tuning(TuningState),
    Running(RunningState),
}

impl Default for AppState {
    fn default() -> Self {
        AppState::Bootstrapping(BootstrapState::default())
    }
}
