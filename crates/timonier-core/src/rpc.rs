//! JSON-RPC 2.0 messages, framed by `Content-Length` like the base protocol of LSP (ADR 0019).
//! Pure: the daemon and its clients do the I/O, and feed the bytes they read to a [`Decoder`].

use serde_json::{Map, Value};

/// The largest message accepted. A view of a large repository is a few megabytes.
pub const MAX_MESSAGE: usize = 64 << 20;

/// A request id: a number or a string.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Id {
    Number(i64),
    String(String),
}

impl Id {
    fn to_value(&self) -> Value {
        match self {
            Self::Number(n) => Value::from(*n),
            Self::String(s) => Value::from(s.as_str()),
        }
    }

    fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Number(n) => n.as_i64().map(Self::Number),
            Value::String(s) => Some(Self::String(s.clone())),
            _ => None,
        }
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "{s:?}"),
        }
    }
}

/// The error of a response.
#[derive(Clone, Debug, PartialEq)]
pub struct ErrorObject {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl ErrorObject {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    fn to_value(&self) -> Value {
        let mut object = Map::new();
        object.insert("code".into(), Value::from(self.code));
        object.insert("message".into(), Value::from(self.message.as_str()));
        if let Some(data) = &self.data {
            object.insert("data".into(), data.clone());
        }
        Value::Object(object)
    }

    fn from_value(value: &Value) -> Option<Self> {
        Some(Self {
            code: value.get("code")?.as_i64()?,
            message: value.get("message")?.as_str()?.to_owned(),
            data: value.get("data").cloned(),
        })
    }
}

impl std::fmt::Display for ErrorObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for ErrorObject {}

/// One message, in either direction. Absent params are `Value::Null`.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Request {
        id: Id,
        method: String,
        params: Value,
    },
    Notification {
        method: String,
        params: Value,
    },
    /// `id` is `None` only for an error about a message whose id could not be read.
    Response {
        id: Option<Id>,
        outcome: Result<Value, ErrorObject>,
    },
}

/// Why bytes are not a message. The connection can go on: the next frame is independent.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InvalidMessage {
    #[error("not JSON: {0}")]
    Json(String),
    #[error("not a JSON-RPC 2.0 message: {0}")]
    Shape(&'static str),
}

impl Message {
    pub fn request(id: Id, method: &str, params: Value) -> Self {
        Self::Request {
            id,
            method: method.to_owned(),
            params,
        }
    }

    pub fn notification(method: &str, params: Value) -> Self {
        Self::Notification {
            method: method.to_owned(),
            params,
        }
    }

    pub fn result(id: Id, result: Value) -> Self {
        Self::Response {
            id: Some(id),
            outcome: Ok(result),
        }
    }

    pub fn error(id: Option<Id>, error: ErrorObject) -> Self {
        Self::Response {
            id,
            outcome: Err(error),
        }
    }

    pub fn to_value(&self) -> Value {
        let mut object = Map::new();
        object.insert("jsonrpc".into(), Value::from("2.0"));
        match self {
            Self::Request { id, method, params } => {
                object.insert("id".into(), id.to_value());
                object.insert("method".into(), Value::from(method.as_str()));
                if !params.is_null() {
                    object.insert("params".into(), params.clone());
                }
            }
            Self::Notification { method, params } => {
                object.insert("method".into(), Value::from(method.as_str()));
                if !params.is_null() {
                    object.insert("params".into(), params.clone());
                }
            }
            Self::Response { id, outcome } => {
                object.insert("id".into(), id.as_ref().map_or(Value::Null, Id::to_value));
                match outcome {
                    Ok(result) => object.insert("result".into(), result.clone()),
                    Err(error) => object.insert("error".into(), error.to_value()),
                };
            }
        }
        Value::Object(object)
    }

    /// Reads one message. A request whose shape is wrong but whose id is readable gets that id
    /// back, so that its error can answer it: see [`Message::id_of`].
    pub fn parse(body: &[u8]) -> Result<Self, InvalidMessage> {
        let value: Value =
            serde_json::from_slice(body).map_err(|e| InvalidMessage::Json(e.to_string()))?;
        Self::from_value(&value)
    }

    pub fn from_value(value: &Value) -> Result<Self, InvalidMessage> {
        let object = value
            .as_object()
            .ok_or(InvalidMessage::Shape("not an object"))?;
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Err(InvalidMessage::Shape("`jsonrpc` is not \"2.0\""));
        }
        let id = match object.get("id") {
            None => None,
            Some(Value::Null) => Some(None),
            Some(id) => Some(Some(
                Id::from_value(id)
                    .ok_or(InvalidMessage::Shape("`id` is not a number or string"))?,
            )),
        };
        let params = match object.get("params") {
            None => Value::Null,
            Some(params @ (Value::Object(_) | Value::Array(_))) => params.clone(),
            Some(_) => return Err(InvalidMessage::Shape("`params` is not an object or array")),
        };
        if let Some(method) = object.get("method") {
            let method = method
                .as_str()
                .ok_or(InvalidMessage::Shape("`method` is not a string"))?
                .to_owned();
            return match id {
                None => Ok(Self::Notification { method, params }),
                Some(Some(id)) => Ok(Self::Request { id, method, params }),
                Some(None) => Err(InvalidMessage::Shape("a request id is null")),
            };
        }
        let id = id.ok_or(InvalidMessage::Shape("a response has no `id`"))?;
        let outcome = match (object.get("result"), object.get("error")) {
            (Some(result), None) => Ok(result.clone()),
            (None, Some(error)) => Err(ErrorObject::from_value(error)
                .ok_or(InvalidMessage::Shape("`error` has no code or message"))?),
            _ => {
                return Err(InvalidMessage::Shape(
                    "a response has either `result` or `error`",
                ));
            }
        };
        Ok(Self::Response { id, outcome })
    }

    /// The id of a request that [`Message::parse`] refused, when it can be read: its error
    /// response must carry it (JSON-RPC 2.0, « Invalid Request »).
    pub fn id_of(body: &[u8]) -> Option<Id> {
        let value: Value = serde_json::from_slice(body).ok()?;
        value.get("method")?;
        Id::from_value(value.get("id")?)
    }

    /// The message with its `Content-Length` header, ready to write.
    pub fn frame(&self) -> Vec<u8> {
        frame(self.to_value().to_string().as_bytes())
    }
}

/// `body` behind its `Content-Length` header.
pub fn frame(body: &[u8]) -> Vec<u8> {
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(body);
    out
}

/// Why a stream of frames cannot be read any further: the connection must close.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    #[error("a header is not `Name: value` or is not UTF-8")]
    Header,
    #[error("no `Content-Length` header")]
    NoLength,
    #[error("a message of {0} bytes is over the limit of {MAX_MESSAGE}")]
    TooLarge(usize),
}

/// Cuts a byte stream into message bodies.
#[derive(Debug, Default)]
pub struct Decoder {
    buffer: Vec<u8>,
}

/// Headers longer than this are not headers.
const MAX_HEADERS: usize = 8 << 10;

impl Decoder {
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// The next complete body, if the bytes pushed so far hold one.
    pub fn next_body(&mut self) -> Result<Option<Vec<u8>>, FrameError> {
        let Some(end) = find(&self.buffer, b"\r\n\r\n") else {
            if self.buffer.len() > MAX_HEADERS {
                return Err(FrameError::Header);
            }
            return Ok(None);
        };
        let headers = std::str::from_utf8(&self.buffer[..end]).map_err(|_| FrameError::Header)?;
        let mut length = None;
        for line in headers.split("\r\n") {
            let (name, value) = line.split_once(':').ok_or(FrameError::Header)?;
            if name.trim().eq_ignore_ascii_case("content-length") {
                let value: usize = value.trim().parse().map_err(|_| FrameError::Header)?;
                length = Some(value);
            }
        }
        let length = length.ok_or(FrameError::NoLength)?;
        if length > MAX_MESSAGE {
            return Err(FrameError::TooLarge(length));
        }
        let start = end + 4;
        if self.buffer.len() < start + length {
            return Ok(None);
        }
        let body = self.buffer[start..start + length].to_vec();
        self.buffer.drain(..start + length);
        Ok(Some(body))
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn messages_go_through_json_and_back() {
        let messages = [
            Message::request(Id::Number(1), "initialize", json!({"protocolVersion": 1})),
            Message::request(Id::String("a".into()), "daemon/status", Value::Null),
            Message::notification("view/update", json!({"subscription": 1})),
            Message::result(Id::Number(1), Value::Null),
            Message::error(
                None,
                ErrorObject::new(-32700, "parse error").with_data(json!({"at": 3})),
            ),
        ];
        for message in messages {
            let value = message.to_value();
            assert_eq!(value["jsonrpc"], "2.0");
            assert_eq!(Message::from_value(&value).unwrap(), message, "{value}");
        }
        // A null result is a result, and a null id is kept.
        let value = Message::result(Id::Number(2), Value::Null).to_value();
        assert!(value.as_object().unwrap().contains_key("result"));
        let value = Message::error(None, ErrorObject::new(-32700, "x")).to_value();
        assert_eq!(value["id"], Value::Null);
    }

    #[test]
    fn malformed_messages_are_refused() {
        for body in [
            &br#"{"jsonrpc":"1.0","id":1,"method":"m"}"#[..],
            br#"{"jsonrpc":"2.0","id":1,"method":3}"#,
            br#"{"jsonrpc":"2.0","id":true,"method":"m"}"#,
            br#"{"jsonrpc":"2.0","id":1,"method":"m","params":3}"#,
            br#"{"jsonrpc":"2.0","id":1}"#,
            br#"{"jsonrpc":"2.0","id":1,"result":1,"error":{"code":1,"message":"m"}}"#,
            br#"[1]"#,
        ] {
            assert!(
                Message::parse(body).is_err(),
                "{}",
                String::from_utf8_lossy(body)
            );
        }
        assert!(matches!(
            Message::parse(b"{nope"),
            Err(InvalidMessage::Json(_))
        ));
        let refused = br#"{"jsonrpc":"2.0","id":7,"method":"m","params":3}"#;
        assert_eq!(Message::id_of(refused), Some(Id::Number(7)));
        assert_eq!(Message::id_of(br#"{"jsonrpc":"2.0","id":7}"#), None);
    }

    #[test]
    fn frames_are_cut_wherever_the_bytes_stop() {
        let first = Message::notification("a", json!({"x": "é"})).frame();
        let second = Message::request(Id::Number(1), "b", Value::Null).frame();
        let stream: Vec<u8> = [first, second].concat();
        // Every split point gives the same two messages.
        for cut in 0..stream.len() {
            let mut decoder = Decoder::default();
            let mut bodies = Vec::new();
            for part in [&stream[..cut], &stream[cut..]] {
                decoder.push(part);
                while let Some(body) = decoder.next_body().unwrap() {
                    bodies.push(Message::parse(&body).unwrap());
                }
            }
            assert_eq!(bodies.len(), 2, "cut at {cut}");
        }
    }

    #[test]
    fn headers_follow_the_lsp_base_protocol() {
        let mut decoder = Decoder::default();
        decoder.push(b"content-length: 2\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n{}");
        assert_eq!(decoder.next_body().unwrap(), Some(b"{}".to_vec()));
        let mut decoder = Decoder::default();
        decoder.push(b"Content-Type: x\r\n\r\n{}");
        assert_eq!(decoder.next_body(), Err(FrameError::NoLength));
        let mut decoder = Decoder::default();
        decoder.push(format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE + 1).as_bytes());
        assert_eq!(
            decoder.next_body(),
            Err(FrameError::TooLarge(MAX_MESSAGE + 1))
        );
        let mut decoder = Decoder::default();
        decoder.push(&vec![b'x'; MAX_HEADERS + 1]);
        assert_eq!(decoder.next_body(), Err(FrameError::Header));
    }
}
