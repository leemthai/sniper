use chrono::Local;

pub struct AuditReporter {
    buffer: Vec<String>,
}

impl AuditReporter {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn add_header(&mut self) {
        self.buffer.push(
            "Timestamp,Pair,PH_Pct,Candles,Candidates,Top_Score,Avg_Dur_Hrs,Stop_Pct,Exec_Ms"
                .to_string(),
        );
    }

   #[allow(clippy::too_many_arguments)]
    pub fn add_row(
        &mut self,
        pair: &str,
        strategy: &str,
        ph: f64,
        candles: usize,
        candidates: usize,
        top_score: f64,
        avg_duration_ms: u64,
        avg_stop_pct: f64,
        exec_ms: u128,
    ) {
        // FIX: Proper Date-Time Format
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
        let dur_hours = avg_duration_ms as f64 / 1000.0 / 60.0 / 60.0;
        
        let row = format!(
            "{},{},{},{:.1}%,{},{},{:.2},{:.2}h,{:.2}%,{}",
            ts,
            pair,
            strategy,
            ph * 100.0,
            candles,
            candidates,
            top_score,
            dur_hours,
            avg_stop_pct * 100.0,
            exec_ms
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
