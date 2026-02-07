#[cfg(not(target_arch = "wasm32"))]
use {
    std::fs::File,
    std::io::{BufReader, BufWriter},
    crate::config::PERSISTENCE,
    crate::models::ledger::OpportunityLedger,
    anyhow::Result,
};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn save_ledger(ledger: &OpportunityLedger) -> Result<()> {
    let path = PERSISTENCE.app.ledger_path;
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    bincode::serialize_into(writer, ledger)?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn load_ledger() -> Result<OpportunityLedger> {
    let path = PERSISTENCE.app.ledger_path;
    if !std::path::Path::new(path).exists() {
        return Ok(OpportunityLedger::new());
    }
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let ledger = bincode::deserialize_from(reader)?;
    Ok(ledger)
}

// #[cfg(target_arch = "wasm32")]
// pub fn save_ledger(_ledger: &OpportunityLedger) -> Result<()> {
//     // WASM persistence is handled differently (usually local_storage), 
//     // but for now we just no-op as requested for the file split.
//     Ok(())
// }

// #[cfg(target_arch = "wasm32")]
// pub fn load_ledger() -> Result<OpportunityLedger> {
//     Ok(OpportunityLedger::new())
// }
