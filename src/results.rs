use std::collections::HashMap;

use colored::Colorize;
use tokio::time::Duration;
use humansize::{format_size, DECIMAL};


#[derive(Default, Debug)]
pub struct WorkerResult {
    pub total_times: Vec<Duration>,
    pub request_times: Vec<Duration>,
    pub buffer_sizes: Vec<usize>,
    pub error_map: HashMap<String, usize>,
}

impl WorkerResult {
    pub fn default() -> Self {
        Self {
            total_times: vec![],
            request_times: vec![],
            buffer_sizes: vec![],
            error_map: HashMap::new(),
        }
    }

    pub fn combine(mut self, other: Self) -> Self {
        self.request_times.extend(other.request_times);
        self.total_times.extend(other.total_times);
        self.buffer_sizes.extend(other.buffer_sizes);

        for (message, count) in other.error_map {
            match self.error_map.get_mut(&message) {
                Some(c) => *c += count,
                None => {
                    self.error_map.insert(message, count);
                },
            }
        }
        self
    }

    pub fn total_requests(&self) -> usize {
        self.request_times.len()
    }

    pub fn total_transfer(&self) -> usize {
        self.buffer_sizes.iter().sum()
    }

    pub fn avg_transfer(&self) -> f64 {
        self.total_transfer() as f64 / self.avg_total_time().as_secs_f64()
    }

    pub fn avg_request_per_sec(&self) -> f64 {
        let amount = self.request_times.len() as f64;
        let avg_time = self.avg_total_time();

        amount / avg_time.as_secs_f64()
    }

    pub fn avg_total_time(&self) -> Duration {
        let avg: f64 = self.total_times.iter().map(|dur| dur.as_secs_f64()).sum();

        let len = self.total_times.len() as f64;
        Duration::from_secs_f64(avg / len)
    }

    pub fn avg_request_latency(&self) -> Duration {
        let avg: f64 = self.request_times.iter().map(|dur| dur.as_secs_f64()).sum();

        let len = self.total_requests() as f64;
        Duration::from_secs_f64(avg / len)
    }

    pub fn max_request_latency(&self) -> Duration {
        self.request_times.iter().max().copied().unwrap_or_default()
    }

    pub fn min_request_latency(&self) -> Duration {
        self.request_times.iter().min().copied().unwrap_or_default()
    }

    pub fn variance(&self) -> f64 {
        let mean = self.avg_request_latency().as_secs_f64();
        let sum_delta: f64 = self
            .request_times
            .iter()
            .map(|dur| {
                let time = dur.as_secs_f64();
                let delta = time - mean;

                delta.powi(2)
            })
            .sum();

        sum_delta / self.total_requests() as f64
    }

    pub fn std_deviation_request_latency(&self) -> f64 {
        let diff = self.variance();
        diff.powf(0.5)
    }

    pub fn display_latencies(&mut self) {
        let modified = 1000_f64;
        let avg = self.avg_request_latency().as_secs_f64() * modified;
        let max = self.max_request_latency().as_secs_f64() * modified;
        let min = self.min_request_latency().as_secs_f64() * modified;
        let std_deviation = self.std_deviation_request_latency() * modified;

        println!(
            "{:<13} {:<7} {:<7} {:<7} {:<7}  ",
            "Thread Stats",
            "Avg".bold(),
            "Stdev".bold(),
            "Min".bold(),
            "Max".bold(),
        );
        println!(
            "{:<13} {:<7} {:<7} {:<7} {:<7}  \n",
            "Latency",
            format!("{:.2}ms", avg),
            format!("{:.2}ms", std_deviation),
            format!("{:.2}ms", min),
            format!("{:.2}ms", max),
        );
    }

    pub fn display_requests(&mut self) {
        let total = self.total_requests();
        let avg = self.avg_request_per_sec();

        println!(
            "Requests: {:<15} Total: {:<7}",
            format!("{:.2} Req/s", avg).as_str().blue().bold(),
            format!("{} Reqs", total).as_str(),
        )
    }

    pub fn display_transfer(&mut self) {
        let total = self.total_transfer() as f64;
        let rate = self.avg_transfer();

        let display_total = format_size(total as u64, DECIMAL);
        let display_rate = format_size(rate as u64, DECIMAL);

        println!(
            "Transfer: {:<15} Total: {:<7} ",
            format!("{}/s", display_rate).as_str().blue().bold(),
            display_total.as_str(),
        )
    }

    pub fn display_errors(&self) {
        if !self.error_map.is_empty() {
            println!();

            for (message, count) in &self.error_map {
                println!("{} Errors: {}", count, message);
            }
        }
    }

}
