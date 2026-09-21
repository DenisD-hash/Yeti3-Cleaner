#[path = "../history/mod.rs"]
mod history;

fn main() -> anyhow::Result<()> {
    let _db = history::HistoryDb::open()?;
    println!("YETI³ Cleaner history database initialized");
    Ok(())
}
