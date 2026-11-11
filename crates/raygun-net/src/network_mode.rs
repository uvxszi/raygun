//! Offline Mode and Data Saver: how much of the network this device lets Raygun use.
//!
//! Offline Mode stops every request that would leave the device. It's one switch held here, set
//! by the app from `Settings::network` ([`set_offline`]):
//! - every client built by [`crate::client_builder`] goes through [`GateLayer`], which refuses to
//!   connect anywhere but this device (loopback: the stream proxy, servers on this machine) while
//!   it's on, so a request nothing else checked still fails at once and sends nothing;
//! - places that send requests ask [`check`] first, so they fail before starting, connections a
//!   client keeps open included, with [`Offline`] ("Offline mode is on"); bodies being read when it
//!   turns on end ([`until_offline`]);
//! - background work waits for it to end ([`wait_online`]) and downloads wait instead of failing.
//!
//! A failure from Offline Mode is nobody's fault: it's never counted against a source, a stream
//! or a song ([`is_offline_error`], [`IssueKind::OfflineMode`](crate::IssueKind::OfflineMode)).
//!
//! Data Saver uses less data without stopping anything: [`data_saver`] says whether it applies
//! now (its switch, and whether the device is on mobile data when it's only for that); the app's
//! services read it with their own settings.

use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use futures_util::{Stream, StreamExt};
use tokio::sync::watch;

/// What a request refused by Offline Mode fails with, and the first line of every such error.
pub const OFFLINE_MESSAGE: &str = "Offline mode is on";

static OFFLINE: AtomicBool = AtomicBool::new(false);
static SAVING: AtomicBool = AtomicBool::new(false);

/// What the network may be used for right now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NetworkState {
    /// Offline Mode: nothing leaves the device.
    pub offline: bool,
    /// Data Saver applies now.
    pub data_saver: bool,
}

fn changes() -> &'static watch::Sender<NetworkState> {
    static CHANGES: OnceLock<watch::Sender<NetworkState>> = OnceLock::new();
    CHANGES.get_or_init(|| watch::channel(NetworkState::default()).0)
}

fn publish() {
    let state = current();
    changes().send_if_modified(|known| {
        let changed = *known != state;
        *known = state;
        changed
    });
}

/// The state now.
pub fn current() -> NetworkState {
    NetworkState {
        offline: offline(),
        data_saver: data_saver(),
    }
}

/// Whether Offline Mode is on.
pub fn offline() -> bool {
    OFFLINE.load(Ordering::SeqCst)
}

/// Turns Offline Mode on or off; true when it changed.
pub fn set_offline(on: bool) -> bool {
    let changed = OFFLINE.swap(on, Ordering::SeqCst) != on;
    if changed {
        publish();
    }
    changed
}

/// Whether Data Saver applies now (never while offline: nothing is sent then).
pub fn data_saver() -> bool {
    SAVING.load(Ordering::SeqCst) && !offline()
}

/// Says whether Data Saver applies now; true when it changed.
pub fn set_data_saver(on: bool) -> bool {
    let changed = SAVING.swap(on, Ordering::SeqCst) != on;
    if changed {
        publish();
    }
    changed
}

static FEWER_SOURCES: AtomicBool = AtomicBool::new(false);
static SMALLER_COVERS: AtomicBool = AtomicBool::new(false);

/// The parts of Data Saver followed below the app (the extension host's fan-outs, the cover
/// cache), set with its other settings.
pub fn set_saver_parts(fewer_sources: bool, smaller_covers: bool) {
    FEWER_SOURCES.store(fewer_sources, Ordering::SeqCst);
    SMALLER_COVERS.store(smaller_covers, Ordering::SeqCst);
}

/// Whether fewer sources are asked at once now (Data Saver).
pub fn fewer_sources() -> bool {
    data_saver() && FEWER_SOURCES.load(Ordering::SeqCst)
}

/// Whether covers are fetched in a smaller size where their address offers one (Data Saver).
pub fn smaller_covers() -> bool {
    data_saver() && SMALLER_COVERS.load(Ordering::SeqCst)
}

/// Wakes whoever waits on the state, which stays as it is: something they also look at changed
/// (Data Saver's parts).
pub fn nudge() {
    changes().send_modify(|_| {});
}

/// Follows the state: the receiver sees each change.
pub fn subscribe() -> watch::Receiver<NetworkState> {
    changes().subscribe()
}

/// Waits while Offline Mode is on; returns at once otherwise. Background work calls it before
/// each round, so it pauses with Offline Mode and goes on where it was once it's off.
pub async fn wait_online() {
    let mut changed = subscribe();
    while offline() {
        if changed.changed().await.is_err() {
            return;
        }
    }
}

/// Returns once Offline Mode is on (at once when it already is): a transfer selects on it to stop
/// when it turns on.
pub async fn went_offline() {
    let mut changed = subscribe();
    while !offline() {
        if changed.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

/// `stream`, the body of an answer from `url`, until Offline Mode turns on: one from elsewhere
/// than this device then ends there. Whoever reads it checks [`offline`] to tell a stop from a
/// finished body.
pub fn until_offline<S: Stream>(url: &str, stream: S) -> impl Stream<Item = S::Item> + use<S> {
    let here = reqwest::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(is_this_device))
        .unwrap_or(false);
    stream.take_until(Box::pin(async move {
        if here {
            std::future::pending::<()>().await;
        } else {
            went_offline().await;
        }
    }))
}

/// A request Offline Mode refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offline;

impl std::fmt::Display for Offline {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(OFFLINE_MESSAGE)
    }
}

impl std::error::Error for Offline {}

impl From<Offline> for std::io::Error {
    fn from(offline: Offline) -> Self {
        std::io::Error::new(std::io::ErrorKind::NotConnected, offline)
    }
}

impl From<Offline> for String {
    fn from(offline: Offline) -> Self {
        offline.to_string()
    }
}

/// Whether `host` (as in a URL; IPv6 with or without brackets) is this device: `localhost`, a
/// name under `.localhost`, or a loopback address. Everything else, the local network included,
/// is somewhere Offline Mode doesn't reach.
pub fn is_this_device(host: &str) -> bool {
    let host = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => ip.is_loopback(),
        Ok(std::net::IpAddr::V6(ip)) => {
            ip.is_loopback() || ip.to_ipv4_mapped().is_some_and(|ip| ip.is_loopback())
        }
        Err(_) => false,
    }
}

/// Whether a request to `url` may go now: always to this device (and to what isn't on a network:
/// `file:`, `content:`, `data:`), elsewhere unless Offline Mode is on. An address that doesn't
/// parse goes nowhere while it's on.
pub fn allows(url: &str) -> bool {
    if !offline() {
        return true;
    }
    let Ok(url) = reqwest::Url::parse(url) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https" | "ws" | "wss") {
        return true;
    }
    url.host_str().is_some_and(is_this_device)
}

/// [`allows`] as a result, for `?`.
pub fn check(url: &str) -> Result<(), Offline> {
    if allows(url) { Ok(()) } else { Err(Offline) }
}

/// `Err(Offline)` while Offline Mode is on: for traffic that isn't one address (the local network's
/// discovery, a whole lookup).
pub fn check_online() -> Result<(), Offline> {
    if offline() { Err(Offline) } else { Ok(()) }
}

/// Whether an error comes from Offline Mode: [`Offline`] among its causes, or its text says so.
pub fn is_offline_error(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(current) = cause {
        if current.downcast_ref::<Offline>().is_some() {
            return true;
        }
        if let Some(io) = current.downcast_ref::<std::io::Error>()
            && io
                .get_ref()
                .is_some_and(|inner| inner.downcast_ref::<Offline>().is_some())
        {
            return true;
        }
        cause = current.source();
    }
    mentions_offline(&error.to_string())
}

/// Whether an error's text comes from Offline Mode (its first line, or a cause after a colon).
pub fn mentions_offline(text: &str) -> bool {
    text.contains(OFFLINE_MESSAGE)
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The connector layer every client of [`crate::client_builder`] has: while Offline Mode is on, a
/// new connection is made only to this device, and any other fails with [`Offline`] before a
/// lookup or a byte is sent.
#[derive(Debug, Clone, Copy, Default)]
pub struct GateLayer;

impl<S> tower::Layer<S> for GateLayer {
    type Service = Gate<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Gate { inner }
    }
}

/// See [`GateLayer`].
#[derive(Debug, Clone)]
pub struct Gate<S> {
    inner: S,
}

impl<S, Request> tower::Service<Request> for Gate<S>
where
    S: tower::Service<Request, Error = BoxError>,
    S::Future: Send + 'static,
    S::Response: Send + 'static,
    Request: std::fmt::Debug,
{
    type Response = S::Response;
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<S::Response, BoxError>> + Send>>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        if offline() && !connects_to_this_device(&request) {
            return Box::pin(std::future::ready(Err(Box::new(Offline) as BoxError)));
        }
        Box::pin(self.inner.call(request))
    }
}

/// Whether a connection request goes to this device. reqwest hands its layers the destination
/// only in a type of its own, whose debug form is `Unnameable(<uri>)`; a form that doesn't read
/// counts as elsewhere (tests catch a reqwest that changes it).
fn connects_to_this_device(request: &impl std::fmt::Debug) -> bool {
    let shown = format!("{request:?}");
    let inner = match (shown.find('('), shown.rfind(')')) {
        (Some(start), Some(end)) if start < end => &shown[start + 1..end],
        _ => shown.as_str(),
    };
    let inner = inner.trim().trim_matches('"');
    reqwest::Url::parse(inner)
        .ok()
        .and_then(|url| url.host_str().map(is_this_device))
        .unwrap_or(false)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Arc, Mutex};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    /// Tests that switch Offline Mode take turns: it's one switch for the whole process.
    pub(crate) static SWITCH: Mutex<()> = Mutex::new(());

    /// Holds the switch for a test, back off when the test ends.
    pub(crate) struct Switched(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

    impl Drop for Switched {
        fn drop(&mut self) {
            set_offline(false);
            set_data_saver(false);
        }
    }

    pub(crate) fn switched() -> Switched {
        let guard = SWITCH
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        set_offline(false);
        Switched(guard)
    }

    /// A server that counts the connections it gets and answers each with `ok`.
    pub(crate) async fn counting_server() -> (std::net::SocketAddr, Arc<AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let seen = count.clone();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                seen.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    let mut buffer = [0u8; 4096];
                    let _ = socket.read(&mut buffer).await;
                    let _ = socket
                        .write_all(
                            b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok",
                        )
                        .await;
                });
            }
        });
        (address, count)
    }

    /// A client of [`crate::client_builder`] that reaches `address` by the name `name`, as if it
    /// were a site elsewhere.
    pub(crate) fn client_reaching(name: &str, address: std::net::SocketAddr) -> reqwest::Client {
        crate::client_builder()
            .resolve(name, address)
            .no_proxy()
            .build()
            .unwrap()
    }

    #[test]
    fn this_device_is_loopback_and_localhost() {
        for host in [
            "localhost",
            "LOCALHOST.",
            "app.localhost",
            "127.0.0.1",
            "127.4.5.6",
            "[::1]",
            "::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(is_this_device(host), "{host}");
        }
        for host in [
            "10.0.2.2",
            "192.168.1.20",
            "nas.local",
            "example.com",
            "localhost.example.com",
            "0.0.0.0",
            "8.8.8.8",
            "",
        ] {
            assert!(!is_this_device(host), "{host}");
        }
    }

    #[test]
    fn offline_mode_refuses_everything_but_this_device() {
        let _switched = switched();
        assert!(allows("https://example.com/a"));
        assert!(check_online().is_ok());
        set_offline(true);
        assert!(!allows("https://example.com/a"));
        assert!(!allows("http://192.168.1.20:8096/"));
        assert!(!allows("not a url"));
        assert!(allows("http://127.0.0.1:8810/mcp"));
        assert!(allows("http://localhost:3000/"));
        assert!(
            allows("content://media/external/video/1"),
            "not on a network"
        );
        assert!(allows("file:///storage/emulated/0/a.mp4"));
        assert_eq!(check("https://example.com/"), Err(Offline));
        assert_eq!(check_online(), Err(Offline));
        assert!(!data_saver(), "nothing to save while nothing is sent");
        set_data_saver(true);
        assert!(!data_saver());
        set_offline(false);
        assert!(data_saver());
        assert!(allows("https://example.com/a"));
    }

    #[test]
    fn offline_errors_are_told_apart() {
        let io: std::io::Error = Offline.into();
        assert!(is_offline_error(&io));
        assert!(is_offline_error(&Offline));
        let other = std::io::Error::other("connection reset");
        assert!(!is_offline_error(&other));
        assert!(mentions_offline("Deezer: Offline mode is on"));
        assert!(!mentions_offline("No internet connection"));
    }

    #[test]
    fn the_destination_is_read_from_the_connectors_request() {
        #[derive(Debug)]
        #[allow(dead_code)]
        struct Unnameable(http_uri::Uri);
        mod http_uri {
            /// Displays as `http::Uri` does in its debug form.
            pub struct Uri(pub &'static str);
            impl std::fmt::Debug for Uri {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    formatter.write_str(self.0)
                }
            }
        }
        assert!(connects_to_this_device(&Unnameable(http_uri::Uri(
            "http://127.0.0.1:4545/"
        ))));
        assert!(connects_to_this_device(&Unnameable(http_uri::Uri(
            "http://[::1]:80/"
        ))));
        assert!(!connects_to_this_device(&Unnameable(http_uri::Uri(
            "https://example.com/"
        ))));
        assert!(!connects_to_this_device(&"something else"));
    }

    /// The gate itself: while Offline Mode is on, a client of `client_builder` sends nothing to a
    /// site elsewhere (the server counts no connection) and fails with [`Offline`]; this device
    /// is still reached; once it's off, requests go again.
    #[tokio::test]
    async fn no_request_leaves_through_the_gate_while_offline() {
        let _switched = switched();
        let (address, connections) = counting_server().await;
        let client = client_reaching("site.example", address);
        let elsewhere = format!("http://site.example:{}/", address.port());

        set_offline(true);
        let error = client
            .get(&elsewhere)
            .send()
            .await
            .expect_err("offline mode refuses it");
        assert!(is_offline_error(&error), "{error:?}");
        assert_eq!(
            crate::issue::request_kind(&error),
            crate::IssueKind::OfflineMode
        );
        assert_eq!(connections.load(Ordering::SeqCst), 0, "nothing was sent");

        // This device is still reached: the proxy and servers on this machine keep working.
        let here = client
            .get(format!("http://127.0.0.1:{}/", address.port()))
            .send()
            .await
            .expect("loopback is this device");
        assert_eq!(here.text().await.unwrap(), "ok");
        assert_eq!(connections.load(Ordering::SeqCst), 1);

        set_offline(false);
        let back = client.get(&elsewhere).send().await.expect("online again");
        assert_eq!(back.text().await.unwrap(), "ok");
        assert_eq!(connections.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn bodies_end_when_offline_mode_turns_on() {
        let _switched = switched();
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<u32>();
        let mut stream = std::pin::pin!(until_offline(
            "https://example.com/song.mp3",
            tokio_stream_from(receiver)
        ));
        sender.send(1).unwrap();
        assert_eq!(stream.next().await, Some(1));
        set_offline(true);
        sender.send(2).unwrap();
        assert_eq!(stream.next().await, None, "the body stops there");
    }

    fn tokio_stream_from(
        mut receiver: tokio::sync::mpsc::UnboundedReceiver<u32>,
    ) -> impl Stream<Item = u32> {
        futures_util::stream::poll_fn(move |context| receiver.poll_recv(context))
    }

    #[tokio::test]
    async fn waits_end_with_offline_mode() {
        let _switched = switched();
        tokio::time::timeout(std::time::Duration::from_secs(1), wait_online())
            .await
            .expect("online: no wait");
        set_offline(true);
        let waiting = tokio::spawn(wait_online());
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        assert!(!waiting.is_finished(), "waits while offline");
        set_offline(false);
        tokio::time::timeout(std::time::Duration::from_secs(1), waiting)
            .await
            .expect("goes on once online")
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_millis(50), went_offline())
            .await
            .expect_err("not offline");
    }
}
