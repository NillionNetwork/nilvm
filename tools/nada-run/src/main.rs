use anyhow::Result;
use nada_run::driver;

fn main() -> Result<()> {
    env_logger::init();
    driver()?;
    Ok(())
}
