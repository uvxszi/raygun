use std::path::Path;
use std::sync::{Arc, Mutex as StdMutex};

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;

use super::fetch::{self, Failure, Origin, Validator};
use super::names::{disposition_name, file_name, unique_name};
use super::*;

const FILE_LEN: usize = 100_000;

fn file() -> Vec<u8> {
    (0..FILE_LEN).map(|index| (index % 251) as u8).collect()
}

/// What the test server saw: path, Range, Cookie, Referer and User-Agent.
#[derive(Clone, Debug, Default)]
struct Asked {
    path: String,
    range: Option<String>,
    cookie: Option<String>,
    referer: Option<String>,
    agent: Option<String>,
}

#[derive(Clone, Default)]
struct Seen(Arc<StdMutex<Vec<Asked>>>);

impl Seen {
    fn all(&self) -> Vec<Asked> {
        self.0.lock().unwrap().clone()
    }
}

/// `/ranged.bin` takes ranges; `/whole.bin` always sends everything; `/gone` is a 404.
async fn serve(State(seen): State<Seen>, uri: Uri, headers: HeaderMap) -> Response {
    let get = |name: header::HeaderName| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    let range = get(header::RANGE);
    seen.0.lock().unwrap().push(Asked {
        path: uri.path().to_owned(),
        range: range.clone(),
        cookie: get(header::COOKIE),
        referer: get(header::REFERER),
        agent: get(header::USER_AGENT),
    });
    let body = file();
    match uri.path() {
        "/ranged.bin" => {
            let start = range.and_then(|range| {
                range
                    .strip_prefix("bytes=")?
                    .strip_suffix('-')?
                    .parse::<usize>()
                    .ok()
            });
            match start {
                Some(start) if start >= body.len() => (
                    StatusCode::RANGE_NOT_SATISFIABLE,
                    [(header::CONTENT_RANGE, format!("bytes */{}", body.len()))],
                )
                    .into_response(),
                Some(start) => (
                    StatusCode::PARTIAL_CONTENT,
                    [
                        (
                            header::CONTENT_RANGE,
                            format!("bytes {start}-{}/{}", body.len() - 1, body.len()),
                        ),
                        (header::ACCEPT_RANGES, "bytes".to_owned()),
                        (header::ETAG, "\"v1\"".to_owned()),
                    ],
                    body[start..].to_vec(),
                )
                    .into_response(),
                None => (
                    [
                        (header::ACCEPT_RANGES, "bytes"),
                        (header::ETAG, "\"v1\""),
                        (header::CONTENT_TYPE, "application/pdf"),
                    ],
                    body,
                )
                    .into_response(),
            }
        }
        "/whole.bin" => body.into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn server() -> (String, Seen) {
    let seen = Seen::default();
    let app = Router::new().fallback(serve).with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), seen)
}

fn origin(url: String) -> Origin {
    Origin {
        url,
        user_agent: Some("PageAgent/1.0".into()),
        referer: Some("https://example.org/page".into()),
        cookies: Some("session=abc".into()),
    }
}

async fn run(
    origin: &Origin,
    partial: &Path,
    validator: &mut Validator,
) -> Result<fetch::Fetched, Failure> {
    let client = raygun_net::http_client().unwrap();
    let (_keep, mut cancel) = watch::channel(false);
    let mut progress = |_: u64, _: Option<u64>| {};
    fetch::fetch(
        &client,
        origin,
        partial,
        validator,
        &mut cancel,
        &mut progress,
    )
    .await
}

#[tokio::test]
async fn a_download_carries_the_pages_cookies_agent_and_referer() {
    let (base, seen) = server().await;
    let temp = TempDir::new().unwrap();
    let partial = temp.path().join("a.part");
    let mut validator = Validator::default();
    let fetched = run(
        &origin(format!("{base}/ranged.bin")),
        &partial,
        &mut validator,
    )
    .await
    .unwrap();
    assert_eq!(fetched.bytes, FILE_LEN as u64);
    assert_eq!(fetched.mime.as_deref(), Some("application/pdf"));
    assert_eq!(std::fs::read(&partial).unwrap(), file());
    assert!(validator.ranges);
    assert_eq!(validator.etag.as_deref(), Some("\"v1\""));
    let asked = &seen.all()[0];
    assert_eq!(asked.cookie.as_deref(), Some("session=abc"));
    assert_eq!(asked.referer.as_deref(), Some("https://example.org/page"));
    assert_eq!(asked.agent.as_deref(), Some("PageAgent/1.0"));
    assert_eq!(asked.path, "/ranged.bin");
    assert_eq!(asked.range, None);
}

#[tokio::test]
async fn a_stopped_download_resumes_where_it_was() {
    let (base, seen) = server().await;
    let temp = TempDir::new().unwrap();
    let partial = temp.path().join("a.part");
    std::fs::write(&partial, &file()[..40_000]).unwrap();
    let mut validator = Validator {
        etag: Some("\"v1\"".into()),
        ranges: true,
        ..Validator::default()
    };
    let fetched = run(
        &origin(format!("{base}/ranged.bin")),
        &partial,
        &mut validator,
    )
    .await
    .unwrap();
    assert_eq!(fetched.bytes, FILE_LEN as u64);
    assert_eq!(std::fs::read(&partial).unwrap(), file());
    assert_eq!(seen.all()[0].range.as_deref(), Some("bytes=40000-"));
    assert_eq!(validator.total, Some(FILE_LEN as u64));
}

#[tokio::test]
async fn a_complete_partial_is_finished_without_fetching_again() {
    let (base, _seen) = server().await;
    let temp = TempDir::new().unwrap();
    let partial = temp.path().join("a.part");
    std::fs::write(&partial, file()).unwrap();
    let mut validator = Validator {
        ranges: true,
        total: Some(FILE_LEN as u64),
        ..Validator::default()
    };
    let fetched = run(
        &origin(format!("{base}/ranged.bin")),
        &partial,
        &mut validator,
    )
    .await
    .unwrap();
    assert_eq!(fetched.bytes, FILE_LEN as u64);
    assert_eq!(std::fs::read(&partial).unwrap(), file());
}

#[tokio::test]
async fn a_server_without_ranges_starts_over() {
    let (base, seen) = server().await;
    let temp = TempDir::new().unwrap();
    let partial = temp.path().join("a.part");
    std::fs::write(&partial, b"stale bytes from before").unwrap();

    // Known not to take ranges: no Range is sent.
    let mut validator = Validator::default();
    run(
        &origin(format!("{base}/whole.bin")),
        &partial,
        &mut validator,
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read(&partial).unwrap(), file());
    assert_eq!(seen.all()[0].range, None);

    // Thought to take them, it answers 200 to a Range: the partial is replaced, not appended.
    std::fs::write(&partial, b"stale").unwrap();
    let mut validator = Validator {
        ranges: true,
        ..Validator::default()
    };
    run(
        &origin(format!("{base}/whole.bin")),
        &partial,
        &mut validator,
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read(&partial).unwrap(), file());
    assert!(!validator.ranges);
}

#[tokio::test]
async fn a_missing_file_fails_for_good() {
    let (base, _seen) = server().await;
    let temp = TempDir::new().unwrap();
    let result = run(
        &origin(format!("{base}/gone")),
        &temp.path().join("a.part"),
        &mut Validator::default(),
    )
    .await;
    assert!(matches!(result, Err(Failure::Permanent(reason)) if reason.contains("404")));
}

#[tokio::test]
async fn cancelling_stops_a_running_download_and_keeps_what_came() {
    // A server that sends a little, then nothing more.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buffer = [0u8; 2048];
        let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut buffer).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100000\r\nAccept-Ranges: bytes\r\n\r\n")
            .await
            .unwrap();
        socket.write_all(&[7u8; 1000]).await.unwrap();
        socket.flush().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    });
    let temp = TempDir::new().unwrap();
    let partial = temp.path().join("a.part");
    let client = raygun_net::http_client().unwrap();
    let (stop, mut cancel) = watch::channel(false);
    let got = Arc::new(StdMutex::new(0u64));
    let seen = got.clone();
    let mut progress = move |done: u64, _: Option<u64>| {
        *seen.lock().unwrap() = done;
        if done >= 1000 {
            let _ = stop.send(true);
        }
    };
    let origin = origin(format!("http://{address}/slow"));
    let mut validator = Validator::default();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        fetch::fetch(
            &client,
            &origin,
            &partial,
            &mut validator,
            &mut cancel,
            &mut progress,
        ),
    )
    .await
    .expect("cancelling ends the download at once");
    assert_eq!(result, Err(Failure::Cancelled));
    assert_eq!(*got.lock().unwrap(), 1000);
    assert_eq!(std::fs::metadata(&partial).unwrap().len(), 1000);
    assert!(validator.ranges);
}

#[tokio::test]
async fn data_addresses_are_written_out() {
    let temp = TempDir::new().unwrap();
    let partial = temp.path().join("a.part");
    let fetched = run(
        &origin("data:text/plain;base64,aGVsbG8=".into()),
        &partial,
        &mut Validator::default(),
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read(&partial).unwrap(), b"hello");
    assert_eq!(fetched.mime.as_deref(), Some("text/plain"));
    assert_eq!(
        fetch::data_url_bytes("data:,a%20b").unwrap().0,
        b"a b".to_vec()
    );
}

#[test]
fn content_ranges_are_read() {
    assert_eq!(
        fetch::content_range("bytes 100-199/1000"),
        Some((100, Some(1000)))
    );
    assert_eq!(fetch::content_range("bytes 0-9/*"), Some((0, None)));
    assert_eq!(fetch::content_range("items 0-9/10"), None);
}

#[test]
fn names_come_from_the_server_the_browser_or_the_address() {
    assert_eq!(
        disposition_name("attachment; filename=\"report 2024.pdf\"").as_deref(),
        Some("report 2024.pdf")
    );
    assert_eq!(
        disposition_name("attachment; filename=plain.txt; filename*=UTF-8''%C3%A9t%C3%A9.txt")
            .as_deref(),
        Some("été.txt")
    );
    assert_eq!(
        disposition_name("attachment; filename=\"a;b.zip\"").as_deref(),
        Some("a;b.zip")
    );
    // A path in the name is dropped.
    assert_eq!(
        disposition_name("attachment; filename=\"../../etc/passwd\"").as_deref(),
        Some("passwd")
    );
    assert_eq!(disposition_name("inline"), None);

    assert_eq!(
        file_name(
            "https://example.org/get?id=1",
            Some("attachment; filename=\"book.epub\""),
            Some("get.bin"),
            None
        ),
        "book.epub"
    );
    assert_eq!(
        file_name("https://example.org/get", None, Some("guess.zip"), None),
        "guess.zip"
    );
    assert_eq!(
        file_name("https://example.org/files/My%20Notes.txt", None, None, None),
        "My Notes.txt"
    );
    // No extension: one from the type.
    assert_eq!(
        file_name(
            "https://example.org/download",
            None,
            None,
            Some("application/pdf")
        ),
        "download.pdf"
    );
    assert_eq!(
        file_name("https://example.org/", None, None, None),
        "download"
    );
    // Characters file systems refuse are replaced.
    assert_eq!(
        file_name("https://example.org/x", None, Some("a:b*c?.txt"), None),
        "a_b_c_.txt"
    );
}

#[test]
fn taken_names_get_a_number() {
    let taken = ["report.pdf", "report (1).pdf", "notes", ".env"];
    let is_taken = |name: &str| taken.contains(&name);
    assert_eq!(unique_name("other.pdf", is_taken), "other.pdf");
    assert_eq!(unique_name("report.pdf", is_taken), "report (2).pdf");
    assert_eq!(unique_name("notes", is_taken), "notes (1)");
    assert_eq!(unique_name(".env", is_taken), ".env (1)");
}

#[test]
fn finished_files_never_replace_one_already_there() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path().join("Raygun");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("report.pdf"), b"older").unwrap();

    let partial = temp.path().join("1.part");
    std::fs::write(&partial, b"newer").unwrap();
    let placed = move_into(&partial, &dir, "report.pdf").unwrap();
    assert_eq!(placed, dir.join("report (1).pdf"));
    assert_eq!(std::fs::read(dir.join("report.pdf")).unwrap(), b"older");
    assert_eq!(std::fs::read(&placed).unwrap(), b"newer");
    assert!(!partial.exists());

    let partial = temp.path().join("2.part");
    std::fs::write(&partial, b"third").unwrap();
    assert_eq!(
        move_into(&partial, &dir, "report.pdf").unwrap(),
        dir.join("report (2).pdf")
    );
}

#[test]
fn requests_from_the_browser_are_read() {
    let request: FileRequest = serde_json::from_value(serde_json::json!({
        "url": "https://example.org/a.zip",
        "userAgent": "Agent",
        "referer": "https://example.org/",
        "cookies": "a=1",
        "mime": "application/zip",
        "size": 1234.0,
        "destination": { "kind": "folder", "uri": "content://tree/x", "name": "Books" },
        "ticket": "t-1",
    }))
    .unwrap();
    assert_eq!(request.file_name(), "a.zip");
    assert_eq!(request.ticket.as_deref(), Some("t-1"));
    assert_eq!(
        request.destination,
        Destination::Folder {
            uri: "content://tree/x".into(),
            name: "Books".into()
        }
    );
    let request: FileRequest = serde_json::from_value(serde_json::json!({
        "url": "https://example.org/a",
        "destination": { "kind": "downloads" },
    }))
    .unwrap();
    assert_eq!(request.destination, Destination::Downloads);
    assert_eq!(request.ticket, None);
}

/// A kept download in `state`, started by the browser when it has a `ticket`.
fn stored(id: &str, state: WebsiteDownloadState, ticket: Option<&str>) -> Stored {
    Stored {
        item: WebsiteDownload {
            id: id.to_owned(),
            url: format!("https://example.org/{id}.zip"),
            name: format!("{id}.zip"),
            mime: Some("application/zip".into()),
            site: Some("example.org".into()),
            state,
            done_bytes: 250.0,
            total_bytes: Some(1000.0),
            bytes_per_second: None,
            resumable: false,
            folder: "Download/Raygun".into(),
            shown: None,
            error: None,
            created_at: 0.0,
        },
        origin: Origin::default(),
        destination: Destination::Downloads,
        validator: Validator::default(),
        saved: None,
        ticket: ticket.map(str::to_owned),
    }
}

#[test]
fn the_list_keeps_running_downloads_and_the_newest_finished() {
    let entry = |state| Entry {
        stored: stored("", state, None),
        cancel: None,
    };
    let mut entries: Vec<Entry> = (0..KEEP + 5)
        .map(|_| entry(WebsiteDownloadState::Done))
        .collect();
    entries.push(entry(WebsiteDownloadState::Queued));
    trim(&mut entries);
    assert_eq!(entries.len(), KEEP + 1);
    assert_eq!(
        entries.last().unwrap().stored.item.state,
        WebsiteDownloadState::Queued
    );
}

#[test]
fn the_browser_hears_only_about_the_downloads_it_started() {
    let mut done = stored("b", WebsiteDownloadState::Done, Some("t-b"));
    done.saved = Some("content://media/downloads/7".into());
    let kept = [
        stored("a", WebsiteDownloadState::Downloading, Some("t-a")),
        done,
        // Started some other way (an older one retried from Downloads, say).
        stored("c", WebsiteDownloadState::Downloading, None),
        stored("d", WebsiteDownloadState::Queued, Some("t-d")),
    ];
    let all: Vec<&Stored> = kept.iter().collect();
    let changed = ["a", "b", "c"].map(str::to_owned).into_iter().collect();

    let views = browser::turn(&changed, Vec::new(), &all);
    let tickets: Vec<&str> = views.iter().map(|view| view.ticket.as_str()).collect();
    assert_eq!(tickets, ["t-a", "t-b"]);
    assert_eq!(views[0].done_bytes, 250.0);
    assert_eq!(views[0].total_bytes, Some(1000.0));
    // A finished one carries the file, to open it from the browser.
    assert_eq!(
        views[1].saved.as_deref(),
        Some("content://media/downloads/7")
    );
    assert_eq!(views[1].mime.as_deref(), Some("application/zip"));

    // What's said as it is comes once, its latest word replacing the view.
    let said = vec![
        browser::BrowserDownload::gone("t-a"),
        browser::BrowserDownload::refused("t-x", "x.zip", "This download couldn't start"),
    ];
    let views = browser::turn(&changed, said, &all);
    let states: Vec<(&str, WebsiteDownloadState)> = views
        .iter()
        .map(|view| (view.ticket.as_str(), view.state))
        .collect();
    assert_eq!(
        states,
        [
            ("t-b", WebsiteDownloadState::Done),
            ("t-a", WebsiteDownloadState::Cancelled),
            ("t-x", WebsiteDownloadState::Failed),
        ]
    );
    assert_eq!(views[2].name, "x.zip");
    assert_eq!(
        views[2].error.as_deref(),
        Some("This download couldn't start")
    );
}

#[test]
fn only_a_browser_on_screen_is_told() {
    let feed = browser::Feed::default();
    // Desktop windows always hear (their menus); Android's browser while it's on screen.
    assert_eq!(feed.heard(), !cfg!(target_os = "android"));
    feed.watch();
    assert!(feed.heard());
    feed.unwatch();
    // One more leaving than came (a browser killed on screen, say) doesn't wrap around.
    feed.unwatch();
    assert_eq!(feed.heard(), !cfg!(target_os = "android"));
    feed.watch();
    assert!(feed.heard());

    feed.touch("a");
    feed.touch("");
    feed.say(browser::BrowserDownload::gone("t-x"));
    let (changed, said) = feed.take();
    assert_eq!(changed.into_iter().collect::<Vec<_>>(), ["a"]);
    assert_eq!(said, [browser::BrowserDownload::gone("t-x")]);
    let (changed, said) = feed.take();
    assert!(changed.is_empty() && said.is_empty());
}

#[test]
fn the_browser_reads_its_downloads_in_its_own_words() {
    let mut running = stored("a", WebsiteDownloadState::Downloading, Some("t-a"));
    running.item.total_bytes = None;
    let view = browser::view(&running).unwrap();
    assert_eq!(
        serde_json::to_value(&view).unwrap(),
        serde_json::json!({
            "ticket": "t-a",
            "id": "a",
            "name": "a.zip",
            "state": "downloading",
            "doneBytes": 250.0,
            "totalBytes": null,
            "folder": "Download/Raygun",
            "error": null,
            "saved": null,
            "mime": "application/zip",
        })
    );
    assert!(browser::view(&stored("b", WebsiteDownloadState::Done, None)).is_none());
}

#[test]
fn the_browsers_questions_are_read() {
    let read = |json| serde_json::from_value::<browser::BrowserAsk>(json).unwrap();
    assert_eq!(
        read(serde_json::json!({ "action": "watch", "tickets": ["t-a", "t-b"] })),
        browser::BrowserAsk::Watch {
            tickets: vec!["t-a".into(), "t-b".into()]
        }
    );
    assert_eq!(
        read(serde_json::json!({ "action": "unwatch" })),
        browser::BrowserAsk::Unwatch
    );
    assert_eq!(
        read(serde_json::json!({ "action": "cancel", "ticket": "t-a" })),
        browser::BrowserAsk::Cancel {
            ticket: "t-a".into()
        }
    );
    assert_eq!(
        read(serde_json::json!({ "action": "retry", "ticket": "t-a" })),
        browser::BrowserAsk::Retry {
            ticket: "t-a".into()
        }
    );
    assert!(
        serde_json::from_value::<browser::BrowserAsk>(serde_json::json!({ "action": "delete" }))
            .is_err()
    );
}
