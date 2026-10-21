//! The listening side (ADR 0048): on 127.0.0.1, 4317 and 4318 when free, else ports the system
//! chooses; each port serves OTLP over gRPC and over HTTP (protobuf or JSON), so the endpoint
//! given to the tests suits any exporter.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use opentelemetry_proto::tonic::collector::logs::v1::{
    ExportLogsServiceRequest, ExportLogsServiceResponse,
};
use opentelemetry_proto::tonic::collector::metrics::v1::metrics_service_server::{
    MetricsService, MetricsServiceServer,
};
use opentelemetry_proto::tonic::collector::metrics::v1::{
    ExportMetricsServiceRequest, ExportMetricsServiceResponse,
};
use opentelemetry_proto::tonic::collector::trace::v1::trace_service_server::{
    TraceService, TraceServiceServer,
};
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use prost::Message;
use timonier_core::perf::{ReceivedMetric, ReceivedSpan};
use timonier_core::usage::AgentRequest;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinSet;

use crate::{GRPC_PORT, HTTP_PORT, decode};

/// Where what the receiver takes goes: the daemon keeps it (ADR 0043). Called off the async
/// runtime, on a blocking thread.
pub trait Sink: Send + Sync + 'static {
    /// `token`: the `x-timonier-token` header of the request, which says which run sends
    /// (audit of 2026-10-21, finding 2); the sink drops what no run's token covers.
    fn spans(&self, token: Option<&str>, spans: Vec<ReceivedSpan>);
    fn metrics(&self, token: Option<&str>, metrics: Vec<ReceivedMetric>);
}

/// Where the agents' requests go (ADR 0058), with the agent's session; called off the runtime.
pub trait AgentSink: Send + Sync + 'static {
    fn requests(&self, requests: Vec<(String, AgentRequest)>);
}

/// The header that carries a run's token, in gRPC metadata or HTTP (`OTEL_EXPORTER_OTLP_HEADERS`).
pub const TOKEN_HEADER: &str = "x-timonier-token";

/// One message at most this big, in either protocol.
const MESSAGE_BYTES: usize = 16 << 20;

/// A running receiver; it stops when dropped or [`Receiver::stop`]ped.
pub struct Receiver {
    http: SocketAddr,
    grpc: SocketAddr,
    stop: watch::Sender<bool>,
    tasks: JoinSet<()>,
}

impl Receiver {
    /// Listens on OTLP's ports, or on ports of the system's choice for those taken.
    pub async fn start(sink: Arc<dyn Sink>) -> std::io::Result<Self> {
        Self::start_on(sink, HTTP_PORT, GRPC_PORT).await
    }

    /// Listens on `http` and `grpc` (0: the system's choice), falling back to the system's
    /// choice for a port taken.
    pub async fn start_on(sink: Arc<dyn Sink>, http: u16, grpc: u16) -> std::io::Result<Self> {
        let http = bind(http).await?;
        let grpc = bind(grpc).await?;
        let (stop, stopped) = watch::channel(false);
        let router = router(sink);
        let mut receiver = Self {
            http: http.local_addr()?,
            grpc: grpc.local_addr()?,
            stop,
            tasks: JoinSet::new(),
        };
        for listener in [http, grpc] {
            let router = router.clone();
            let mut stopped = stopped.clone();
            receiver.tasks.spawn(async move {
                let serving = axum::serve(listener, router).with_graceful_shutdown(async move {
                    let _ = stopped.wait_for(|stop| *stop).await;
                });
                if let Err(error) = serving.await {
                    tracing::warn!(%error, "the OTLP receiver stopped");
                }
            });
        }
        tracing::info!(http = %receiver.http, grpc = %receiver.grpc, "OTLP receiver listening");
        Ok(receiver)
    }

    pub fn http(&self) -> SocketAddr {
        self.http
    }

    pub fn grpc(&self) -> SocketAddr {
        self.grpc
    }

    /// What `OTEL_EXPORTER_OTLP_ENDPOINT` says: the HTTP port, which serves gRPC too.
    pub fn endpoint(&self) -> String {
        format!("http://{}", self.http)
    }

    pub async fn stop(mut self) {
        let _ = self.stop.send(true);
        while self.tasks.join_next().await.is_some() {}
    }
}

/// The receiver of the agents' telemetry on the machine's port (ADR 0058): OTLP logs over HTTP
/// only. A port taken stays taken: another daemon of the machine holds it.
pub struct AgentReceiver {
    http: SocketAddr,
    stop: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}

impl AgentReceiver {
    pub async fn start(sink: Arc<dyn AgentSink>, port: u16) -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
        let http = listener.local_addr()?;
        let (stop, mut stopped) = watch::channel(false);
        let router = agent_router(sink);
        let task = tokio::spawn(async move {
            let serving = axum::serve(listener, router).with_graceful_shutdown(async move {
                let _ = stopped.wait_for(|stop| *stop).await;
            });
            if let Err(error) = serving.await {
                tracing::warn!(%error, "the agents' OTLP receiver stopped");
            }
        });
        tracing::info!(%http, "agents' OTLP receiver listening");
        Ok(Self { http, stop, task })
    }

    pub fn http(&self) -> SocketAddr {
        self.http
    }

    pub async fn stop(self) {
        let _ = self.stop.send(true);
        let _ = self.task.await;
    }
}

fn agent_router(sink: Arc<dyn AgentSink>) -> axum::Router {
    axum::Router::new()
        .route(
            "/v1/logs",
            post(move |headers: HeaderMap, body: Bytes| {
                let sink = Arc::clone(&sink);
                async move {
                    let body = match gunzip(&headers, body) {
                        Ok(body) => body,
                        Err(refused) => return refused.into_response(),
                    };
                    let encoding = match encoding(&headers_without_gzip(&headers)) {
                        Ok(encoding) => encoding,
                        Err(refused) => return refused.into_response(),
                    };
                    match decode::<ExportLogsServiceRequest>(encoding, &body) {
                        Ok(request) => {
                            let requests = decode::agent_requests(&request);
                            if !requests.is_empty() {
                                let _ =
                                    tokio::task::spawn_blocking(move || sink.requests(requests))
                                        .await;
                            }
                            answer(encoding, &ExportLogsServiceResponse::default())
                        }
                        Err(refused) => refused.into_response(),
                    }
                }
            }),
        )
        .layer(DefaultBodyLimit::max(MESSAGE_BYTES))
}

/// The body, inflated when it came gzipped; at most [`MESSAGE_BYTES`] once inflated.
fn gunzip(headers: &HeaderMap, body: Bytes) -> Result<Bytes, Refused> {
    if headers
        .get(header::CONTENT_ENCODING)
        .is_none_or(|e| e != "gzip")
    {
        return Ok(body);
    }
    use std::io::Read;
    let mut inflated = Vec::new();
    flate2::read::GzDecoder::new(body.as_ref())
        .take(MESSAGE_BYTES as u64 + 1)
        .read_to_end(&mut inflated)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    if inflated.len() > MESSAGE_BYTES {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, "message too big".to_owned()));
    }
    Ok(Bytes::from(inflated))
}

fn headers_without_gzip(headers: &HeaderMap) -> HeaderMap {
    let mut headers = headers.clone();
    headers.remove(header::CONTENT_ENCODING);
    headers
}

async fn bind(port: u16) -> std::io::Result<TcpListener> {
    match TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await {
        Err(error) if port != 0 && error.kind() == std::io::ErrorKind::AddrInUse => {
            tracing::info!(port, "OTLP port taken: one of the system's choice instead");
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await
        }
        bound => bound,
    }
}

#[derive(Clone)]
struct Service {
    sink: Arc<dyn Sink>,
}

impl Service {
    async fn traces(&self, token: Option<String>, request: &ExportTraceServiceRequest) {
        let decoded = decode::traces(request);
        if decoded.without_run > 0 {
            tracing::debug!(spans = decoded.without_run, "spans of no run, dropped");
        }
        if !decoded.kept.is_empty() {
            let sink = Arc::clone(&self.sink);
            let _ = tokio::task::spawn_blocking(move || sink.spans(token.as_deref(), decoded.kept))
                .await;
        }
    }

    async fn metrics(&self, token: Option<String>, request: &ExportMetricsServiceRequest) {
        let decoded = decode::metrics(request);
        if decoded.without_run > 0 {
            tracing::debug!(
                points = decoded.without_run,
                "metric points of no run, dropped"
            );
        }
        if !decoded.kept.is_empty() {
            let sink = Arc::clone(&self.sink);
            let _ =
                tokio::task::spawn_blocking(move || sink.metrics(token.as_deref(), decoded.kept))
                    .await;
        }
    }
}

#[tonic::async_trait]
impl TraceService for Service {
    async fn export(
        &self,
        request: tonic::Request<ExportTraceServiceRequest>,
    ) -> Result<tonic::Response<ExportTraceServiceResponse>, tonic::Status> {
        let token = grpc_token(&request);
        self.traces(token, request.get_ref()).await;
        Ok(tonic::Response::new(ExportTraceServiceResponse::default()))
    }
}

#[tonic::async_trait]
impl MetricsService for Service {
    async fn export(
        &self,
        request: tonic::Request<ExportMetricsServiceRequest>,
    ) -> Result<tonic::Response<ExportMetricsServiceResponse>, tonic::Status> {
        let token = grpc_token(&request);
        self.metrics(token, request.get_ref()).await;
        Ok(tonic::Response::new(ExportMetricsServiceResponse::default()))
    }
}

fn grpc_token<T>(request: &tonic::Request<T>) -> Option<String> {
    request
        .metadata()
        .get(TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

fn http_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

/// How an HTTP request is encoded, from its `Content-Type` (OTLP/HTTP).
#[derive(Clone, Copy)]
enum Encoding {
    Protobuf,
    Json,
}

/// Why a request is refused: its status and what to do.
type Refused = (StatusCode, String);

fn encoding(headers: &HeaderMap) -> Result<Encoding, Refused> {
    if headers
        .get(header::CONTENT_ENCODING)
        .is_some_and(|e| e != "identity")
    {
        return Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "compressed OTLP is not read yet: send it uncompressed".to_owned(),
        ));
    }
    let kind = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if kind.starts_with("application/json") {
        Ok(Encoding::Json)
    } else if kind.starts_with("application/x-protobuf") {
        Ok(Encoding::Protobuf)
    } else {
        Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "OTLP/HTTP is application/x-protobuf or application/json".to_owned(),
        ))
    }
}

fn decode<T: Message + Default + serde::de::DeserializeOwned>(
    encoding: Encoding,
    body: &Bytes,
) -> Result<T, Refused> {
    let decoded = match encoding {
        Encoding::Protobuf => T::decode(body.as_ref()).map_err(|e| e.to_string()),
        Encoding::Json => serde_json::from_slice(body).map_err(|e| e.to_string()),
    };
    decoded.map_err(|error| (StatusCode::BAD_REQUEST, error))
}

/// The answer OTLP/HTTP expects: the response message, in the request's encoding.
fn answer<T: Message + serde::Serialize>(encoding: Encoding, response: &T) -> Response {
    match encoding {
        Encoding::Protobuf => (
            [(header::CONTENT_TYPE, "application/x-protobuf")],
            response.encode_to_vec(),
        )
            .into_response(),
        Encoding::Json => (
            [(header::CONTENT_TYPE, "application/json")],
            serde_json::to_vec(response).unwrap_or_default(),
        )
            .into_response(),
    }
}

fn router(sink: Arc<dyn Sink>) -> axum::Router {
    let service = Service { sink };
    let traces = service.clone();
    let metrics = service.clone();
    tonic::service::Routes::new(
        TraceServiceServer::new(service.clone()).max_decoding_message_size(MESSAGE_BYTES),
    )
    .add_service(MetricsServiceServer::new(service).max_decoding_message_size(MESSAGE_BYTES))
    .into_axum_router()
    .route(
        "/v1/traces",
        post(move |headers: HeaderMap, body: Bytes| async move {
            let encoding = match encoding(&headers) {
                Ok(encoding) => encoding,
                Err(refused) => return refused.into_response(),
            };
            match decode::<ExportTraceServiceRequest>(encoding, &body) {
                Ok(request) => {
                    traces.traces(http_token(&headers), &request).await;
                    answer(encoding, &ExportTraceServiceResponse::default())
                }
                Err(refused) => refused.into_response(),
            }
        }),
    )
    .route(
        "/v1/metrics",
        post(move |headers: HeaderMap, body: Bytes| async move {
            let encoding = match encoding(&headers) {
                Ok(encoding) => encoding,
                Err(refused) => return refused.into_response(),
            };
            match decode::<ExportMetricsServiceRequest>(encoding, &body) {
                Ok(request) => {
                    metrics.metrics(http_token(&headers), &request).await;
                    answer(encoding, &ExportMetricsServiceResponse::default())
                }
                Err(refused) => refused.into_response(),
            }
        }),
    )
    .layer(DefaultBodyLimit::max(MESSAGE_BYTES))
}
