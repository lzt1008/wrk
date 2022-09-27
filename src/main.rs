mod bench;
mod http;
mod parse;
mod results;
mod usage;
mod request;

use anyhow::Result;
use bench::start_benchmark;
use crate::parse::parse;

fn main() -> Result<()> {
    let settings = parse()?;
    start_benchmark(settings);

    Ok(())
}
