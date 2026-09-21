#[path = "../config/mod.rs"]
mod config;

fn main() -> anyhow::Result<()> {
    let settings = config::load()?;
    config::save(&settings)?;
    println!("{}", config::settings_path()?.display());
    Ok(())
}
