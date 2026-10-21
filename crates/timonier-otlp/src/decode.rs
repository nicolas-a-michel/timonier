//! From OTLP messages to what Timonier keeps (ADR 0048): the spans and metric points of the
//! runs, their `code.*` attributes and their test. Pure.

use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue, any_value};
use opentelemetry_proto::tonic::metrics::v1::{metric, number_data_point};
use serde_json::{Map, Value};
use timonier_core::perf::{CodeLocation, MetricKind, ReceivedMetric, ReceivedSpan, SpanStatus};

/// The attributes kept besides those read into fields, at most this many bytes of JSON.
const ATTRIBUTES_BYTES: usize = 4 << 10;

/// What a message held: what belongs to a run, and how much did not.
#[derive(Debug, PartialEq)]
pub struct Decoded<T> {
    pub kept: Vec<T>,
    /// Spans or points with no `timonier.run_id`: not a run's (ADR 0048).
    pub without_run: usize,
}

impl<T> Default for Decoded<T> {
    fn default() -> Self {
        Self {
            kept: Vec::new(),
            without_run: 0,
        }
    }
}

fn json(value: Option<&AnyValue>) -> Value {
    match value.and_then(|v| v.value.as_ref()) {
        Some(any_value::Value::StringValue(s)) => Value::String(s.clone()),
        Some(any_value::Value::BoolValue(b)) => Value::Bool(*b),
        Some(any_value::Value::IntValue(i)) => Value::from(*i),
        Some(any_value::Value::DoubleValue(d)) => Value::from(*d),
        Some(any_value::Value::ArrayValue(array)) => {
            Value::Array(array.values.iter().map(|v| json(Some(v))).collect())
        }
        Some(any_value::Value::KvlistValue(list)) => Value::Object(object(&list.values)),
        Some(any_value::Value::BytesValue(bytes)) => Value::String(hex(bytes)),
        Some(any_value::Value::StringValueStrindex(_)) | None => Value::Null,
    }
}

fn object(attributes: &[KeyValue]) -> Map<String, Value> {
    attributes
        .iter()
        .map(|kv| (kv.key.clone(), json(kv.value.as_ref())))
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn text(attributes: &Map<String, Value>, key: &str) -> Option<String> {
    match attributes.get(key)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// The attribute of the span, else of its resource.
fn either(span: &Map<String, Value>, resource: &Map<String, Value>, key: &str) -> Option<String> {
    text(span, key).or_else(|| text(resource, key))
}

/// The `code.*` attributes; `code.function.name` derived for a `tracing` span (ADR 0044).
fn code(attributes: &Map<String, Value>, span_name: &str) -> CodeLocation {
    let line = attributes.get("code.line.number").and_then(|v| {
        v.as_u64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    });
    let function = text(attributes, "code.function.name");
    let module = text(attributes, "code.module.name");
    let derived = function.is_none() && module.is_some();
    CodeLocation {
        function: function.or_else(|| module.map(|m| format!("{m}::{span_name}"))),
        function_derived: derived,
        file: text(attributes, "code.file.path"),
        line,
    }
}

/// Read into fields, so not kept again among the other attributes.
const READ: [&str; 7] = [
    "code.function.name",
    "code.module.name",
    "code.file.path",
    "code.line.number",
    "timonier.run_id",
    "timonier.test.name",
    "timonier.test.classname",
];

/// The other attributes, while they fit in [`ATTRIBUTES_BYTES`]: the first ones, by key.
fn rest(attributes: Map<String, Value>) -> Map<String, Value> {
    let mut kept = Map::new();
    let mut bytes = 2;
    for (key, value) in attributes {
        if READ.contains(&key.as_str()) {
            continue;
        }
        let size = key.len() + value.to_string().len() + 4;
        if bytes + size > ATTRIBUTES_BYTES {
            kept.insert("timonier.attributes_truncated".into(), true.into());
            break;
        }
        bytes += size;
        kept.insert(key, value);
    }
    kept
}

#[tracing::instrument(skip_all)]
pub fn traces(request: &ExportTraceServiceRequest) -> Decoded<ReceivedSpan> {
    let mut decoded = Decoded::default();
    for resource_spans in &request.resource_spans {
        let resource = resource_spans
            .resource
            .as_ref()
            .map(|r| object(&r.attributes))
            .unwrap_or_default();
        let language = text(&resource, "telemetry.sdk.language");
        for scope in &resource_spans.scope_spans {
            for span in &scope.spans {
                let attributes = object(&span.attributes);
                let Some(run) = either(&attributes, &resource, "timonier.run_id") else {
                    decoded.without_run += 1;
                    continue;
                };
                let status = match span.status.as_ref().map(|s| s.code) {
                    Some(1) => SpanStatus::Ok,
                    Some(2) => SpanStatus::Error,
                    _ => SpanStatus::Unset,
                };
                decoded.kept.push(ReceivedSpan {
                    run,
                    trace_id: hex(&span.trace_id),
                    span_id: hex(&span.span_id),
                    parent_span_id: (!span.parent_span_id.is_empty())
                        .then(|| hex(&span.parent_span_id)),
                    name: span.name.clone(),
                    start_ns: span.start_time_unix_nano,
                    end_ns: span.end_time_unix_nano.max(span.start_time_unix_nano),
                    status,
                    test_name: either(&attributes, &resource, "timonier.test.name"),
                    test_classname: either(&attributes, &resource, "timonier.test.classname"),
                    code: code(&attributes, &span.name),
                    language: language.clone(),
                    attributes: rest(attributes),
                });
            }
        }
    }
    decoded
}

#[tracing::instrument(skip_all)]
pub fn metrics(request: &ExportMetricsServiceRequest) -> Decoded<ReceivedMetric> {
    let mut decoded = Decoded::default();
    for resource_metrics in &request.resource_metrics {
        let resource = resource_metrics
            .resource
            .as_ref()
            .map(|r| object(&r.attributes))
            .unwrap_or_default();
        for scope in &resource_metrics.scope_metrics {
            for metric in &scope.metrics {
                let mut point = |kind, attributes: &[KeyValue], time_ns, value, count, sum| {
                    let attributes = object(attributes);
                    let Some(run) = either(&attributes, &resource, "timonier.run_id") else {
                        decoded.without_run += 1;
                        return;
                    };
                    decoded.kept.push(ReceivedMetric {
                        run,
                        name: metric.name.clone(),
                        kind,
                        time_ns,
                        value,
                        count,
                        sum,
                        attributes: rest(attributes),
                    });
                };
                let number =
                    |p: &opentelemetry_proto::tonic::metrics::v1::NumberDataPoint| match p.value {
                        Some(number_data_point::Value::AsDouble(d)) => Some(d),
                        #[allow(clippy::cast_precision_loss)]
                        Some(number_data_point::Value::AsInt(i)) => Some(i as f64),
                        None => None,
                    };
                match &metric.data {
                    Some(metric::Data::Sum(sum)) => {
                        for p in &sum.data_points {
                            point(
                                MetricKind::Sum,
                                &p.attributes,
                                p.time_unix_nano,
                                number(p),
                                None,
                                None,
                            );
                        }
                    }
                    Some(metric::Data::Gauge(gauge)) => {
                        for p in &gauge.data_points {
                            point(
                                MetricKind::Gauge,
                                &p.attributes,
                                p.time_unix_nano,
                                number(p),
                                None,
                                None,
                            );
                        }
                    }
                    Some(metric::Data::Histogram(histogram)) => {
                        for p in &histogram.data_points {
                            point(
                                MetricKind::Histogram,
                                &p.attributes,
                                p.time_unix_nano,
                                None,
                                Some(p.count),
                                p.sum,
                            );
                        }
                    }
                    // Exponential histograms and summaries: not read yet.
                    _ => {}
                }
            }
        }
    }
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An OTLP/JSON message as the specification writes it: hex ids, times as strings.
    const TRACES: &str = r#"{"resourceSpans":[{
        "resource":{"attributes":[
            {"key":"service.name","value":{"stringValue":"demo"}},
            {"key":"telemetry.sdk.language","value":{"stringValue":"rust"}},
            {"key":"timonier.run_id","value":{"stringValue":"R-3"}},
            {"key":"timonier.test.name","value":{"stringValue":"parses"}}]},
        "scopeSpans":[{"scope":{"name":"demo"},"spans":[
            {"traceId":"5b8efff798038103d269b633813fc60c","spanId":"eee19b7ec3c1b174",
             "parentSpanId":"eee19b7ec3c1b173","name":"parse",
             "startTimeUnixNano":"1544712660000000000","endTimeUnixNano":"1544712661000000000",
             "status":{"code":2},
             "attributes":[
                {"key":"code.module.name","value":{"stringValue":"demo::parser"}},
                {"key":"code.file.path","value":{"stringValue":"src/parser.rs"}},
                {"key":"code.line.number","value":{"intValue":"42"}},
                {"key":"timonier.payload.bytes","value":{"intValue":"512"}}]},
            {"traceId":"5b8efff798038103d269b633813fc60c","spanId":"eee19b7ec3c1b175",
             "name":"named","startTimeUnixNano":"1","endTimeUnixNano":"2",
             "attributes":[
                {"key":"code.function.name","value":{"stringValue":"demo::view::draw"}},
                {"key":"timonier.test.name","value":{"stringValue":"renders"}}]}]}]},
        {"resource":{"attributes":[]},"scopeSpans":[{"spans":[
            {"traceId":"00","spanId":"01","name":"elsewhere"}]}]}]}"#;

    #[test]
    fn spans_of_a_run_keep_their_code_and_test() {
        let request: ExportTraceServiceRequest = serde_json::from_str(TRACES).unwrap();
        let decoded = traces(&request);
        assert_eq!(decoded.without_run, 1);
        let [parse, named] = decoded.kept.as_slice() else {
            panic!("{decoded:?}")
        };
        assert_eq!(parse.run, "R-3");
        assert_eq!(parse.trace_id, "5b8efff798038103d269b633813fc60c");
        assert_eq!(parse.parent_span_id.as_deref(), Some("eee19b7ec3c1b173"));
        assert_eq!(
            (parse.start_ns, parse.end_ns),
            (1_544_712_660_000_000_000, 1_544_712_661_000_000_000)
        );
        assert_eq!(parse.status, SpanStatus::Error);
        assert_eq!(parse.language.as_deref(), Some("rust"));
        // ADR 0044: derived from the module and the span's name.
        assert_eq!(
            parse.code,
            CodeLocation {
                function: Some("demo::parser::parse".to_owned()),
                function_derived: true,
                file: Some("src/parser.rs".to_owned()),
                line: Some(42),
            }
        );
        // The resource's test, unless the span says its own.
        assert_eq!(parse.test_name.as_deref(), Some("parses"));
        assert_eq!(named.test_name.as_deref(), Some("renders"));
        assert_eq!(named.code.function.as_deref(), Some("demo::view::draw"));
        assert!(!named.code.function_derived);
        assert_eq!(named.parent_span_id, None);
        // Read into fields, not kept twice.
        assert_eq!(
            parse.attributes.get("timonier.payload.bytes"),
            Some(&Value::from(512))
        );
        assert!(!parse.attributes.contains_key("code.file.path"));
    }

    #[test]
    fn many_attributes_are_cut_and_say_so() {
        let attributes: Map<String, Value> = (0..400)
            .map(|i| (format!("k{i:03}"), Value::String("x".repeat(20))))
            .collect();
        let kept = rest(attributes);
        assert!(kept.len() < 400);
        assert_eq!(kept["timonier.attributes_truncated"], true);
        assert!(Value::Object(kept).to_string().len() <= ATTRIBUTES_BYTES + 64);
    }

    #[test]
    fn metric_points_of_a_run_are_kept() {
        let request: ExportMetricsServiceRequest = serde_json::from_str(
            r#"{"resourceMetrics":[{"resource":{"attributes":[
                {"key":"timonier.run_id","value":{"stringValue":"R-1"}}]},
              "scopeMetrics":[{"metrics":[
                {"name":"timonier.rows","sum":{"dataPoints":[{"asInt":"7","timeUnixNano":"5"}],
                  "aggregationTemporality":2,"isMonotonic":true}},
                {"name":"latency","histogram":{"dataPoints":[{"count":"3","sum":1.5,
                  "timeUnixNano":"6"}],"aggregationTemporality":2}}]}]}]}"#,
        )
        .unwrap();
        let decoded = metrics(&request);
        assert_eq!(decoded.without_run, 0);
        assert_eq!(decoded.kept.len(), 2);
        assert_eq!(
            (decoded.kept[0].kind, decoded.kept[0].value),
            (MetricKind::Sum, Some(7.0))
        );
        assert_eq!(
            (
                decoded.kept[1].kind,
                decoded.kept[1].count,
                decoded.kept[1].sum
            ),
            (MetricKind::Histogram, Some(3), Some(1.5))
        );
    }
}
