use {
    crate::{config::PERSISTENCE, models::OpportunityLedger},
    anyhow::Result,
    std::{
        fs::File,
        io::{BufReader, BufWriter},
    },
};

pub(crate) fn save_ledger(ledger: &OpportunityLedger) -> Result<()> {
    let path = PERSISTENCE.app.ledger_path;
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    bincode::serialize_into(writer, ledger)?;
    Ok(())
}

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
