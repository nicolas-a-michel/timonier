//! A blocking client of the daemon (ADR 0019): the CLI's, and the integration tests'. The
//! extension has its own, on vscode-jsonrpc.

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use timonier_core::protocol::{InitializeParams, InitializeResult, PROTOCOL_VERSION, methods};
use timonier_core::rpc::{Decoder, ErrorObject, FrameError, Id, InvalidMessage, Message};

/// How long a request waits for its answer, by default.
pub const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("cannot reach the daemon: {0}")]
    Io(#[from] std::io::Error),
    #[error("the daemon closed the connection")]
    Closed,
    #[error("no answer from the daemon within {0:?}")]
    Timeout(Duration),
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error(transparent)]
    Invalid(#[from] InvalidMessage),
    #[error("the daemon refused: {0}")]
    Rpc(ErrorObject),
    #[error("unexpected answer: {0}")]
    Decode(#[from] serde_json::Error),
}

/// A message the daemon sent on its own: a notification, or a request to the client.
#[derive(Clone, Debug, PartialEq)]
pub struct Incoming {
    pub method: String,
    pub params: Value,
    /// Set for a request, which waits for an answer.
    pub id: Option<Id>,
}

pub struct Client {
    stream: UnixStream,
    decoder: Decoder,
    next_id: i64,
    incoming: VecDeque<Incoming>,
    timeout: Duration,
}

impl Client {
    pub fn connect(socket: &Path) -> Result<Self, ClientError> {
        Ok(Self {
            stream: UnixStream::connect(socket)?,
            decoder: Decoder::default(),
            next_id: 1,
            incoming: VecDeque::new(),
            timeout: TIMEOUT,
        })
    }

    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// `initialize`, with this daemon's protocol version.
    pub fn initialize(&mut self, client: &str) -> Result<InitializeResult, ClientError> {
        self.request(
            methods::INITIALIZE,
            &InitializeParams {
                protocol_version: PROTOCOL_VERSION,
                client: client.to_owned(),
                lsp_relay: false,
            },
        )
    }

    /// Sends a request and waits for its answer. What the daemon sends meanwhile is kept for
    /// [`Client::next_incoming`].
    pub fn request<R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: &impl Serialize,
    ) -> Result<R, ClientError> {
        let id = Id::Number(self.next_id);
        self.next_id += 1;
        let params = serde_json::to_value(params)?;
        self.write(&Message::request(id.clone(), method, params))?;
        let deadline = Instant::now() + self.timeout;
        loop {
            let Some(message) = self.read(deadline)? else {
                return Err(ClientError::Timeout(self.timeout));
            };
            match message {
                Message::Response {
                    id: Some(answered),
                    outcome,
                } if answered == id => {
                    return match outcome {
                        Ok(result) => Ok(serde_json::from_value(result)?),
                        Err(error) => Err(ClientError::Rpc(error)),
                    };
                }
                Message::Response {
                    id: None,
                    outcome: Err(error),
                } => {
                    return Err(ClientError::Rpc(error));
                }
                Message::Response { .. } => {}
                Message::Notification { method, params } => self.incoming.push_back(Incoming {
                    method,
                    params,
                    id: None,
                }),
                Message::Request { id, method, params } => self.incoming.push_back(Incoming {
                    method,
                    params,
                    id: Some(id),
                }),
            }
        }
    }

    pub fn notify(&mut self, method: &str, params: &impl Serialize) -> Result<(), ClientError> {
        let params = serde_json::to_value(params)?;
        self.write(&Message::notification(method, params))
    }

    /// The next message the daemon sent on its own, waiting up to `timeout`.
    pub fn next_incoming(&mut self, timeout: Duration) -> Result<Option<Incoming>, ClientError> {
        if let Some(incoming) = self.incoming.pop_front() {
            return Ok(Some(incoming));
        }
        let deadline = Instant::now() + timeout;
        loop {
            match self.read(deadline)? {
                None => return Ok(None),
                Some(Message::Notification { method, params }) => {
                    return Ok(Some(Incoming {
                        method,
                        params,
                        id: None,
                    }));
                }
                Some(Message::Request { id, method, params }) => {
                    return Ok(Some(Incoming {
                        method,
                        params,
                        id: Some(id),
                    }));
                }
                Some(Message::Response { .. }) => {}
            }
        }
    }

    /// Answers a request the daemon sent (`lsp/outgoingCalls`).
    pub fn respond(&mut self, id: Id, result: &impl Serialize) -> Result<(), ClientError> {
        let result = serde_json::to_value(result)?;
        self.write(&Message::result(id, result))
    }

    /// Answers a request the daemon sent with an error (ADR 0022: `ContentModified`).
    pub fn refuse(&mut self, id: Id, error: ErrorObject) -> Result<(), ClientError> {
        self.write(&Message::error(Some(id), error))
    }

    /// Sends raw bytes: for tests of malformed input.
    pub fn send_raw(&mut self, bytes: &[u8]) -> Result<(), ClientError> {
        self.stream.write_all(bytes)?;
        Ok(())
    }

    /// The next message, raw: for tests of malformed input.
    pub fn read_raw(&mut self, timeout: Duration) -> Result<Option<Message>, ClientError> {
        self.read(Instant::now() + timeout)
    }

    fn write(&mut self, message: &Message) -> Result<(), ClientError> {
        self.stream.write_all(&message.frame())?;
        Ok(())
    }

    fn read(&mut self, deadline: Instant) -> Result<Option<Message>, ClientError> {
        let mut buffer = [0; 64 << 10];
        loop {
            if let Some(body) = self.decoder.next_body()? {
                return Ok(Some(Message::parse(&body)?));
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(None);
            }
            self.stream.set_read_timeout(Some(left))?;
            match self.stream.read(&mut buffer) {
                Ok(0) => return Err(ClientError::Closed),
                Ok(n) => self.decoder.push(&buffer[..n]),
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    return Ok(None);
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
}
