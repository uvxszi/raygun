//! Calls someone waited on that ran past their time. A search or a lookup gives a source
//! [`crate::INTERACTIVE_TIMEOUT`], and then says the source is still working rather than that it
//! failed: the call goes on, up to [`crate::BACKGROUND_TIMEOUT`], and its answer is kept a while.
//! The same ask again gets it at once, or waits on the call still going. A slow source (a
//! converted plugin scraping a whole page on a slow phone) is then of use the second time rather
//! than never.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::watch;
use tokio::time::Instant;

use crate::CallError;

/// How long a late answer is kept for the same ask.
const KEEP: Duration = Duration::from_secs(120);
/// Late calls kept at once; past it, a call that runs out of time stops, as before.
const MOST: usize = 64;

type Answer = Result<Value, CallError>;

/// A call that ran late: still going, or answered.
enum Late {
    Running(watch::Receiver<Option<Answer>>),
    Done { at: Instant, answer: Answer },
}

/// What an ask finds of an earlier one that ran late.
pub(crate) enum Found {
    /// Its answer.
    Answer(Answer),
    /// The call, still going.
    Running(watch::Receiver<Option<Answer>>),
}

/// Why a call that ran late isn't kept going.
pub(crate) enum NotKept {
    /// The same ask goes on already.
    Already,
    /// Too many late calls go on.
    Full,
}

/// The late calls of one host.
#[derive(Default)]
pub(crate) struct LateCalls(Mutex<HashMap<String, Late>>);

/// What identifies an ask: the provider, the method and the arguments.
pub(crate) fn key(provider: &str, method: &str, args: &Value) -> String {
    format!("{provider}\u{1f}{method}\u{1f}{args}")
}

impl LateCalls {
    fn calls(&self) -> std::sync::MutexGuard<'_, HashMap<String, Late>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// What's known of an earlier ask like this one: an answer is handed over once.
    pub(crate) fn find(&self, key: &str) -> Option<Found> {
        let mut calls = self.calls();
        let now = Instant::now();
        calls.retain(|_, late| !matches!(late, Late::Done { at, .. } if now - *at > KEEP));
        match calls.get(key)? {
            Late::Running(receiver) => Some(Found::Running(receiver.clone())),
            Late::Done { .. } => match calls.remove(key) {
                Some(Late::Done { answer, .. }) => Some(Found::Answer(answer)),
                _ => None,
            },
        }
    }

    /// Notes a call going on past its time; the sender gets its answer. The caller stops the
    /// call when it isn't kept.
    pub(crate) fn going_on(&self, key: &str) -> Result<watch::Sender<Option<Answer>>, NotKept> {
        let mut calls = self.calls();
        if calls.contains_key(key) {
            return Err(NotKept::Already);
        }
        if calls.len() >= MOST {
            return Err(NotKept::Full);
        }
        let (sender, receiver) = watch::channel(None);
        calls.insert(key.to_owned(), Late::Running(receiver));
        Ok(sender)
    }

    /// Keeps the answer of a call that went on.
    pub(crate) fn answered(&self, key: &str, answer: Answer) {
        self.calls().insert(
            key.to_owned(),
            Late::Done {
                at: Instant::now(),
                answer,
            },
        );
    }

    /// Takes an answer a waiter saw come in, so the next ask asks anew.
    pub(crate) fn forget(&self, key: &str) {
        self.calls().remove(key);
    }
}

/// A task that's stopped when dropped, unless kept.
pub(crate) struct StopOnDrop<T>(pub Option<tokio::task::JoinHandle<T>>);

impl<T> StopOnDrop<T> {
    /// The task, to let it go on.
    pub(crate) fn keep(mut self) -> Option<tokio::task::JoinHandle<T>> {
        self.0.take()
    }
}

impl<T> Drop for StopOnDrop<T> {
    fn drop(&mut self) {
        if let Some(task) = self.0.take() {
            task.abort();
        }
    }
}
