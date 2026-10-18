//! The language servers of a client's IDE, relayed to the pipeline (ADR 0014, 0019, 0022). The
//! client says `lspRelay` at `initialize`; the pipeline thread then sends it `lsp/outgoingCalls`,
//! `lsp/definitions` and `lsp/references`, and waits for the answers. Without such a client,
//! refinement waits.

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use timonier_core::Position;
use timonier_core::protocol::{
    DefinitionQuestion, DefinitionTarget, DefinitionsParams, LspAnswers, OutgoingCall,
    OutgoingCallsParams, ReferencesParams, errors, methods,
};
use timonier_core::rpc::{ErrorObject, Id, Message};
use timonier_index::lsp::{CallTarget, Definition, LanguageServer, LspError, Question};
use tokio::sync::mpsc::error::TrySendError;

use crate::subscriptions::Outbox;

/// How long one call hierarchy may take in the IDE.
const TIMEOUT: Duration = Duration::from_secs(10);

/// More for each position of a batch of definitions.
const PER_POSITION: Duration = Duration::from_millis(50);

/// How JSON-RPC peers say a request is given up (LSP's `$/cancelRequest`).
const CANCEL: &str = "$/cancelRequest";

/// While it waits for an answer, the pipeline looks this often whether other work came first.
const SLICE: Duration = Duration::from_millis(20);

type Answer = Result<Value, ErrorObject>;

#[derive(Default)]
struct Inner {
    /// The relaying connections, and where their messages go: the last one relays, the
    /// others take over when it leaves.
    clients: Vec<(u64, Outbox)>,
    pending: HashMap<i64, mpsc::Sender<Answer>>,
    next: i64,
    answers: LspAnswers,
    /// Changes of the client that relays.
    hands: u64,
}

impl Inner {
    fn active(&self) -> Option<&(u64, Outbox)> {
        self.clients.last()
    }
}

/// Shared by the connections, which attach and answer, and the pipeline, which asks.
#[derive(Clone, Default)]
pub(crate) struct Relay {
    inner: Arc<Mutex<Inner>>,
}

impl Relay {
    /// `connection` relays from now on, in place of any other.
    pub(crate) fn attach(&self, connection: u64, outbox: Outbox) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.clients.retain(|(c, _)| *c != connection);
            inner.clients.push((connection, outbox));
            // Questions to the previous client will not be answered.
            inner.pending.clear();
            inner.hands += 1;
        }
    }

    /// `connection` is gone: its questions fail at once, and the client before it relays.
    /// Returns whether another client relays in its place.
    pub(crate) fn detach(&self, connection: u64) -> bool {
        let Ok(mut inner) = self.inner.lock() else {
            return false;
        };
        let active = inner.active().is_some_and(|(c, _)| *c == connection);
        inner.clients.retain(|(c, _)| *c != connection);
        if active {
            inner.pending.clear();
            inner.hands += 1;
        }
        active && inner.active().is_some()
    }

    /// An answer from `connection`; `false` when nothing waited for it.
    pub(crate) fn answer(&self, connection: u64, id: &Id, outcome: Answer) -> bool {
        let Id::Number(id) = id else { return false };
        let Ok(mut inner) = self.inner.lock() else {
            return false;
        };
        if inner.active().is_none_or(|(c, _)| *c != connection) {
            return false;
        }
        match inner.pending.remove(id) {
            Some(waiting) => waiting.send(outcome).is_ok(),
            None => false,
        }
    }

    /// The questions answered so far, by method.
    pub(crate) fn answers(&self) -> LspAnswers {
        self.inner
            .lock()
            .map(|inner| inner.answers)
            .unwrap_or_default()
    }

    pub(crate) fn server(&self) -> RelayServer {
        RelayServer {
            relay: self.clone(),
            timeout: TIMEOUT,
            hands: 0,
        }
    }
}

/// The relay as the pipeline sees it.
pub(crate) struct RelayServer {
    relay: Relay,
    timeout: Duration,
    /// The changes of relaying client it has seen.
    hands: u64,
}

impl RelayServer {
    /// Asks the relaying client, without ever blocking on it (ADR 0019): the question is queued
    /// without waiting for room, and the wait for the answer gives way as soon as other work
    /// waits.
    fn ask(
        &self,
        method: &'static str,
        params: &impl serde::Serialize,
        timeout: Duration,
        yield_now: &dyn Fn() -> bool,
    ) -> Result<Value, LspError> {
        // No client relays, or the one asked left: the job waits for the next (ADR 0019).
        let unavailable = || LspError::Interrupted;
        let (send, receive) = mpsc::channel();
        let (connection, outbox, id) = {
            let mut inner = self.relay.inner.lock().map_err(|_| unavailable())?;
            let Some((connection, outbox)) = inner.active().cloned() else {
                return Err(unavailable());
            };
            inner.next += 1;
            let id = inner.next;
            inner.pending.insert(id, send);
            (connection, outbox, id)
        };
        let forget = || {
            if let Ok(mut inner) = self.relay.inner.lock() {
                inner.pending.remove(&id);
            }
        };
        let params = serde_json::to_value(params).map_err(|e| LspError::Failed(e.to_string()))?;
        let request = Message::request(Id::Number(id), method, params);
        match outbox.try_send(request.frame()) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                forget();
                return Err(LspError::Failed(
                    "the relaying client does not read its messages".into(),
                ));
            }
            Err(TrySendError::Closed(_)) => {
                forget();
                // Its connection writes no more, though it has not left yet (it may still
                // read): it relays no longer, or the job would come back to it at once, again
                // and again (review n° 7, I7-1).
                self.relay.detach(connection);
                return Err(unavailable());
            }
        }
        let deadline = Instant::now() + timeout;
        let answer = loop {
            match receive
                .recv_timeout(SLICE.min(deadline.saturating_duration_since(Instant::now())))
            {
                Ok(answer) => break Ok(answer),
                // Another client relays now, or none: the question is lost, not the job.
                Err(mpsc::RecvTimeoutError::Disconnected) => break Err(unavailable()),
                // Its connection stopped writing after the question was queued: no answer will
                // come (review n° 8, m8-1).
                Err(mpsc::RecvTimeoutError::Timeout) if outbox.is_closed() => {
                    self.relay.detach(connection);
                    break Err(unavailable());
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if yield_now() {
                        break Err(LspError::Interrupted);
                    }
                    if Instant::now() >= deadline {
                        break Err(LspError::Failed(format!(
                            "no answer within {} s",
                            timeout.as_secs()
                        )));
                    }
                }
            }
        };
        forget();
        if answer.is_err() {
            // Given up: the client may stop working on it (`$/cancelRequest`, as in LSP).
            let cancel = Message::notification(CANCEL, serde_json::json!({ "id": id }));
            let _ = outbox.try_send(cancel.frame());
        }
        let outcome = answer?;
        if let Ok(mut inner) = self.relay.inner.lock() {
            let answers = &mut inner.answers;
            match (&outcome, method) {
                (Err(error), _) if error.code == errors::CONTENT_MODIFIED => {}
                (Err(_), _) => answers.failed += 1,
                (Ok(_), methods::OUTGOING_CALLS) => answers.outgoing_calls += 1,
                (Ok(_), methods::DEFINITIONS) => answers.definitions += 1,
                (Ok(_), methods::REFERENCES) => answers.references += 1,
                (Ok(_), _) => {}
            }
        }
        outcome.map_err(|error| match error.code {
            errors::CONTENT_MODIFIED => LspError::Unsaved,
            _ => LspError::Failed(error.message),
        })
    }
}

fn parse<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, LspError> {
    serde_json::from_value(value).map_err(|e| LspError::Failed(e.to_string()))
}

impl LanguageServer for RelayServer {
    fn changed_hands(&mut self) -> bool {
        let Ok(inner) = self.relay.inner.lock() else {
            return false;
        };
        let changed = inner.hands != self.hands;
        self.hands = inner.hands;
        changed
    }

    fn ready(&self) -> bool {
        self.relay
            .inner
            .lock()
            .is_ok_and(|inner| inner.active().is_some())
    }

    fn outgoing_calls(&mut self, path: &str, at: Position) -> Result<Vec<CallTarget>, LspError> {
        self.outgoing_calls_or_yield(path, at, &|| false)
    }

    fn outgoing_calls_or_yield(
        &mut self,
        path: &str,
        at: Position,
        yield_now: &dyn Fn() -> bool,
    ) -> Result<Vec<CallTarget>, LspError> {
        let params = OutgoingCallsParams {
            path: path.to_owned(),
            position: at,
        };
        let answer = self.ask(methods::OUTGOING_CALLS, &params, self.timeout, yield_now)?;
        let calls: Vec<OutgoingCall> = parse(answer)?;
        Ok(calls
            .into_iter()
            .map(|call| CallTarget {
                path: call.path,
                at: call.at,
                sites: call.sites,
            })
            .collect())
    }

    fn definitions(
        &mut self,
        path: &str,
        questions: &[Question],
    ) -> Result<Vec<Vec<Definition>>, LspError> {
        self.definitions_or_yield(path, questions, &|| false)
    }

    /// One question for the whole file: the client asks its servers in parallel (ADR 0022).
    fn definitions_or_yield(
        &mut self,
        path: &str,
        questions: &[Question],
        yield_now: &dyn Fn() -> bool,
    ) -> Result<Vec<Vec<Definition>>, LspError> {
        let params = DefinitionsParams {
            path: path.to_owned(),
            questions: questions
                .iter()
                .map(|q| DefinitionQuestion {
                    position: q.at,
                    elsewhere: q.elsewhere,
                })
                .collect(),
        };
        let extra = PER_POSITION.saturating_mul(u32::try_from(questions.len()).unwrap_or(u32::MAX));
        let answer = self.ask(
            methods::DEFINITIONS,
            &params,
            self.timeout + extra,
            yield_now,
        )?;
        let found: Vec<Vec<DefinitionTarget>> = parse(answer)?;
        Ok(found
            .into_iter()
            .map(|targets| {
                targets
                    .into_iter()
                    .map(|t| Definition {
                        path: t.path,
                        at: t.at,
                    })
                    .collect()
            })
            .collect())
    }

    fn references(&mut self, path: &str, at: Position) -> Result<Vec<String>, LspError> {
        self.references_or_yield(path, at, &|| false)
    }

    fn references_or_yield(
        &mut self,
        path: &str,
        at: Position,
        yield_now: &dyn Fn() -> bool,
    ) -> Result<Vec<String>, LspError> {
        let params = ReferencesParams {
            path: path.to_owned(),
            position: at,
        };
        parse(self.ask(methods::REFERENCES, &params, self.timeout, yield_now)?)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use timonier_core::rpc::Decoder;
    use tokio::sync::mpsc as tokio_mpsc;

    use super::*;

    /// A client that answers each question with `answer`, on its own thread.
    fn client(relay: &Relay, answer: Answer) -> std::thread::JoinHandle<Vec<Value>> {
        let (outbox, mut queue) = tokio_mpsc::channel::<Vec<u8>>(8);
        relay.attach(7, outbox);
        let relay = relay.clone();
        std::thread::spawn(move || {
            let mut asked = Vec::new();
            let mut decoder = Decoder::default();
            while let Some(frame) = queue.blocking_recv() {
                decoder.push(&frame);
                while let Some(body) = decoder.next_body().unwrap() {
                    let Message::Request { id, params, .. } = Message::parse(&body).unwrap() else {
                        panic!("a request");
                    };
                    asked.push(params);
                    assert!(relay.answer(7, &id, answer.clone()));
                }
            }
            asked
        })
    }

    #[test]
    fn calls_go_to_the_relaying_client_and_back() {
        let relay = Relay::default();
        let mut server = relay.server();
        assert!(!server.ready());
        // Nobody relays: the job waits for a client, it is not dropped.
        assert_eq!(
            server.outgoing_calls("src/a.ts", Position::new(0, 0)),
            Err(LspError::Interrupted)
        );
        let answer = json!([{"path": "src/b.ts", "at": {"line": 2, "column": 16}, "sites": 3}, {"path": null, "at": {"line": 0, "column": 0}, "sites": 1}]);
        let thread = client(&relay, Ok(answer));
        assert!(server.ready());
        let calls = server
            .outgoing_calls("src/a.ts", Position::new(4, 9))
            .unwrap();
        assert_eq!(
            calls,
            [
                CallTarget {
                    path: Some("src/b.ts".into()),
                    at: Position::new(2, 16),
                    sites: 3
                },
                CallTarget {
                    path: None,
                    at: Position::new(0, 0),
                    sites: 1
                },
            ]
        );
        relay.detach(7);
        assert!(!server.ready());
        drop(server);
        drop(relay);
        let asked = thread.join().unwrap();
        assert_eq!(
            asked,
            [json!({"path": "src/a.ts", "position": {"line": 4, "column": 9}})]
        );
    }

    #[test]
    fn an_error_or_a_silence_is_a_failure() {
        let relay = Relay::default();
        let _thread = client(
            &relay,
            Err(ErrorObject::new(-32803, "no call hierarchy here")),
        );
        let mut server = relay.server();
        match server.outgoing_calls("src/a.ts", Position::new(0, 0)) {
            Err(LspError::Failed(message)) => assert!(message.contains("no call hierarchy")),
            other => panic!("{other:?}"),
        }
        // Counted as failed, not as answered.
        assert_eq!(
            relay.answers(),
            LspAnswers {
                failed: 1,
                ..LspAnswers::default()
            }
        );
        // A client that never answers.
        let (outbox, _queue) = tokio_mpsc::channel::<Vec<u8>>(8);
        relay.attach(8, outbox);
        let mut server = RelayServer {
            relay: relay.clone(),
            timeout: Duration::from_millis(50),
            hands: 0,
        };
        assert!(matches!(
            server.outgoing_calls("src/a.ts", Position::new(0, 0)),
            Err(LspError::Failed(_))
        ));
        // Another connection cannot answer for it.
        assert!(!relay.answer(7, &Id::Number(1), Ok(Value::Null)));
    }

    /// Review n° 6, m6-3: the relaying client leaves, or another one takes over, while a question
    /// waits. The question is lost, not the job: it waits again, for the client that relays.
    #[test]
    fn a_question_lost_with_its_client_gives_the_job_back() {
        let relay = Relay::default();
        let (outbox, _queue) = tokio_mpsc::channel::<Vec<u8>>(8);
        relay.attach(7, outbox);
        let leaving = relay.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let (outbox, _queue) = tokio_mpsc::channel::<Vec<u8>>(8);
            // Another client relays: the question to the first one will not be answered.
            leaving.attach(8, outbox);
        });
        let mut server = relay.server();
        assert_eq!(
            server.outgoing_calls("src/a.ts", Position::new(0, 0)),
            Err(LspError::Interrupted)
        );
        thread.join().unwrap();
    }

    /// Review n° 7, I7-1: a client whose connection writes no more, though it has not left (it
    /// still reads), relays no longer: the next question goes to the client before it, or waits
    /// for one, instead of coming back to it again and again.
    #[test]
    fn a_client_that_writes_no_more_relays_no_longer() {
        let relay = Relay::default();
        let (outbox, mut older) = tokio_mpsc::channel::<Vec<u8>>(8);
        relay.attach(7, outbox);
        let (outbox, closed) = tokio_mpsc::channel::<Vec<u8>>(8);
        relay.attach(8, outbox);
        drop(closed);
        let mut server = relay.server();
        assert_eq!(
            server.outgoing_calls("src/a.ts", Position::new(0, 0)),
            Err(LspError::Interrupted)
        );
        // The older client relays again, and is asked.
        assert!(server.ready());
        let mut quick = RelayServer {
            relay: relay.clone(),
            timeout: Duration::from_millis(50),
            hands: 0,
        };
        let _ = quick.outgoing_calls("src/a.ts", Position::new(0, 0));
        assert!(older.try_recv().is_ok(), "the older client was not asked");
        relay.detach(7);
        assert!(!server.ready());
    }

    /// Review n° 8, m8-1: the connection stops writing once the question is queued: no answer
    /// will come. The client relays no longer, and the job comes back at once, not after the
    /// timeout as a failure.
    #[test]
    fn a_client_that_stops_writing_after_the_question_gives_the_job_back() {
        let relay = Relay::default();
        let (outbox, mut queue) = tokio_mpsc::channel::<Vec<u8>>(8);
        relay.attach(7, outbox);
        // Its writer takes the question, then ends: the queue closes.
        let thread = std::thread::spawn(move || {
            let _question = queue.blocking_recv();
            std::thread::sleep(Duration::from_millis(50));
            drop(queue);
        });
        let mut server = relay.server();
        assert!(server.changed_hands(), "a client relays");
        assert!(!server.changed_hands());
        let start = std::time::Instant::now();
        assert_eq!(
            server.outgoing_calls("src/a.ts", Position::new(0, 0)),
            Err(LspError::Interrupted)
        );
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "{:?}",
            start.elapsed()
        );
        assert!(!server.ready());
        assert!(server.changed_hands(), "it relays no longer");
        thread.join().unwrap();
    }

    /// Review n° 8, m8-3 (P1): a batch of definitions may take 50 ms more per position than
    /// one question: the client asks its servers for each.
    #[test]
    fn a_batch_of_definitions_has_more_time() {
        let relay = Relay::default();
        let (outbox, mut queue) = tokio_mpsc::channel::<Vec<u8>>(8);
        relay.attach(7, outbox);
        let answering = relay.clone();
        let thread = std::thread::spawn(move || {
            let mut decoder = Decoder::default();
            let frame = queue.blocking_recv().unwrap();
            decoder.push(&frame);
            let body = decoder.next_body().unwrap().unwrap();
            let Message::Request { id, .. } = Message::parse(&body).unwrap() else {
                panic!("a request");
            };
            // Past the timeout of one question, within that of twenty positions.
            std::thread::sleep(Duration::from_millis(400));
            let answer = Value::Array(vec![Value::Array(Vec::new()); 20]);
            assert!(answering.answer(7, &id, Ok(answer)));
        });
        let mut server = RelayServer {
            relay: relay.clone(),
            timeout: Duration::from_millis(100),
            hands: 0,
        };
        let questions = vec![
            Question {
                at: Position::new(0, 0),
                elsewhere: false,
            };
            20
        ];
        let found = server.definitions("src/a.rs", &questions).unwrap();
        assert_eq!(found.len(), 20);
        thread.join().unwrap();
    }

    /// Review n° 7, m7-1 (N12): the relaying client leaves while a question waits, and none
    /// relays in its place. The question gives its job back at once, not after the timeout.
    #[test]
    fn a_question_whose_client_leaves_gives_the_job_back_at_once() {
        let relay = Relay::default();
        let (outbox, _queue) = tokio_mpsc::channel::<Vec<u8>>(8);
        relay.attach(7, outbox);
        let leaving = relay.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            assert!(!leaving.detach(7), "nobody relays in its place");
        });
        let mut server = relay.server();
        let start = std::time::Instant::now();
        assert_eq!(
            server.outgoing_calls("src/a.ts", Position::new(0, 0)),
            Err(LspError::Interrupted)
        );
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "{:?}",
            start.elapsed()
        );
        thread.join().unwrap();
    }

    /// ADR 0022: a client whose documents are not what is on disk says so, as LSP does; the
    /// pipeline then holds the question instead of dropping it.
    #[test]
    fn unsaved_documents_are_not_a_failure() {
        let relay = Relay::default();
        let _thread = client(
            &relay,
            Err(ErrorObject::new(
                errors::CONTENT_MODIFIED,
                "unsaved changes in src/a.ts",
            )),
        );
        let mut server = relay.server();
        assert_eq!(
            server.outgoing_calls("src/a.ts", Position::new(0, 0)),
            Err(LspError::Unsaved)
        );
        assert_eq!(
            server.references("src/a.ts", Position::new(0, 0)),
            Err(LspError::Unsaved)
        );
        assert_eq!(relay.answers(), LspAnswers::default(), "not answers");
        // The only relaying client leaves: nobody relays in its place.
        assert!(!relay.detach(7));
    }

    #[test]
    fn a_client_that_does_not_read_never_holds_the_pipeline() {
        let relay = Relay::default();
        // A queue of one, full already: the question cannot even be sent.
        let (outbox, _queue) = tokio_mpsc::channel::<Vec<u8>>(1);
        outbox.try_send(Vec::new()).unwrap();
        relay.attach(9, outbox);
        let mut server = relay.server();
        let start = std::time::Instant::now();
        match server.outgoing_calls("src/a.ts", Position::new(0, 0)) {
            Err(LspError::Failed(message)) => assert!(message.contains("does not read")),
            other => panic!("{other:?}"),
        }
        assert!(start.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn a_slow_answer_gives_way_to_other_work() {
        let relay = Relay::default();
        let (outbox, mut queue) = tokio_mpsc::channel::<Vec<u8>>(8);
        relay.attach(9, outbox);
        let mut server = relay.server();
        let start = std::time::Instant::now();
        let asked = std::cell::Cell::new(0);
        let result = server.outgoing_calls_or_yield("src/a.ts", Position::new(0, 0), &|| {
            asked.set(asked.get() + 1);
            asked.get() > 2
        });
        assert!(matches!(result, Err(LspError::Interrupted)));
        // Well before the 10 s of a silent client.
        assert!(start.elapsed() < Duration::from_secs(1));
        // The question, then its cancellation.
        let mut decoder = Decoder::default();
        let mut methods_sent = Vec::new();
        while let Ok(frame) = queue.try_recv() {
            decoder.push(&frame);
            while let Some(body) = decoder.next_body().unwrap() {
                match Message::parse(&body).unwrap() {
                    Message::Request { method, .. } | Message::Notification { method, .. } => {
                        methods_sent.push(method);
                    }
                    other => panic!("{other:?}"),
                }
            }
        }
        assert_eq!(methods_sent, [methods::OUTGOING_CALLS, CANCEL]);
    }

    #[test]
    fn the_previous_relay_takes_over_when_the_last_one_leaves() {
        let relay = Relay::default();
        let (first, _q1) = tokio_mpsc::channel::<Vec<u8>>(8);
        let (second, _q2) = tokio_mpsc::channel::<Vec<u8>>(8);
        relay.attach(1, first);
        relay.attach(2, second);
        let server = relay.server();
        relay.detach(2);
        assert!(server.ready(), "the first relay should take over");
        relay.detach(1);
        assert!(!server.ready());
    }

    #[test]
    fn definitions_and_references_go_to_the_relaying_client_and_back() {
        let relay = Relay::default();
        let (outbox, mut queue) = tokio_mpsc::channel::<Vec<u8>>(8);
        relay.attach(7, outbox);
        let answering = relay.clone();
        let thread = std::thread::spawn(move || {
            let mut asked = Vec::new();
            let mut decoder = Decoder::default();
            while let Some(frame) = queue.blocking_recv() {
                decoder.push(&frame);
                while let Some(body) = decoder.next_body().unwrap() {
                    let Message::Request { id, method, params } = Message::parse(&body).unwrap()
                    else {
                        panic!("a request");
                    };
                    let answer = match method.as_str() {
                        methods::DEFINITIONS => json!([
                            [{"path": "src/b.rs", "at": {"line": 1, "column": 4}}],
                            []
                        ]),
                        methods::REFERENCES => json!(["src/c.rs", "src/d.rs"]),
                        other => panic!("{other}"),
                    };
                    asked.push((method, params));
                    assert!(answering.answer(7, &id, Ok(answer)));
                }
            }
            asked
        });
        let mut server = relay.server();
        let question = |line, column, elsewhere| Question {
            at: Position::new(line, column),
            elsewhere,
        };
        let found = server
            .definitions("src/a.rs", &[question(2, 3, true), question(5, 0, false)])
            .unwrap();
        assert_eq!(
            found,
            [
                vec![Definition {
                    path: Some("src/b.rs".into()),
                    at: Position::new(1, 4)
                }],
                Vec::new()
            ]
        );
        let users = server.references("src/a.rs", Position::new(2, 3)).unwrap();
        assert_eq!(users, ["src/c.rs", "src/d.rs"]);
        assert_eq!(
            relay.answers(),
            LspAnswers {
                outgoing_calls: 0,
                definitions: 1,
                references: 1,
                failed: 0
            }
        );
        relay.detach(7);
        drop(server);
        drop(relay);
        let asked = thread.join().unwrap();
        assert_eq!(
            asked[0].1,
            json!({"path": "src/a.rs", "questions": [
                {"position": {"line": 2, "column": 3}, "elsewhere": true},
                {"position": {"line": 5, "column": 0}, "elsewhere": false}
            ]})
        );
        assert_eq!(
            asked[1].1,
            json!({"path": "src/a.rs", "position": {"line": 2, "column": 3}})
        );
    }
}
