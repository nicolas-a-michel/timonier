//! The Timonier MCP server: a thin proxy to the daemon, with no business logic and no storage
//! access (ADR 0027). MCP's `tools/list` and `tools/call` become the daemon's methods of the same
//! names over its socket, and their answers go back as they are: the catalogue, the validation,
//! the writes and the mailbox header (ADR 0028) are the daemon's.
//!
//! See the `mcp-tools` skill.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt};
use serde_json::{Value, json};
use timonier_core::protocol::{PROTOCOL_VERSION, methods};
use timonier_core::rpc::{Decoder, ErrorObject, Id, Message};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::net::unix::OwnedWriteHalf;
use tokio::sync::{Mutex, OnceCell, oneshot};

/// What the server tells the agent once, at the start.
const INSTRUCTIONS: &str = "Timonier keeps this project's plan, todo and session journal. Open a \
session with session_start before changing the code, keep the todo up to date with todo_add \
and todo_complete, update the plan's tasks with plan_update_task, and close the session with \
session_end. Every result starts with the mailbox header.";

/// Why the proxy stopped.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot reach the daemon at `{path}`: {source}")]
    Connect {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("MCP: {0}")]
    Mcp(String),
}

/// [`serve`], on a runtime of its own: for the command line.
pub fn run(socket: &Path) -> Result<(), Error> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::Mcp(e.to_string()))?
        .block_on(serve(socket))
}

/// Serves MCP on the standard input and output, for the daemon listening at `socket`, until the
/// agent goes.
pub async fn serve(socket: &Path) -> Result<(), Error> {
    let link = Link::connect(socket).await?;
    let running = Proxy {
        link,
        initialized: Arc::new(OnceCell::new()),
    }
    .serve(rmcp::transport::stdio())
    .await
    .map_err(|e| Error::Mcp(e.to_string()))?;
    running
        .waiting()
        .await
        .map_err(|e| Error::Mcp(e.to_string()))?;
    Ok(())
}

/// The agent of a client that gives no name (`clientInfo` is optional in MCP 2026-07-28).
const UNNAMED: &str = "mcp-client";

/// The proxy: MCP on one side, the daemon on the other.
#[derive(Clone)]
pub struct Proxy {
    link: Link,
    /// The daemon learns the agent's name once, before the first tool (ADR 0029).
    initialized: Arc<OnceCell<()>>,
}

impl Proxy {
    pub fn new(link: Link) -> Self {
        Self {
            link,
            initialized: Arc::new(OnceCell::new()),
        }
    }

    /// The daemon's answer to `method`, once the connection is introduced.
    async fn ask(
        &self,
        context: &RequestContext<RoleServer>,
        method: &str,
        params: Value,
    ) -> Result<Value, ErrorData> {
        // From the handshake, or from each request's metadata (MCP 2026-07-28, SEP-2575, where a
        // client may skip `initialize`): rmcp's `client_info` reads both.
        let agent = context
            .client_info()
            .map_or_else(|| UNNAMED.to_owned(), |info| info.name);
        self.initialized
            .get_or_try_init(|| async {
                self.link
                    .request(
                        methods::INITIALIZE,
                        json!({ "protocolVersion": PROTOCOL_VERSION, "client": agent }),
                    )
                    .await
                    .map(|_| ())
            })
            .await
            .map_err(daemon_error)?;
        self.link
            .request(method, params)
            .await
            .map_err(daemon_error)
    }
}

impl ServerHandler for Proxy {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("timonier", timonier_core::VERSION))
            .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let answer = self.ask(&context, methods::TOOLS_LIST, Value::Null).await?;
        let tools: Vec<Tool> = serde_json::from_value(answer["tools"].clone())
            .map_err(|e| ErrorData::internal_error(format!("the daemon's tools: {e}"), None))?;
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let params = json!({ "name": request.name, "arguments": request.arguments });
        let answer = self.ask(&context, methods::TOOLS_CALL, params).await?;
        let result: CallToolResult = serde_json::from_value(answer)
            .map_err(|e| ErrorData::internal_error(format!("the daemon's result: {e}"), None))?;
        Ok(result.into())
    }
}

fn daemon_error(error: ErrorObject) -> ErrorData {
    ErrorData::internal_error(format!("Timonier daemon: {}", error.message), None)
}

/// The requests sent, by id, each with where its answer goes.
type Waiting = HashMap<i64, oneshot::Sender<Result<Value, ErrorObject>>>;

/// A connection to the daemon that several requests share: each answer goes to its request.
#[derive(Clone)]
pub struct Link {
    writer: Arc<Mutex<OwnedWriteHalf>>,
    waiting: Arc<std::sync::Mutex<Waiting>>,
    next: Arc<AtomicI64>,
}

impl Link {
    pub async fn connect(socket: &Path) -> Result<Self, Error> {
        let stream = UnixStream::connect(socket)
            .await
            .map_err(|source| Error::Connect {
                path: socket.to_path_buf(),
                source,
            })?;
        let (mut reader, writer) = stream.into_split();
        let link = Self {
            writer: Arc::new(Mutex::new(writer)),
            waiting: Arc::default(),
            next: Arc::new(AtomicI64::new(1)),
        };
        let waiting = Arc::clone(&link.waiting);
        tokio::spawn(async move {
            let mut decoder = Decoder::default();
            let mut buffer = vec![0; 64 * 1024];
            loop {
                let read = match reader.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(read) => read,
                };
                decoder.push(&buffer[..read]);
                while let Ok(Some(body)) = decoder.next_body() {
                    // Views and relays are for other clients: only answers matter here.
                    if let Ok(Message::Response {
                        id: Some(Id::Number(id)),
                        outcome,
                    }) = Message::parse(&body)
                        && let Some(answer) = waiting.lock().ok().and_then(|mut w| w.remove(&id))
                    {
                        let _ = answer.send(outcome);
                    }
                }
            }
            // The daemon went: whoever waits learns it.
            if let Ok(mut waiting) = waiting.lock() {
                waiting.clear();
            }
        });
        Ok(link)
    }

    /// Sends a request and waits for its answer.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, ErrorObject> {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let (answer, answered) = oneshot::channel();
        if let Ok(mut waiting) = self.waiting.lock() {
            waiting.insert(id, answer);
        }
        let frame = Message::request(Id::Number(id), method, params).frame();
        let gone = || ErrorObject::new(-32603, "the daemon is gone; start the agent again");
        self.writer
            .lock()
            .await
            .write_all(&frame)
            .await
            .map_err(|_| gone())?;
        answered.await.map_err(|_| gone())?
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shares_the_workspace_version() {
        assert_eq!(timonier_core::VERSION, env!("CARGO_PKG_VERSION"));
    }
}
