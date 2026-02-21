use {
    crate::{
        analysis::pair_analysis,
        config::{BASE_INTERVAL, OptimizationStrategy, PhPct, Price, PriceLike, StationId},
        data::timeseries::TimeSeriesCollection,
        engine::run_pathfinder_simulations,
        models::find_matching_ohlcv,
        ph_audit::{AUDIT_PAIRS, AuditReporter, PH_LEVELS},
        utils::time_utils::AppInstant,
    },
    std::collections::HashMap,
    strum::IntoEnumIterator,
};

#[cfg(feature = "ph_audit")]
pub fn execute_audit(
    ts_collection: &TimeSeriesCollection,
    current_prices: &HashMap<String, Price>, // NEW: Live prices from Ticker
) {
    println!("=== STARTING MULTI-STRATEGY SPECTRUM AUDIT ===");

    let mut reporter = AuditReporter::new();
    reporter.add_header();

    for &pair in AUDIT_PAIRS {
        // Validate Data Exists
        if find_matching_ohlcv(
            &ts_collection.series_data,
            pair,
            BASE_INTERVAL.as_millis() as i64,
        )
        .is_err()
        {
            println!("Skipping {} (No OHLCV Data Loaded)", pair);
            continue;
        }

        // Validate Live Price Exists (Strict)
        let Some(&live_price) = current_prices.get(pair) else {
            println!("Skipping {} (No Live Price Available)", pair);
            continue;
        };

        if !live_price.is_positive() {
            println!("Skipping {} (Live Price is Zero)", pair);
            continue;
        }

        println!(">> Scanning {} @ ${:.4}...", pair, live_price);

        // Loop Strategies
        for strategy in OptimizationStrategy::iter() {
            // Loop PH Levels
            for &ph_pct in PH_LEVELS {
                run_single_simulation(
                    pair,
                    live_price,
                    &strategy,
                    PhPct::new(ph_pct),
                    ts_collection,
                    // base_config,
                    &mut reporter,
                );
            }
        }
    }

    println!("Audit Complete. Flushing CSV...");
    reporter.print_all();

    std::process::exit(0);
}

fn run_single_simulation(
    pair: &str,
    price: Price,
    strategy: &OptimizationStrategy,
    ph_pct: PhPct,
    ts_collection: &TimeSeriesCollection,
    reporter: &mut AuditReporter,
) {
    let ohlcv = find_matching_ohlcv(
        &ts_collection.series_data,
        pair,
        BASE_INTERVAL.as_millis() as i64,
    )
    .unwrap(); // Unwrap is safe here because we checked existence in the main loop
    let start_time = AppInstant::now();

    // C. Run Pipeline (Using worker internals)
    let cva_res = pair_analysis::pair_analysis_pure(pair.to_string(), ts_collection, price, ph_pct);

    let strat_name = format!("{:?}", strategy);

    if cva_res.is_err() {
        return;
    }

    let cva = cva_res.unwrap();
    let ph_candles = cva.relevant_candle_count;

    // Pathfinder (Scout + Drill)
    let pf_result = run_pathfinder_simulations(
        ohlcv,
        price,
        ph_pct,
        *strategy,
        StationId::default(),
        Some(&cva),
    );
    let elapsed = start_time.elapsed().as_millis();
    let opportunities = pf_result.opportunities;
    let trend_k = pf_result.trend_lookback; // Truth from the engine
    let sim_k = pf_result.sim_duration; // Truth from the engine

    // D. Extract Stats
    let count = opportunities.len();

    let top_score = opportunities.first().map(|o| o.calculate_quality_score());

    // DISPLAY LOGIC: Convert ms to hours for CSV readability
    let durations_hours: Vec<f64> = opportunities
        .iter()
        .take(5)
        .map(|o| o.avg_duration.to_hours())
        .collect();

    // Avg Stop Loss %
    let top_5_b = opportunities.iter().take(5);
    let avg_stop: Option<f64> = if count > 0 {
        let sum: f64 = top_5_b
            .clone()
            .map(|o| (Price::from(o.stop_price) - o.start_price).abs() / o.start_price)
            .sum();
        Some(sum / top_5_b.count() as f64)
    } else {
        None
    };

    reporter.add_row(
        pair,
        &strat_name,
        ph_pct,
        trend_k,
        sim_k,
        ohlcv.close_prices.len(),
        ph_candles,
        count,
        top_score,
        avg_stop,
        elapsed,
        &durations_hours,
    );
}
