//! The views of every client, on one thread (ADR 0019). It reads the head once per change,
//! computes each view again, and sends the deltas in order with the answers about the same
//! views: a snapshot always comes before the deltas that follow it.
//!
//! Nothing here waits for a client. A delta that does not fit in a client's queue is dropped:
//! the client sees a gap in the revisions and asks for `view/resync`. An answer that does not
//! fit closes the connection.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Instant;

use serde_json::Value;
use timonier_core::protocol::{ReviewMark, ReviewSummary, ViewSpec, errors, methods};
use timonier_core::review::{self, ReviewDiff};
use timonier_core::rpc::{ErrorObject, Id, Message};
use timonier_core::{ContentHash, SnapshotKind};
use timonier_store::{Reader, Store};
use tokio::sync::Notify;
use tokio::sync::mpsc::{self, error::TrySendError};

use crate::view::{self, Graph, Review, ViewState};

/// Frames waiting to be written to one client.
pub(crate) type Outbox = mpsc::Sender<Vec<u8>>;

pub(crate) enum Command {
    /// A client connected: where its messages go, how to close it, and the flag its writer
    /// reads to say when its full queue has drained.
    Open {
        connection: u64,
        outbox: Outbox,
        close: Arc<Notify>,
        congested: Arc<AtomicBool>,
    },
    /// The client's queue, full before, is empty: the views whose deltas were lost can say so.
    Drained {
        connection: u64,
    },
    Subscribe {
        connection: u64,
        id: Id,
        spec: ViewSpec,
    },
    Resync {
        connection: u64,
        id: Id,
        subscription: u64,
    },
    Unsubscribe {
        connection: u64,
        id: Id,
        subscription: u64,
    },
    Closed {
        connection: u64,
    },
    /// The graph changed: the flag given to [`spawn`] says it too, in case this does not fit.
    Changed,
    Stop,
}

struct Client {
    outbox: Outbox,
    close: Arc<Notify>,
    congested: Arc<AtomicBool>,
}

struct Subscription {
    connection: u64,
    spec: ViewSpec,
    revision: u64,
    state: ViewState,
    /// A delta of this view did not fit in the client's queue: the client does not know yet.
    lost: bool,
}

/// The snapshot of the last review, frozen: read once per review (ADR 0024).
struct Reviewed {
    mark: ReviewMark,
    graph: Graph,
    /// The content hash of each file when it was marked.
    contents: HashMap<String, ContentHash>,
}

/// The head as last read, and its review diff, for every view.
struct Head {
    graph: Graph,
    review: Option<(ReviewDiff, ReviewSummary)>,
}

impl Head {
    fn review<'a>(&'a self, reviewed: Option<&'a Reviewed>) -> Option<Review<'a>> {
        let ((diff, summary), reviewed) = (self.review.as_ref()?, reviewed?);
        Some(Review {
            reviewed: &reviewed.graph,
            diff,
            summary,
        })
    }
}

struct Subscriptions {
    store: Store,
    reader: Option<Reader>,
    /// The head as last read; `None` when it changed since, or nobody looks.
    head: Option<Head>,
    reviewed: Option<Reviewed>,
    clients: HashMap<u64, Client>,
    subscriptions: BTreeMap<u64, Subscription>,
    next: u64,
}

/// Runs the subscriptions until [`Command::Stop`] or until every sender is gone. `changed` is
/// set by the pipeline thread whenever the graph changed.
pub(crate) fn spawn(
    store: Store,
    changed: Arc<AtomicBool>,
    mut commands: mpsc::Receiver<Command>,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("timonier-views".into())
        .spawn(move || {
            let mut subscriptions = Subscriptions {
                store,
                reader: None,
                head: None,
                reviewed: None,
                clients: HashMap::new(),
                subscriptions: BTreeMap::new(),
                next: 1,
            };
            while let Some(command) = commands.blocking_recv() {
                if matches!(command, Command::Stop) {
                    break;
                }
                subscriptions.handle(command);
                if changed.swap(false, Ordering::SeqCst) {
                    subscriptions.head = None;
                    subscriptions.refresh();
                }
            }
        })
}

impl Subscriptions {
    fn handle(&mut self, command: Command) {
        match command {
            Command::Open {
                connection,
                outbox,
                close,
                congested,
            } => {
                self.clients.insert(
                    connection,
                    Client {
                        outbox,
                        close,
                        congested,
                    },
                );
            }
            Command::Drained { connection } => self.signal_lost(connection),
            Command::Subscribe {
                connection,
                id,
                spec,
            } => {
                let state = match self.read_head() {
                    Ok(()) => {
                        let head = self.head.as_ref().expect("read just above");
                        ViewState::compute(
                            &head.graph,
                            &spec,
                            head.review(self.reviewed.as_ref()).as_ref(),
                        )
                    }
                    Err(error) => {
                        tracing::warn!(%error, "cannot read the graph");
                        let error = ErrorObject::new(errors::INTERNAL, error.to_string());
                        self.answer(connection, Message::error(Some(id), error));
                        return;
                    }
                };
                let subscription = self.next;
                self.next += 1;
                let snapshot = state.snapshot(subscription, 0);
                self.subscriptions.insert(
                    subscription,
                    Subscription {
                        connection,
                        spec,
                        revision: 0,
                        state,
                        lost: false,
                    },
                );
                tracing::debug!(
                    connection,
                    subscription,
                    nodes = snapshot.nodes.len(),
                    "view subscribed"
                );
                self.answer(connection, Message::result(id, to_value(&snapshot)));
            }
            Command::Resync {
                connection,
                id,
                subscription,
            } => {
                let answer = match self.subscriptions.get(&subscription) {
                    Some(s) if s.connection == connection => {
                        tracing::debug!(
                            connection,
                            subscription,
                            revision = s.revision,
                            "view resynced"
                        );
                        Message::result(id, to_value(&s.state.snapshot(subscription, s.revision)))
                    }
                    _ => unknown(id, subscription),
                };
                self.answer(connection, answer);
            }
            Command::Unsubscribe {
                connection,
                id,
                subscription,
            } => {
                let answer = match self.subscriptions.get(&subscription) {
                    Some(s) if s.connection == connection => {
                        self.subscriptions.remove(&subscription);
                        Message::result(id, Value::Null)
                    }
                    _ => unknown(id, subscription),
                };
                self.answer(connection, answer);
            }
            Command::Closed { connection } => self.forget(connection),
            Command::Changed | Command::Stop => {}
        }
    }

    /// The head, read again if it changed, with its review diff.
    fn read_head(&mut self) -> crate::Result<()> {
        if self.head.is_some() {
            return Ok(());
        }
        let start = Instant::now();
        let reader = match &mut self.reader {
            Some(reader) => reader,
            None => self.reader.insert(self.store.reader()?),
        };
        let cache = &mut self.reviewed;
        // The head, the review and the contents as of one moment (review n° 1 of Phase 5, m11).
        let (graph, review) = reader.consistent(|reader| -> crate::Result<_> {
            let graph = view::load(reader)?;
            let review = match reader.latest(SnapshotKind::ReviewMark)? {
                // No diff before the first review (Q27).
                None => {
                    *cache = None;
                    None
                }
                Some(mark) => {
                    if cache.as_ref().map(|r| r.mark.seq) != Some(mark.seq) {
                        *cache = Some(Reviewed {
                            mark: ReviewMark::from(&mark),
                            graph: Graph::new(
                                reader.nodes_at(mark.seq)?,
                                reader.edges_at(mark.seq)?,
                            ),
                            contents: reader.reviewed_contents(mark.seq)?,
                        });
                    }
                    let reviewed = cache.as_ref().expect("read just above");
                    let diff = review::compare(
                        &reviewed.graph,
                        &graph,
                        &reviewed.contents,
                        &reader.file_contents()?,
                    );
                    let summary = view::summary(reviewed.mark, &reviewed.graph, &graph, &diff);
                    Some((diff, summary))
                }
            };
            Ok((graph, review))
        })?;
        tracing::debug!(
            elapsed_ms = start.elapsed().as_secs_f64() * 1000.0,
            "head read for the views"
        );
        self.head = Some(Head { graph, review });
        Ok(())
    }

    /// Every view again, and a delta to each one that changed.
    fn refresh(&mut self) {
        if self.subscriptions.is_empty() {
            return;
        }
        let start = Instant::now();
        if let Err(error) = self.read_head() {
            tracing::warn!(%error, "cannot read the graph: the views wait for the next change");
            return;
        }
        let head = self.head.as_ref().expect("read just above");
        let review = head.review(self.reviewed.as_ref());
        let mut deltas = Vec::new();
        for (id, subscription) in &mut self.subscriptions {
            let next = ViewState::compute(&head.graph, &subscription.spec, review.as_ref());
            let delta = subscription.state.delta(&next, *id, subscription.revision);
            if delta.is_empty() {
                continue;
            }
            subscription.revision += 1;
            subscription.state = next;
            deltas.push((*id, subscription.connection, delta));
        }
        let sent = deltas.len();
        for (subscription, connection, delta) in deltas {
            let message = Message::notification(methods::UPDATE, to_value(&delta));
            if !self.notify(connection, message)
                && let Some(view) = self.subscriptions.get_mut(&subscription)
            {
                view.lost = true;
            }
        }
        tracing::debug!(
            views = self.subscriptions.len(),
            sent,
            elapsed_ms = start.elapsed().as_secs_f64() * 1000.0,
            "views updated"
        );
    }

    /// An answer must arrive: a client too slow to take it is closed.
    fn answer(&mut self, connection: u64, message: Message) {
        let Some(client) = self.clients.get(&connection) else {
            return;
        };
        match client.outbox.try_send(message.frame()) {
            Ok(()) | Err(TrySendError::Closed(_)) => {}
            Err(TrySendError::Full(_)) => {
                tracing::warn!(connection, "client too slow to read its answers: closed");
                client.close.notify_one();
                self.forget(connection);
            }
        }
    }

    /// A delta may be lost: `false` then, and the client's writer will say when it drained.
    fn notify(&self, connection: u64, message: Message) -> bool {
        let Some(client) = self.clients.get(&connection) else {
            return true;
        };
        match client.outbox.try_send(message.frame()) {
            Ok(()) | Err(TrySendError::Closed(_)) => true,
            Err(TrySendError::Full(frame)) => {
                // Said before trying again: a writer that empties its queue from now on sees
                // it, and one that emptied it just before leaves room for the retry.
                client.congested.store(true, Ordering::SeqCst);
                match client.outbox.try_send(frame) {
                    Ok(()) | Err(TrySendError::Closed(_)) => true,
                    Err(TrySendError::Full(_)) => {
                        tracing::debug!(connection, "client queue full: delta dropped");
                        false
                    }
                }
            }
        }
    }

    /// The client has room again: each view that lost a delta gets an empty one, from its
    /// revision to the next. The client sees the gap and asks for `view/resync`, even when the
    /// graph does not change any more.
    fn signal_lost(&mut self, connection: u64) {
        let lost: Vec<u64> = self
            .subscriptions
            .iter()
            .filter(|(_, s)| s.connection == connection && s.lost)
            .map(|(id, _)| *id)
            .collect();
        for id in lost {
            let Some(view) = self.subscriptions.get(&id) else {
                continue;
            };
            let gap = view.state.delta(&view.state, id, view.revision);
            let message = Message::notification(methods::UPDATE, to_value(&gap));
            if self.notify(connection, message)
                && let Some(view) = self.subscriptions.get_mut(&id)
            {
                view.revision += 1;
                view.lost = false;
            }
        }
    }

    fn forget(&mut self, connection: u64) {
        self.clients.remove(&connection);
        self.subscriptions.retain(|_, s| s.connection != connection);
        if self.subscriptions.is_empty() {
            self.head = None;
            self.reviewed = None;
        }
    }
}

fn unknown(id: Id, subscription: u64) -> Message {
    let error = ErrorObject::new(
        errors::UNKNOWN_SUBSCRIPTION,
        format!("no subscription {subscription} on this connection"),
    );
    Message::error(Some(id), error)
}

fn to_value(value: &impl serde::Serialize) -> Value {
    // The protocol types are plain data: their JSON cannot fail.
    serde_json::to_value(value).unwrap_or(Value::Null)
}
