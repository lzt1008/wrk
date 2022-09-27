use std::time::Duration;

use tokio::runtime::Builder;

use ::http::{HeaderMap, Method};
use anyhow::Result;
use colored::*;
use futures_util::StreamExt;
use humantime::format_duration;
use hyper::body::Bytes;

use crate::results::WorkerResult;

use crate::http;

#[derive(Clone, Debug)]
pub struct BenchmarkSettings {
    pub threads: usize,
    pub connections: usize,
    pub host: String,
    pub duration: Duration,
    pub rounds: usize,
    pub method: Method,
    pub headers: HeaderMap,
    pub body: Bytes,
}

pub fn start_benchmark(settings: BenchmarkSettings) {
    let runtime = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(settings.threads)
        .build()
        .expect("Failed to build the runtime.");

    let rounds = settings.rounds;
    for i in 0..rounds {
        print!("Round {}: ", (i + 1).to_string().bold().blue());

        if let Err(e) = runtime.block_on(run(settings.clone())) {
            eprintln!();
            eprintln!("{}", e);
            return;
        }

        println!();
    }
}

async fn run(settings: BenchmarkSettings) -> Result<()> {
    let predict_size = settings.duration.as_secs() * 10_000;

    let mut handles = http::start_tasks(
        settings.duration,
        settings.connections,
        settings.host.trim().to_string(),
        settings.method,
        settings.headers,
        settings.body,
        predict_size as usize,
    )
    .await?;

    println!(
        "Benchmarking {} for {} connection(s) and {}\n",
        settings.host.bold(),
        format!("{:.2}", settings.connections).bold(),
        format_duration(settings.duration).to_string().bold(),
    );

    let mut combiner = WorkerResult::default();

    while let Some(result) = handles.next().await {
        combiner = combiner.combine(result.unwrap()?);
    }

    if combiner.total_requests() == 0 {
        println!("No requests completed successfully");
        return Ok(());
    }

    combiner.display_latencies();
    combiner.display_requests();
    combiner.display_transfer();

    combiner.display_errors();

    Ok(())
}
