pub struct DemoResources {
    pub pairs: &'static [&'static str],
}

pub struct DemoConfig {
    pub max_pairs: usize,
    pub resources: DemoResources,
}

pub const DEMO: DemoConfig = DemoConfig {
    max_pairs: 10,
    resources: DemoResources {
        pairs: &[
            "BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT", "PAXGUSDT", "DOGEUSDT", "USDCUSDT",
        ],
    },
};
