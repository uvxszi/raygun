//! What Raygun's browser shows of the downloads it started: a bar under the page on Android
//! (`WebsiteActivity`), the site window's Downloads menu on desktop.
//!
//! Each request the browser hands over carries a ticket of its own. Every change to a download
//! with a ticket goes back to the browser, a few times a second at most: on Android in a
//! package-only broadcast its process `:web` listens to while the browser is on screen (data
//! only, nothing reaches the pages), on desktop to the window the ticket names. The browser asks
//! back with [`BrowserAsk`]: it's on screen again (and wants its downloads' state), it left, or
//! a download should stop or be tried again.

use std::collections::BTreeSet;
use std::sync::Mutex;

use raygun_model::WebsiteDownloadState;
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use super::Stored;

/// One download as the browser shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserDownload {
    /// The browser's own name for it, from its request.
    pub ticket: String,
    pub id: String,
    pub name: String,
    pub state: WebsiteDownloadState,
    pub done_bytes: f64,
    pub total_bytes: Option<f64>,
    /// Where it goes, as shown ("Download/Raygun", a picked folder's name).
    pub folder: String,
    pub error: Option<String>,
    /// The finished file, to open it: a content URI on Android, a path on desktop.
    pub saved: Option<String>,
    pub mime: Option<String>,
}

impl BrowserDownload {
    /// A download the browser asked about that isn't kept any more (or was never taken): its
    /// row goes.
    pub fn gone(ticket: &str) -> Self {
        Self {
            ticket: ticket.to_owned(),
            id: String::new(),
            name: String::new(),
            state: WebsiteDownloadState::Cancelled,
            done_bytes: 0.0,
            total_bytes: None,
            folder: String::new(),
            error: None,
            saved: None,
            mime: None,
        }
    }

    /// A request that couldn't start, said as a failed download.
    pub fn refused(ticket: &str, name: &str, reason: &str) -> Self {
        Self {
            name: name.to_owned(),
            state: WebsiteDownloadState::Failed,
            error: Some(reason.to_owned()),
            ..Self::gone(ticket)
        }
    }
}

/// How a kept download looks to the browser; `None` for one the browser didn't start.
pub(super) fn view(stored: &Stored) -> Option<BrowserDownload> {
    let ticket = stored.ticket.clone()?;
    let item = &stored.item;
    Some(BrowserDownload {
        ticket,
        id: item.id.clone(),
        name: item.name.clone(),
        state: item.state,
        done_bytes: item.done_bytes,
        total_bytes: item.total_bytes,
        folder: item.folder.clone(),
        error: item.error.clone(),
        saved: stored.saved.clone(),
        mime: item.mime.clone(),
    })
}

/// What the browser asks about its downloads (`{"downloadAsk": …}` in its reports).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", tag = "action")]
pub enum BrowserAsk {
    /// The browser is on screen: these downloads' state now (it may have missed changes while
    /// away), and every change from here on.
    Watch {
        tickets: Vec<String>,
    },
    /// The browser left the screen: nothing until it's back.
    Unwatch,
    Cancel {
        ticket: String,
    },
    Retry {
        ticket: String,
    },
}

/// Downloads whose change the browser hasn't heard yet.
#[derive(Default)]
pub(super) struct Feed {
    /// By download id.
    changed: Mutex<BTreeSet<String>>,
    /// Said as they are: removed downloads, refused requests, unknown tickets.
    said: Mutex<Vec<BrowserDownload>>,
    /// Android's browsers on screen. Their process may be frozen while they aren't, so nothing
    /// is sent then; they ask again when they're back.
    watching: Mutex<usize>,
    pub(super) wake: Notify,
}

impl Feed {
    pub(super) fn watch(&self) {
        *self.watching.lock().expect("feed poisoned") += 1;
    }

    /// One more leaving than came (a browser killed on screen) stays at none.
    pub(super) fn unwatch(&self) {
        let mut watching = self.watching.lock().expect("feed poisoned");
        *watching = watching.saturating_sub(1);
    }

    /// Whether anything sent now is heard: Android's browser only while it's on screen; desktop
    /// windows always (their menus).
    pub(super) fn heard(&self) -> bool {
        !cfg!(target_os = "android") || *self.watching.lock().expect("feed poisoned") > 0
    }

    pub(super) fn touch(&self, id: &str) {
        if id.is_empty() {
            return;
        }
        self.changed
            .lock()
            .expect("feed poisoned")
            .insert(id.to_owned());
        self.wake.notify_one();
    }

    pub(super) fn say(&self, download: BrowserDownload) {
        self.said.lock().expect("feed poisoned").push(download);
        self.wake.notify_one();
    }

    /// What's waiting: the changed ids, and what's said as it is.
    pub(super) fn take(&self) -> (BTreeSet<String>, Vec<BrowserDownload>) {
        (
            std::mem::take(&mut *self.changed.lock().expect("feed poisoned")),
            std::mem::take(&mut *self.said.lock().expect("feed poisoned")),
        )
    }
}

/// What's sent for a turn of the feed: the views of the changed downloads that the browser
/// started, then what's said as it is (a ticket appears once, its latest word).
pub(super) fn turn(
    changed: &BTreeSet<String>,
    said: Vec<BrowserDownload>,
    stored: &[&Stored],
) -> Vec<BrowserDownload> {
    let mut views: Vec<BrowserDownload> = stored
        .iter()
        .filter(|stored| changed.contains(&stored.item.id))
        .filter_map(|stored| view(stored))
        .collect();
    for download in said {
        views.retain(|view| view.ticket != download.ticket);
        views.push(download);
    }
    views
}
