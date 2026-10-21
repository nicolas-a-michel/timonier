//! Where the daemon listens (ADR 0019): a Unix socket, or on Windows a named pipe, which the spec
//! names beside it (« local socket, Unix or named pipe »; T-125). The connections are any
//! asynchronous stream: the server splits them the same way.

use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;

#[cfg(unix)]
use tokio::net::UnixListener;
#[cfg(windows)]
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

/// The socket bound before the daemon serves: clients queue on it until it accepts. A named pipe
/// exists only once its first instance is made, inside the runtime: its name is kept until then.
pub(crate) struct Bound {
    #[cfg(unix)]
    listener: std::os::unix::net::UnixListener,
    #[cfg(windows)]
    name: PathBuf,
}

impl Bound {
    pub(crate) fn bind(socket: &Path) -> std::io::Result<Self> {
        #[cfg(unix)]
        {
            // The lock is ours: a socket left there belongs to a daemon that died.
            match std::fs::remove_file(socket) {
                Ok(()) => tracing::info!(socket = %socket.display(), "stale socket replaced"),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            let listener = std::os::unix::net::UnixListener::bind(socket)?;
            listener.set_nonblocking(true)?;
            Ok(Self { listener })
        }
        #[cfg(windows)]
        Ok(Self {
            name: socket.to_path_buf(),
        })
    }

    /// The listener, in the runtime.
    pub(crate) fn listen(self) -> std::io::Result<Listener> {
        #[cfg(unix)]
        {
            UnixListener::from_std(self.listener).map(Listener)
        }
        #[cfg(windows)]
        {
            let next = ServerOptions::new()
                .first_pipe_instance(true)
                .create(&self.name)?;
            Ok(Listener {
                name: self.name,
                next,
            })
        }
    }
}

#[cfg(unix)]
pub(crate) struct Listener(UnixListener);

#[cfg(unix)]
pub(crate) type Connection = tokio::net::UnixStream;

#[cfg(unix)]
impl Listener {
    pub(crate) async fn accept(&mut self) -> std::io::Result<Connection> {
        self.0.accept().await.map(|(stream, _)| stream)
    }
}

/// A named pipe serves one client per instance: the next one is made as soon as one connects.
#[cfg(windows)]
pub(crate) struct Listener {
    name: PathBuf,
    next: NamedPipeServer,
}

#[cfg(windows)]
pub(crate) type Connection = NamedPipeServer;

#[cfg(windows)]
impl Listener {
    pub(crate) async fn accept(&mut self) -> std::io::Result<Connection> {
        self.next.connect().await?;
        let next = ServerOptions::new().create(&self.name)?;
        Ok(std::mem::replace(&mut self.next, next))
    }
}

/// Whether a daemon answers on `socket`.
pub fn answers(socket: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::net::UnixStream::connect(socket).is_ok()
    }
    #[cfg(windows)]
    {
        /// `ERROR_PIPE_BUSY`: there, every instance taken for a moment.
        const PIPE_BUSY: i32 = 231;
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(socket)
        {
            Ok(_) => true,
            Err(error) => error.raw_os_error() == Some(PIPE_BUSY),
        }
    }
}

/// Removes what a stopped daemon leaves: the socket file (a named pipe goes with its server).
pub(crate) fn remove(socket: &Path) {
    if cfg!(unix) {
        let _ = std::fs::remove_file(socket);
    }
}
