//! The receiver with a real OTLP exporter (PLAN.md 8.4, ADR 0048): `opentelemetry-otlp` over
//! gRPC, on either port; OTLP/HTTP in protobuf and JSON; a taken port.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use opentelemetry::KeyValue;
use opentelemetry::trace::{Span, Tracer, TracerProvider};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue as ProtoKeyValue, any_value};
use opentelemetry_proto::tonic::resource::v1::Resource;
use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span as ProtoSpan};
use prost::Message;
use timonier_core::perf::{ReceivedMetric, ReceivedSpan};
use timonier_otlp::{Receiver, Sink};

#[derive(Default)]
struct Kept {
    spans: Mutex<Vec<ReceivedSpan>>,
    metrics: Mutex<Vec<ReceivedMetric>>,
}

impl Sink for Kept {
    fn spans(&self, spans: Vec<ReceivedSpan>) {
        self.spans.lock().expect("not poisoned").extend(spans);
    }
    fn metrics(&self, metrics: Vec<ReceivedMetric>) {
        self.metrics.lock().expect("not poisoned").extend(metrics);
    }
}

impl Kept {
    fn wait_for(&self, spans: usize) -> Vec<ReceivedSpan> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let kept = self.spans.lock().expect("not poisoned").clone();
            if kept.len() >= spans || Instant::now() > deadline {
                return kept;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("the test setup")
}

/// Spans sent the way a test process sends them: the run in the resource.
fn export(rt: &tokio::runtime::Runtime, endpoint: String) {
    let provider = rt.block_on(async {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .expect("the test setup");
        opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(
                opentelemetry_sdk::Resource::builder()
                    .with_attributes([
                        KeyValue::new("timonier.run_id", "R-7"),
                        KeyValue::new("timonier.test.name", "parses"),
                    ])
                    .build(),
            )
            .build()
    });
    let tracer = provider.tracer("fixture");
    tracer.in_span("outer", |_| {
        let mut inner = tracer.start("inner");
        inner.set_attribute(KeyValue::new("code.function.name", "demo::parser::parse"));
        inner.set_attribute(KeyValue::new("code.file.path", "src/parser.rs"));
        inner.set_attribute(KeyValue::new("code.line.number", 12_i64));
        inner.end();
    });
    provider.shutdown().expect("the test setup");
}

#[test]
fn a_real_exporter_sends_its_spans_over_grpc_to_either_port() {
    let rt = runtime();
    for port in ["http", "grpc"] {
        let kept = Arc::new(Kept::default());
        let receiver = rt
            .block_on(Receiver::start_on(kept.clone(), 0, 0))
            .expect("the test setup");
        let address = if port == "http" {
            receiver.http()
        } else {
            receiver.grpc()
        };
        export(&rt, format!("http://{address}"));
        let spans = kept.wait_for(2);
        assert_eq!(spans.len(), 2, "on the {port} port: {spans:?}");
        let inner = spans
            .iter()
            .find(|s| s.name == "inner")
            .expect("the test setup");
        let outer = spans
            .iter()
            .find(|s| s.name == "outer")
            .expect("the test setup");
        assert_eq!(inner.run, "R-7");
        assert_eq!(inner.test_name.as_deref(), Some("parses"));
        assert_eq!(
            inner.parent_span_id.as_deref(),
            Some(outer.span_id.as_str())
        );
        assert_eq!(inner.trace_id, outer.trace_id);
        assert_eq!(inner.code.function.as_deref(), Some("demo::parser::parse"));
        assert_eq!(inner.code.file.as_deref(), Some("src/parser.rs"));
        assert_eq!(inner.code.line, Some(12));
        assert_eq!(inner.language.as_deref(), Some("rust"));
        assert!(inner.end_ns >= inner.start_ns && inner.start_ns > 0);
        rt.block_on(receiver.stop());
    }
}

/// An OTLP/HTTP request by hand: what an HTTP exporter sends.
fn post(address: std::net::SocketAddr, kind: &str, body: &[u8]) -> String {
    let mut stream = TcpStream::connect(address).expect("the test setup");
    write!(
        stream,
        "POST /v1/traces HTTP/1.1\r\nHost: {address}\r\nContent-Type: {kind}\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .expect("the test setup");
    stream.write_all(body).expect("the test setup");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).ok();
    answer
}

fn request(run: &str) -> ExportTraceServiceRequest {
    let text = |value: &str| AnyValue {
        value: Some(any_value::Value::StringValue(value.to_owned())),
    };
    ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            resource: Some(Resource {
                attributes: vec![ProtoKeyValue {
                    key: "timonier.run_id".to_owned(),
                    value: Some(text(run)),
                    ..ProtoKeyValue::default()
                }],
                ..Resource::default()
            }),
            scope_spans: vec![ScopeSpans {
                spans: vec![ProtoSpan {
                    trace_id: vec![1; 16],
                    span_id: vec![2; 8],
                    name: "by hand".to_owned(),
                    start_time_unix_nano: 10,
                    end_time_unix_nano: 20,
                    ..ProtoSpan::default()
                }],
                ..ScopeSpans::default()
            }],
            ..ResourceSpans::default()
        }],
    }
}

#[test]
fn otlp_over_http_reads_protobuf_and_json_and_refuses_the_rest() {
    let rt = runtime();
    let kept = Arc::new(Kept::default());
    let receiver = rt
        .block_on(Receiver::start_on(kept.clone(), 0, 0))
        .expect("the test setup");
    let protobuf = post(
        receiver.http(),
        "application/x-protobuf",
        &request("R-1").encode_to_vec(),
    );
    assert!(protobuf.starts_with("HTTP/1.1 200"), "{protobuf}");
    let json = post(
        receiver.http(),
        "application/json",
        serde_json::to_string(&request("R-2"))
            .expect("the test setup")
            .as_bytes(),
    );
    assert!(json.starts_with("HTTP/1.1 200"), "{json}");
    assert!(json.contains("application/json"), "{json}");
    let spans = kept.wait_for(2);
    let runs: Vec<_> = spans.iter().map(|s| s.run.as_str()).collect();
    assert_eq!(runs, ["R-1", "R-2"]);
    assert_eq!(spans[0].trace_id, "01".repeat(16));
    assert_eq!(spans[0].span_id, "02".repeat(8));

    let wrong = post(receiver.http(), "text/plain", b"hello");
    assert!(wrong.starts_with("HTTP/1.1 415"), "{wrong}");
    let broken = post(
        receiver.http(),
        "application/x-protobuf",
        &[0xff, 0xff, 0xff],
    );
    assert!(broken.starts_with("HTTP/1.1 400"), "{broken}");
    rt.block_on(receiver.stop());
}

#[test]
fn a_taken_port_gives_way_to_one_of_the_system_s_choice() {
    let rt = runtime();
    let taken = std::net::TcpListener::bind("127.0.0.1:0").expect("the test setup");
    let port = taken.local_addr().expect("the test setup").port();
    let receiver = rt
        .block_on(Receiver::start_on(Arc::new(Kept::default()), port, port))
        .expect("the test setup");
    assert_ne!(receiver.http().port(), port);
    assert_ne!(receiver.grpc().port(), port);
    assert!(receiver.http().ip().is_loopback());
    assert_eq!(receiver.endpoint(), format!("http://{}", receiver.http()));
    rt.block_on(receiver.stop());
}
