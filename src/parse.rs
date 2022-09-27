extern crate clap;

use std::str::FromStr;

use ::http::header::HeaderName;
use ::http::{HeaderMap, HeaderValue, Method};
use anyhow::{Context, Result};
use clap::{App, Arg, ArgMatches};
use humantime::parse_duration;
use hyper::body::Bytes;

use crate::bench::{self, BenchmarkSettings};


pub fn parse() -> Result<BenchmarkSettings> {
    let args = parse_args();

    let threads: usize = args
        .value_of("threads")
        .unwrap_or("1")
        .trim()
        .parse()
        .with_context(|| {
            "invalid parameter for 'threads' given, input type must be a integer."
        })?;

    let conns: usize = args
        .value_of("connections")
        .unwrap_or("1")
        .trim()
        .parse()
        .with_context(|| {
            "invalid parameter for 'connections' given, input type must be a integer."
        })?;

    let host: &str = args
        .value_of("host")
        .with_context(|| "missing 'host' parameter.")?;

    let duration: &str = args.value_of("duration").unwrap_or("1s");
    let duration = parse_duration(duration)
        .with_context(|| "failed to parse duration parameter")?;

    let rounds: usize = args
        .value_of("rounds")
        .unwrap_or("1")
        .trim()
        .parse::<usize>()
        .unwrap_or(1);

    let method = args
        .value_of("method")
        .map(|method| Method::from_str(&method.to_uppercase()))
        .transpose()?
        .unwrap_or(Method::GET);

    let headers = args
        .values_of("header")
        .unwrap_or_default()
        .map(parse_header)
        .collect::<Result<HeaderMap<_>>>()
        .with_context(|| "failed to parse method")?;

    let body: &str = args.value_of("body").unwrap_or_default();
    let body = Bytes::copy_from_slice(body.as_bytes());

    Ok(bench::BenchmarkSettings {
        threads,
        connections: conns,
        host: host.to_string(),
        duration,
        rounds,
        method,
        headers,
        body,
    })
}

fn parse_header(value: &str) -> Result<(HeaderName, HeaderValue)> {
    let (key, value) = value
        .split_once(": ")
        .context("Header value missing colon (\": \")")?;
    let key = HeaderName::from_str(key).context("Invalid header name")?;
    let value = HeaderValue::from_str(value).context("Invalid header value")?;
    Ok((key, value))
}

fn parse_args() -> ArgMatches<'static> {
    App::new("wrk")
      .version("0.0.1")
      .arg(
          Arg::with_name("threads")
              .short("t")
              .long("threads")
              .help("Set the amount of threads to use")
              .takes_value(true)
              .default_value("1"),
      )
      .arg(
          Arg::with_name("connections")
              .short("c")
              .long("connections")
              .help("Set the amount of concurrent")
              .takes_value(true)
              .default_value("1"),
      )
      .arg(
          Arg::with_name("host")
              .short("h")
              .long("host")
              .help("Set the host to bench'")
              .takes_value(true)
              .required(true),
      )
      .arg(
          Arg::with_name("duration")
              .short("d")
              .long("duration")
              .help("Set the duration of the benchmark")
              .takes_value(true)
              .default_value("2s")
      )
      .arg(
          Arg::with_name("rounds")
              .long("rounds")
              .short("r")
              .help("Repeats the benchmarks n amount of times")
              .takes_value(true)
              .required(false),
      )
      .arg(
          Arg::with_name("method")
              .long("method")
              .short("m")
              .help("Set request method")
              .takes_value(true)
              .required(false)
              .multiple(true),
      )
      .arg(
          Arg::with_name("header")
              .long("header")
              .short("H")
              .help("Add header to request")
              .takes_value(true)
              .required(false)
              .multiple(true),
      )
      .arg(
          Arg::with_name("body")
              .long("body")
              .short("b")
              .help("Add body to request")
              .takes_value(true)
              .required(false),
      )
      .get_matches()
}
