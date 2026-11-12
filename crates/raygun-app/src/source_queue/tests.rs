use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use raygun_model::{ServerKind, SourceFormat, StremioAddon};

use super::*;

fn found(kind: DetectedKind, name: &str) -> DetectedSource {
    DetectedSource {
        kind,
        format: (kind != DetectedKind::Server).then_some(SourceFormat::Raygun),
        server: None,
        url: format!("https://{}.example/", name.to_lowercase().replace(' ', "-")),
        name: name.to_owned(),
        description: None,
        added: false,
        addon: None,
        entries: Vec::new(),
        note: None,
    }
}

fn server(name: &str) -> DetectedSource {
    DetectedSource {
        server: Some(ServerKind::Jellyfin),
        ..found(DetectedKind::Server, name)
    }
}

fn addon(name: &str, needs_setup: bool) -> DetectedSource {
    let url = format!("https://{}.example", name.to_lowercase());
    DetectedSource {
        addon: Some(StremioAddon {
            id: String::new(),
            url: format!("{url}/manifest.json"),
            name: name.to_owned(),
            description: None,
            logo: None,
            version: "1.0.0".to_owned(),
            resources: vec!["stream".to_owned()],
            types: vec!["movie".to_owned()],
            catalogs: 0,
            adult: false,
            error: None,
            enabled: true,
            provides: Vec::new(),
            p2p: false,
            configure_url: needs_setup.then(|| format!("{url}/configure")),
            needs_setup,
            failing: false,
        }),
        format: None,
        ..found(DetectedKind::Addon, name)
    }
}

fn shown(status: &SourceQueueStatus) -> Vec<(&str, SourceQueueState)> {
    status
        .items
        .iter()
        .map(|item| (item.address.as_str(), item.state))
        .collect()
}

#[test]
fn a_list_is_read_line_by_line_each_address_once() {
    let list = read_list(
        "\u{feff}https://a.example/index.json\r\n\n   # Paraline and friends\n  https://b.example/manifest.json  \nstremio://b.example/manifest.json\nhttps://A.example/index.json/\n#https://c.example/index.json\nmusic.example\n",
    );
    assert_eq!(
        list.addresses,
        [
            "https://a.example/index.json",
            "https://b.example/manifest.json",
            "music.example"
        ]
    );
    // Stremio's link to the same manifest, and the same index with its host in capitals.
    assert_eq!(list.duplicates, 2);
    assert_eq!(read_list("\n  \n# only a note\n"), List::default());
    assert_eq!(
        address_key("music.example"),
        address_key("https://Music.example/")
    );
}

#[test]
fn addresses_waiting_arent_queued_twice_and_a_new_list_keeps_what_waits_for_you() {
    use SourceQueueState::{AlreadyAdded, Failed, NeedsSignIn, Waiting};
    let mut queue = Queue::default();
    assert_eq!(queue.enqueue(&read_list("a.example\nb.example")), (2, 0, 0));
    // `a` waits already; `c` joins the same list.
    assert_eq!(
        queue.enqueue(&read_list("https://a.example/\nc.example")),
        (1, 1, 0)
    );
    let status = queue.status();
    assert_eq!((status.batch, status.total), (1, 3));
    assert!(status.running);
    assert_eq!(
        shown(&status),
        [
            ("a.example", Waiting),
            ("b.example", Waiting),
            ("c.example", Waiting)
        ]
    );

    let (id, address) = queue.start_next().unwrap();
    assert_eq!(address, "a.example");
    assert!(queue.start_next().is_none(), "one at a time");
    assert_eq!(queue.status().current.as_deref(), Some("a.example"));
    assert_eq!(
        queue.looked(id, Err("Couldn't reach a.example".to_owned())),
        Step::Done
    );
    let (id, _) = queue.start_next().unwrap();
    assert_eq!(queue.looked(id, Ok(server("Home Media"))), Step::Done);
    let (id, _) = queue.start_next().unwrap();
    let known = DetectedSource {
        added: true,
        ..found(DetectedKind::Repository, "Known")
    };
    assert_eq!(queue.looked(id, Ok(known)), Step::Done);
    assert!(queue.start_next().is_none());

    let status = queue.status();
    assert!(!status.running);
    assert_eq!(
        shown(&status),
        [
            ("a.example", Failed),
            ("b.example", NeedsSignIn),
            ("c.example", AlreadyAdded)
        ]
    );
    assert_eq!(
        (
            status.failed,
            status.needs_sign_in,
            status.already_added,
            status.total
        ),
        (1, 1, 1, 3)
    );
    let signing_in = &status.items[1];
    assert_eq!(signing_in.server, Some(ServerKind::Jellyfin));
    assert_eq!(signing_in.name.as_deref(), Some("Home Media"));
    assert_eq!(
        signing_in.url.as_deref(),
        Some("https://home-media.example/")
    );
    assert_eq!(
        status.items[0].error.as_deref(),
        Some("Couldn't reach a.example")
    );

    // The next list starts a batch: what ended goes, what waits for the person stays outside its
    // counts.
    queue.enqueue(&read_list("d.example"));
    let status = queue.status();
    assert_eq!((status.batch, status.total, status.failed), (2, 1, 0));
    assert_eq!(
        shown(&status),
        [
            ("a.example", Failed),
            ("b.example", NeedsSignIn),
            ("d.example", Waiting)
        ]
    );
}

#[test]
fn cancelling_retrying_signing_in_and_dismissing() {
    use SourceQueueState::{Added, Checking, Waiting};
    let mut queue = Queue::default();
    queue.enqueue(&read_list("a.example\nb.example\nc.example\nd.example"));
    let (a, _) = queue.start_next().unwrap();
    queue.looked(a, Err("Couldn't reach a.example".to_owned()));
    let (b, _) = queue.start_next().unwrap();
    assert_eq!(queue.state_of(b), Some(Checking));
    // The look running is called off at once; its late answer changes nothing.
    assert!(queue.cancel(Some(b)));
    assert_eq!(
        queue.looked(b, Ok(found(DetectedKind::Repository, "B"))),
        Step::Skip
    );
    assert!(!queue.cancel(None), "nothing else was being looked at");
    let status = queue.status();
    assert_eq!((status.cancelled, status.failed), (3, 1));
    assert!(!status.running);

    // Retrying is a list of its own, in the failures' places.
    assert_eq!(queue.retry(), 1);
    let status = queue.status();
    assert_eq!(status.batch, 2);
    assert_eq!(shown(&status), [("a.example", Waiting)]);
    assert_eq!(status.items[0].error, None);
    let (again, _) = queue.start_next().unwrap();
    assert_eq!(again, a);
    queue.looked(again, Ok(server("Media")));
    assert!(queue.signed_in(again));
    assert!(!queue.signed_in(again), "once");
    assert_eq!(shown(&queue.status()), [("a.example", Added)]);
    assert_eq!(queue.status().added, 1);

    queue.enqueue(&read_list("e.example\nf.example"));
    queue.cancel(Some(queue.status().items[1].id));
    queue.dismiss(Some(queue.status().items[1].id));
    queue.dismiss(None);
    assert_eq!(
        shown(&queue.status()),
        [("e.example", Waiting)],
        "only what waits stays"
    );
}

#[test]
fn extensions_installed_load_once_nothing_is_left() {
    let mut queue = Queue::default();
    queue.enqueue(&read_list("x.example/pkg.zip\nr.example/index.json"));
    let (id, _) = queue.start_next().unwrap();
    assert_eq!(
        queue.looked(id, Ok(found(DetectedKind::Extension, "Pkg"))),
        Step::Add
    );
    assert_eq!(queue.state_of(id), Some(SourceQueueState::Adding));
    assert!(
        queue.start_next().is_none(),
        "one being added holds the rest"
    );
    queue.added(id, Ok(()));
    assert!(!queue.start_loading(), "one still waits");
    let (id, _) = queue.start_next().unwrap();
    queue.looked(id, Ok(found(DetectedKind::Repository, "Repo")));
    queue.added(id, Err("Couldn't add the repository".to_owned()));
    assert!(queue.status().running, "loading follows");
    assert!(queue.start_loading());
    assert!(queue.status().loading);
    assert!(
        queue.start_next().is_none(),
        "nothing starts while they load"
    );
    queue.finish_loading();
    let status = queue.status();
    assert!(!status.running);
    assert_eq!((status.added, status.failed), (1, 1));
    assert!(!queue.start_loading(), "loaded once");
}

#[test]
fn offline_mode_puts_a_look_it_stopped_back_in_line() {
    let mut queue = Queue::default();
    queue.enqueue(&read_list("a.example\nb.example"));
    let (id, _) = queue.start_next().unwrap();
    assert_eq!(
        queue.looked(id, Err("Offline mode is on".to_owned())),
        Step::Offline
    );
    queue.set_held(true);
    let status = queue.status();
    assert!(status.paused && status.running);
    assert_eq!(status.failed, 0);
    // It's first again.
    assert_eq!(queue.start_next().unwrap().0, id);
}

#[test]
fn a_list_holds_so_many_addresses() {
    let mut queue = Queue::default();
    let text: Vec<String> = (0..MAX_QUEUED + 3)
        .map(|index| format!("s{index}.example"))
        .collect();
    assert_eq!(
        queue.enqueue(&read_list(&text.join("\n"))),
        (MAX_QUEUED as u32, 0, 3)
    );
}

#[cfg(not(target_os = "android"))]
#[test]
fn list_files_are_read_and_a_source_stands_for_itself() {
    let temp = tempfile::TempDir::new().unwrap();
    let list = temp.path().join("sources.txt");
    std::fs::write(
        &list,
        "# mine\nhttps://a.example/index.json\n\nhttps://a.example/index.json\nb.example\n",
    )
    .unwrap();
    assert_eq!(
        read_list_file(&list).unwrap(),
        ["https://a.example/index.json", "b.example"]
    );
    let array = temp.path().join("list.json");
    std::fs::write(&array, r#"["https://a.example/index.json", "c.example"]"#).unwrap();
    assert_eq!(
        read_list_file(&array).unwrap(),
        ["https://a.example/index.json", "c.example"]
    );
    let index = temp.path().join("index.json");
    std::fs::write(
        &index,
        r#"{"formatVersion":1,"name":"Mine","extensions":[]}"#,
    )
    .unwrap();
    assert_eq!(
        read_list_file(&index).unwrap(),
        [index.display().to_string()]
    );
    let script = temp.path().join("Example.lua");
    std::fs::write(
        &script,
        "-- {\"id\":7,\"ver\":\"1.2.0\"}\nlocal baseURL = \"https://x.example\"\nreturn { name = \"Example\" }\n",
    )
    .unwrap();
    assert_eq!(
        read_list_file(&script).unwrap(),
        [script.display().to_string()]
    );
    let notes = temp.path().join("notes.txt");
    std::fs::write(&notes, "# nothing yet\n\n").unwrap();
    assert!(
        read_list_file(&notes)
            .unwrap_err()
            .contains("lists no addresses")
    );
}

// ---- The worker ----

/// Answers looks from a table (an address's first answers before it), each look waiting on
/// `gate` when there is one; keeps the looks and additions in order. A look answered with
/// Offline Mode's error turns Offline Mode on, as the switch would have.
/// An address's first answers, given before the table's.
type Early = Arc<Mutex<HashMap<String, Vec<Result<DetectedSource, String>>>>>;

#[derive(Clone)]
struct Fake {
    answers: Arc<HashMap<String, Result<DetectedSource, String>>>,
    first: Early,
    gate: Option<Arc<tokio::sync::Semaphore>>,
    looked: Arc<Mutex<Vec<String>>>,
    added: Arc<Mutex<Vec<String>>>,
    loads: Arc<AtomicU32>,
    running: Arc<AtomicU32>,
    most: Arc<AtomicU32>,
    offline: Arc<tokio::sync::watch::Sender<bool>>,
}

impl Fake {
    fn new(answers: Vec<(&str, Result<DetectedSource, String>)>) -> Self {
        Self {
            answers: Arc::new(
                answers
                    .into_iter()
                    .map(|(address, answer)| (address.to_owned(), answer))
                    .collect(),
            ),
            first: Arc::default(),
            gate: None,
            looked: Arc::default(),
            added: Arc::default(),
            loads: Arc::default(),
            running: Arc::default(),
            most: Arc::default(),
            offline: Arc::new(tokio::sync::watch::channel(false).0),
        }
    }

    fn first(self, address: &str, answer: Result<DetectedSource, String>) -> Self {
        self.first
            .lock()
            .unwrap()
            .entry(address.to_owned())
            .or_default()
            .push(answer);
        self
    }

    fn looked(&self) -> Vec<String> {
        self.looked.lock().unwrap().clone()
    }

    fn added(&self) -> Vec<String> {
        self.added.lock().unwrap().clone()
    }
}

impl Sources for Fake {
    async fn look(&self, address: &str) -> Result<DetectedSource, String> {
        let now = self.running.fetch_add(1, Ordering::SeqCst) + 1;
        self.most.fetch_max(now, Ordering::SeqCst);
        if let Some(gate) = &self.gate {
            gate.acquire().await.unwrap().forget();
        }
        self.running.fetch_sub(1, Ordering::SeqCst);
        self.looked.lock().unwrap().push(address.to_owned());
        let early = self
            .first
            .lock()
            .unwrap()
            .get_mut(address)
            .filter(|answers| !answers.is_empty())
            .map(|answers| answers.remove(0));
        let answer = early.unwrap_or_else(|| {
            self.answers.get(address).cloned().unwrap_or_else(|| {
                Err(format!(
                    "Raygun found no repository, addon, server or extension at {address}"
                ))
            })
        });
        if let Err(error) = &answer
            && raygun_net::network_mode::mentions_offline(error)
        {
            self.offline.send_replace(true);
        }
        answer
    }

    async fn add(&self, address: &str) -> Result<(), String> {
        self.added.lock().unwrap().push(address.to_owned());
        Ok(())
    }

    async fn load(&self) {
        self.loads.fetch_add(1, Ordering::SeqCst);
    }

    fn offline(&self) -> bool {
        *self.offline.borrow()
    }

    fn wait_online(&self) -> impl Future<Output = ()> + Send {
        let mut changes = self.offline.subscribe();
        async move {
            let _ = changes.wait_for(|offline| !offline).await;
        }
    }
}

async fn until(queue: &SourceQueue, done: impl Fn(&SourceQueueStatus) -> bool) {
    for _ in 0..600 {
        if done(&queue.status()) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("the queue didn't get there: {:#?}", queue.status());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_worker_adds_in_order_and_goes_on_past_what_waits_for_you() {
    use SourceQueueState::{Added, AlreadyAdded, Failed, NeedsSetup, NeedsSignIn};
    let fake = Fake::new(vec![
        (
            "https://r.example/index.json",
            Ok(found(DetectedKind::Repository, "Repo")),
        ),
        ("media.example", Ok(server("Home Media"))),
        (
            "https://x.example/pkg.zip",
            Ok(found(DetectedKind::Extension, "Pkg")),
        ),
        (
            "https://gone.example/index.json",
            Err("Couldn't reach gone.example".to_owned()),
        ),
        (
            "https://films.example/manifest.json",
            Ok(addon("Films", true)),
        ),
        (
            "https://known.example/index.json",
            Ok(DetectedSource {
                added: true,
                ..found(DetectedKind::Repository, "Known")
            }),
        ),
        (
            "https://busy.example/manifest.json",
            Ok(addon("Busy", false)),
        ),
    ])
    // A host that asks to slow down is asked again.
    .first(
        "https://busy.example/manifest.json",
        Err("busy.example answered HTTP 429".to_owned()),
    );
    let events = Arc::new(Mutex::new(Vec::<SourceQueueStatus>::new()));
    let seen = events.clone();
    let queue = SourceQueue::start(fake.clone(), move |status| {
        seen.lock().unwrap().push(status.clone());
    });
    let queued = queue.enqueue(
        "https://r.example/index.json\nmedia.example\n# one on its own\nhttps://x.example/pkg.zip\n\nhttps://gone.example/index.json\nhttps://films.example/manifest.json\nhttps://known.example/index.json\nhttps://r.example/index.json\nhttps://busy.example/manifest.json\n",
    );
    assert_eq!((queued.queued, queued.duplicates), (7, 1));
    until(&queue, |status| !status.running).await;

    assert_eq!(
        fake.looked(),
        [
            "https://r.example/index.json",
            "media.example",
            "https://x.example/pkg.zip",
            "https://gone.example/index.json",
            "https://films.example/manifest.json",
            "https://known.example/index.json",
            "https://busy.example/manifest.json",
            "https://busy.example/manifest.json",
        ]
    );
    assert_eq!(
        fake.added(),
        [
            "https://r.example/index.json",
            "https://x.example/pkg.zip",
            "https://busy.example/manifest.json",
        ]
    );
    assert_eq!(fake.most.load(Ordering::SeqCst), 1, "one look at a time");
    assert_eq!(fake.loads.load(Ordering::SeqCst), 1, "loaded once");
    let status = queue.status();
    assert_eq!(
        status
            .items
            .iter()
            .map(|item| item.state)
            .collect::<Vec<_>>(),
        [
            Added,
            NeedsSignIn,
            Added,
            Failed,
            NeedsSetup,
            AlreadyAdded,
            Added
        ]
    );
    assert_eq!(
        (
            status.added,
            status.already_added,
            status.needs_sign_in,
            status.needs_setup,
            status.failed,
            status.total
        ),
        (3, 1, 1, 1, 1, 7)
    );
    assert_eq!(
        status.items[4].configure_url.as_deref(),
        Some("https://films.example/configure")
    );
    let events = events.lock().unwrap();
    assert!(events.iter().any(|status| {
        status.current.as_deref() == Some("https://gone.example/index.json")
            && status.items[3].state == SourceQueueState::Checking
    }));
    assert!(events.iter().any(|status| status.loading));
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_calls_off_the_look_running_and_what_waits() {
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let fake = Fake {
        gate: Some(gate.clone()),
        ..Fake::new(vec![
            ("a.example", Ok(found(DetectedKind::Repository, "A"))),
            ("b.example", Ok(found(DetectedKind::Repository, "B"))),
            ("d.example", Ok(found(DetectedKind::Repository, "D"))),
        ])
    };
    let queue = SourceQueue::start(fake.clone(), |_| {});
    queue.enqueue("a.example\nb.example\nc.example");
    until(&queue, |status| {
        status.current.as_deref() == Some("a.example")
    })
    .await;
    queue.cancel(None);
    until(&queue, |status| !status.running).await;
    gate.add_permits(5);
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(queue.status().cancelled, 3);
    assert!(fake.looked().is_empty(), "the look running was dropped");
    assert!(fake.added().is_empty());

    // The next list goes on as usual.
    queue.enqueue("d.example");
    until(&queue, |status| status.batch == 2 && !status.running).await;
    assert_eq!(fake.looked(), ["d.example"]);
    assert_eq!(fake.added(), ["d.example"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn offline_mode_holds_the_queue_until_it_ends() {
    let fake = Fake::new(vec![
        ("a.example", Ok(found(DetectedKind::Repository, "A"))),
        ("b.example", Ok(found(DetectedKind::Repository, "B"))),
        ("c.example", Ok(found(DetectedKind::Repository, "C"))),
    ])
    .first("b.example", Err("Offline mode is on".to_owned()));
    fake.offline.send_replace(true);
    let queue = SourceQueue::start(fake.clone(), |_| {});
    queue.enqueue("a.example\nb.example\nc.example");
    until(&queue, |status| status.paused).await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(fake.looked().is_empty(), "nothing is asked while offline");
    assert!(queue.status().running);

    // `b`'s look meets Offline Mode turning on again: it waits its turn, ahead of `c`.
    fake.offline.send_replace(false);
    let asked = fake.clone();
    until(&queue, |status| status.paused && asked.looked().len() == 2).await;
    assert_eq!(fake.looked(), ["a.example", "b.example"]);
    assert_eq!(queue.status().failed, 0);
    fake.offline.send_replace(false);
    until(&queue, |status| !status.running).await;
    assert_eq!(
        fake.looked(),
        ["a.example", "b.example", "b.example", "c.example"]
    );
    assert_eq!(fake.added(), ["a.example", "b.example", "c.example"]);
}
