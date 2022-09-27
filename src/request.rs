use std::convert::TryFrom;
use std::net::{SocketAddr, ToSocketAddrs};

use anyhow::{anyhow, Result};
use http::header::HeaderValue;
use http::uri::Uri;
use http::{HeaderMap, Method};
use hyper::body::Bytes;
use tokio::task::spawn_blocking;
use tokio_native_tls::TlsConnector;

#[derive(Clone, Debug)]
pub enum Scheme {
    Http,
    Https(TlsConnector),
}

impl Scheme {
    fn default_port(&self) -> u16 {
        match self {
            Self::Http => 80,
            Self::Https(_) => 443,
        }
    }
}

#[derive(Clone)]
pub struct Request {
    pub addr: SocketAddr,
    pub scheme: Scheme,
    pub host: String,
    pub host_header: HeaderValue,
    pub uri: Uri,
    pub method: Method,
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl Request {
    pub async fn new(
        string: String,
        method: Method,
        headers: HeaderMap,
        body: Bytes,
    ) -> Result<Self> {
        spawn_blocking(move || Self::blocking_new(string, method, headers, body))
            .await
            .unwrap()
    }

    fn blocking_new(
        string: String,
        method: Method,
        headers: HeaderMap,
        body: Bytes,
    ) -> Result<Self> {
        let uri = Uri::try_from(string)?;
        let scheme = uri.scheme().unwrap_or(&http::uri::Scheme::HTTP).as_str();

        let scheme = match scheme {
            "http" => Scheme::Http,
            "https" => Scheme::Https(TlsConnector::from(
                native_tls::TlsConnector::builder()
                    .danger_accept_invalid_certs(true)
                    .danger_accept_invalid_hostnames(true)
                    .request_alpns(&["http/1.1"])
                    .build()?,
            )),
            _ => return Err(anyhow::Error::msg("invalid scheme")),
        };
        let authority = uri
            .authority()
            .ok_or_else(|| anyhow!("host not present on uri"))?;

        let mut host = authority.host().to_owned();
        if host.is_empty() {
            host.push_str("localhost");
        }

        let port = authority
            .port_u16()
            .unwrap_or_else(|| scheme.default_port());
        let host_header = HeaderValue::from_str(&host)?;

        let addr_iter = (host.as_str(), port).to_socket_addrs()?;
        let mut last_addr = None;
        for addr in addr_iter {
            last_addr = Some(addr);
            if addr.is_ipv4() {
                break;
            }
        }
        let addr = last_addr.ok_or_else(|| anyhow!("hostname lookup failed"))?;

        Ok(Self {
            addr,
            scheme,
            host,
            host_header,
            uri,
            method,
            headers,
            body,
        })
    }
}
