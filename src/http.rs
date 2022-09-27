use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::anyhow;
use futures_util::stream::FuturesUnordered;
use futures_util::TryFutureExt;
use http::header::{self, HeaderMap};
use http::{Method, Request};
use hyper::body::Bytes;
use hyper::client::conn::{self, SendRequest};
use hyper::Body;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tokio::time::error::Elapsed;
use tokio::time::{sleep, timeout_at, Instant};
use tower::util::ServiceExt;
use tower::Service;

use crate::results::WorkerResult;
use crate::usage::Usage;
use crate::request::{Scheme, Request as UserRequest};

pub type Handle = JoinHandle<anyhow::Result<WorkerResult>>;

pub async fn start_tasks(
    time_for: Duration,
    connections: usize,
    uri_string: String,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
    _predicted_size: usize,
) -> anyhow::Result<FuturesUnordered<Handle>> {
    let deadline = Instant::now() + time_for;
    let user_request = UserRequest::new(uri_string, method, headers, body).await?;

    let handles = FuturesUnordered::new();

    for _ in 0..connections {
        handles.push(tokio::spawn(benchmark(deadline, user_request.clone())));
    }

    Ok(handles)
}

async fn benchmark(
    deadline: Instant,
    user_request: UserRequest,
) -> anyhow::Result<WorkerResult> {
    let benchmark_start = Instant::now();
    let connector = Connector::new(
        deadline,
        user_request.addr,
        user_request.scheme,
        user_request.host,
    );

    let (mut send_request, mut connection_task) =
        match timeout_at(deadline, connector.connect()).await {
            Ok(result) => result?,
            Err(_elapsed) => return Ok(WorkerResult::default()),
        };

    let mut request_headers = HeaderMap::new();
    request_headers.insert(header::HOST, user_request.host_header);
    request_headers.extend(user_request.headers);

    let mut request_times = Vec::new();
    let mut error_map = HashMap::new();

    loop {
        let mut request = Request::new(Body::from(user_request.body.clone()));
        *request.method_mut() = user_request.method.clone();
        *request.uri_mut() = user_request.uri.clone();
        *request.headers_mut() = request_headers.clone();

        let future = send_request
            .ready()
            .and_then(|sr| sr.call(request))
            .and_then(|response| hyper::body::to_bytes(response.into_body()));

        let future = async {
            tokio::select! {
                biased;
                result = (&mut connection_task) => {
                    match result.unwrap() {
                        Ok(()) => Err(anyhow!("connection closed")),
                        Err(e) => Err(anyhow!(e)),
                    }
                },
                result = future => result.map(|_| ()).map_err(Into::into),
            }
        };

        let request_start = Instant::now();

        if let Ok(result) = timeout_at(deadline, future).await {
            if let Err(e) = result {
                let error = e.to_string();

                match error_map.get_mut(&error) {
                    Some(count) => *count += 1,
                    None => {
                        error_map.insert(error, 1);
                    },
                }

                match connector.try_connect_until().await {
                    Ok((sr, task)) => {
                        send_request = sr;
                        connection_task = task;
                    },
                    Err(_elapsed) => break,
                };
            }
        } else {
            break;
        }

        request_times.push(request_start.elapsed());
    }

    Ok(WorkerResult {
        total_times: vec![benchmark_start.elapsed()],
        request_times,
        buffer_sizes: vec![connector.get_received_bytes()],
        error_map,
    })
}

struct Connector {
    deadline: Instant,
    addr: SocketAddr,
    scheme: Scheme,
    host: String,
    usage: Usage,
}

impl Connector {
    fn new(deadline: Instant, addr: SocketAddr, scheme: Scheme, host: String) -> Self {
        Self {
            deadline,
            addr,
            scheme,
            host,
            usage: Usage::new(),
        }
    }

    async fn try_connect_until(
        &self,
    ) -> Result<(SendRequest<Body>, JoinHandle<hyper::Result<()>>), Elapsed> {
        let future = async {
            loop {
                if let Ok(v) = self.connect().await {
                    return v;
                }

                sleep(Duration::from_millis(25)).await;
            }
        };

        timeout_at(self.deadline, future).await
    }

    async fn connect(
        &self,
    ) -> anyhow::Result<(SendRequest<Body>, JoinHandle<hyper::Result<()>>)> {
        let conn_builder = conn::Builder::new();
        let stream = self.usage.wrap_stream(TcpStream::connect(self.addr).await?);

        let send_request = match self.scheme {
            Scheme::Http => handshake(conn_builder, stream).await?,
            Scheme::Https(ref tls_connector) => {
                let stream = tls_connector.connect(&self.host, stream).await?;
                handshake(conn_builder, stream).await?
            },
        };

        Ok(send_request)
    }

    fn get_received_bytes(&self) -> usize {
        self.usage.get_received_bytes()
    }
}

async fn handshake<S>(
    conn_builder: conn::Builder,
    stream: S,
) -> anyhow::Result<(SendRequest<Body>, JoinHandle<hyper::Result<()>>)>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (send_request, connection) = conn_builder.handshake(stream).await?;
    let connection_task = tokio::spawn(connection);
    Ok((send_request, connection_task))
}
