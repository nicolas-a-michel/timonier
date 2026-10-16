//! The single writer: a dedicated thread owns the only read-write connection (ADR 0011).

use std::any::Any;
use std::panic::{self, AssertUnwindSafe};
use std::sync::mpsc;
use std::thread;

use rusqlite::Connection;

use crate::{Error, Result};

type Job = Box<dyn FnOnce(&mut Connection) + Send>;

/// Bounded: callers wait when the writer is behind (back-pressure).
const QUEUE: usize = 64;

pub(crate) struct Writer {
    sender: Option<mpsc::SyncSender<Job>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Writer {
    pub(crate) fn spawn(mut connection: Connection) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Job>(QUEUE);
        let thread = thread::Builder::new()
            .name("timonier-store-writer".into())
            .spawn(move || {
                for job in receiver {
                    job(&mut connection);
                }
            })?;
        Ok(Self {
            sender: Some(sender),
            thread: Some(thread),
        })
    }

    /// Runs `job` on the writer thread and waits for its result. A panic inside the job is
    /// caught: its transaction is rolled back (dropped) and the writer keeps serving.
    pub(crate) fn run<R, F>(&self, job: F) -> Result<R>
    where
        R: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<R> + Send + 'static,
    {
        let (reply, answer) = mpsc::sync_channel(1);
        let job: Job = Box::new(move |connection| {
            let outcome = panic::catch_unwind(AssertUnwindSafe(|| job(connection))).unwrap_or_else(
                |payload| Err(Error::WritePanicked(panic_message(payload.as_ref()))),
            );
            // The caller may have stopped waiting; nothing else to do then.
            let _ = reply.send(outcome);
        });
        let sender = self.sender.as_ref().ok_or(Error::WriterStopped)?;
        sender.send(job).map_err(|_| Error::WriterStopped)?;
        answer.recv().map_err(|_| Error::WriterStopped)?
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        // Closing the channel ends the thread once the queued jobs are done.
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn writer() -> Writer {
        Writer::spawn(Connection::open_in_memory().unwrap()).unwrap()
    }

    #[test]
    fn runs_jobs_in_order_and_returns_their_results() {
        let writer = writer();
        writer
            .run(|c| Ok(c.execute_batch("CREATE TABLE t (v INTEGER)")?))
            .unwrap();
        for v in 0..10 {
            writer
                .run(move |c| Ok(c.execute("INSERT INTO t VALUES (?1)", [v])?))
                .unwrap();
        }
        let sum: i64 = writer
            .run(|c| Ok(c.query_row("SELECT sum(v) FROM t", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(sum, 45);
    }

    #[test]
    fn a_panicking_job_is_rolled_back_and_the_writer_survives() {
        let writer = writer();
        writer
            .run(|c| Ok(c.execute_batch("CREATE TABLE t (v INTEGER)")?))
            .unwrap();
        let result: Result<()> = writer.run(|c| {
            let tx = c.transaction()?;
            tx.execute("INSERT INTO t VALUES (1)", [])?;
            panic!("boom");
        });
        assert!(matches!(result, Err(Error::WritePanicked(message)) if message == "boom"));
        let count: i64 = writer
            .run(|c| Ok(c.query_row("SELECT count(*) FROM t", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn jobs_can_come_from_several_threads() {
        let writer = std::sync::Arc::new(writer());
        writer
            .run(|c| Ok(c.execute_batch("CREATE TABLE t (v INTEGER)")?))
            .unwrap();
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let writer = writer.clone();
                thread::spawn(move || {
                    writer.run(move |c| Ok(c.execute("INSERT INTO t VALUES (?1)", [i])?))
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap().unwrap();
        }
        let count: i64 = writer
            .run(|c| Ok(c.query_row("SELECT count(*) FROM t", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(count, 8);
    }
}
