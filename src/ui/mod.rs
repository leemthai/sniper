mod plot_layers;
mod screens;
mod styles;
mod ticker;
mod time_tuner;
mod ui_config;
mod ui_panels;
mod ui_plot_view;
mod ui_render;
mod ui_text;

pub(crate) use plot_layers::{
    BackgroundLayer, CandlestickLayer, HorizonLinesLayer, LayerContext, OpportunityLayer,
    PlotLayer, PriceLineLayer, ReversalZoneLayer, SegmentSeparatorLayer, StickyZoneLayer,
};

pub(crate) use screens::render_bootstrap;

pub(crate) use styles::{
    DirectionColor, UiStyleExt, apply_opacity, get_momentum_color, get_outcome_color,
};
pub(crate) use ticker::TickerState;

pub(crate) use time_tuner::{TunerAction, render_time_tuner};

pub(crate) use ui_config::{UI_CONFIG, UI_TEXT};
pub(crate) use ui_panels::CandleRangePanel;
pub(crate) use ui_plot_view::{PlotCache, PlotInteraction, PlotView, PlotVisibility};
pub(crate) use ui_render::{
    NavigationState, NavigationTarget, ScrollBehavior, SortColumn, TradeFinderRow,
};
