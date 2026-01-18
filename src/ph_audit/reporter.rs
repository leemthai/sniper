use chrono::Local;

pub struct AuditReporter {
    buffer: Vec<String>,
}

impl AuditReporter {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn add_header(&mut self) {
        // Added PH_Candles column
        self.buffer.push("Timestamp,Pair,Strategy,PH_Pct,Trend_K,Sim_K,Total_Candles,PH_Candles,Candidates,Top_Score,Stop_Pct,Exec_Ms,Dur_1_Hrs,Dur_2_Hrs,Dur_3_Hrs,Dur_4_Hrs,Dur_5_Hrs".to_string());
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_row(
        &mut self,
        pair: &str,
        strategy: &str,
        ph: f64,
        trend_k: usize,
        sim_k: usize,
        total_candles: usize,
        ph_candles: usize,
        candidates: usize,
        top_score: Option<f64>,
        avg_stop_pct: Option<f64>,
        exec_ms: u128,
        durations_hours: &[f64],
    ) {

        let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
        
        let mut d_str = String::new();
        for i in 0..5 {
            if let Some(hrs) = durations_hours.get(i) {
                d_str.push_str(&format!(",{:.2}", hrs));
            } else {
                d_str.push_str(",--");
            }
        }

        // FIX: Conditional Formatting based on Strategy
        let score_str = top_score.map(|v| {
            if strategy.contains("ROI") {
                format!("{:.2}%", v) // MaxROI or MaxAROI -> Add %
            } else {
                format!("{:.2}", v)  // Balanced -> Raw Score
            }
        }).unwrap_or_else(|| "--".to_string());
        
        // FIX: High Precision for Stop Loss
        let stop_str = avg_stop_pct.map(|v| format!("{:.4}%", v * 100.0)).unwrap_or_else(|| "--".to_string());

        let row = format!(
            "{},{},{},{:.1}%,{},{},{},{},{},{},{},{}{}",
            ts,
            pair,
            strategy,
            ph * 100.0,
            trend_k,
            sim_k,
            total_candles,
            ph_candles,
            candidates,
            score_str,
            stop_str,
            exec_ms,
            d_str
        );
        self.buffer.push(row);
    }

    pub fn print_all(&self) {
        // Optional: clear screen or add newlines to separate from noise
        println!("\n\n\n");
        println!("==================== CSV DATA START ====================");
        for line in &self.buffer {
            println!("{}", line);
        }
        println!("===================== CSV DATA END =====================");
    }
}
