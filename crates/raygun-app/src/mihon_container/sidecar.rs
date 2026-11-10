//! One desktop sidecar: a `java -jar mihon-host.jar` process running a single Mihon extension's
//! own code, spoken to as JSON lines over stdin/stdout. The APK's dex is converted to a jar with
//! dex2jar once (cached by the APK's SHA-256); the sidecar loads it, answers the `manga.source`
//! calls, and is stopped after it sits idle. A crash is noticed on the next call, which restarts
//! it. Not built on Android (`cfg(not(target_os = "android"))`).
#![cfg(not(target_os = "android"))]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::sync::{Mutex, oneshot};

use super::tools::Tools;

/// A call takes at most this long end to end (the host's own budget is a little shorter).
const CALL_TIMEOUT: Duration = Duration::from_secs(100);
/// The sidecar stops after this long without a call.
pub const IDLE: Duration = Duration::from_secs(300);
/// The JVM heap cap.
const HEAP: &str = "-Xmx256m";

/// How to start and load one extension's sidecar.
pub struct Load {
    /// The verified APK on disk.
    pub apk: PathBuf,
    /// Where converted jars and the extension's data live (`<data>/mihon/<package>/`).
    pub work: PathBuf,
    pub package: String,
    /// `tachiyomi.extension.class` from the APK (may start with `.`).
    pub class: String,
    pub user_agent: String,
    /// Raygun's cookies for the source's hosts, as the host's `load` takes them.
    pub cookies: Vec<Value>,
}

pub struct Sidecar {
    tools: Arc<Tools>,
    load: Load,
    state: Mutex<Option<Running>>,
    next_id: AtomicU64,
    last_used: Mutex<Instant>,
}

struct Running {
    child: Child,
    stdin: ChildStdin,
    /// Calls waiting for their answer, by request id.
    waiting: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,
    /// The sources the host registered on load.
    sources: Vec<Value>,
}

impl Sidecar {
    pub fn new(tools: Arc<Tools>, load: Load) -> Self {
        Self {
            tools,
            load,
            state: Mutex::new(None),
            next_id: AtomicU64::new(1),
            last_used: Mutex::new(Instant::now()),
        }
    }

    /// Whether a call has come in within `window` (the idle sweeper keeps running sidecars that
    /// have; stops the rest).
    pub async fn touched_within(&self, window: Duration) -> bool {
        if self.state.lock().await.is_none() {
            return true; // not running: nothing to stop
        }
        self.last_used.lock().await.elapsed() < window
    }

    /// The sources the extension registers (id, name, lang, supportsLatest, baseUrl), starting
    /// the sidecar if needed.
    pub async fn sources(&self) -> Result<Vec<Value>, String> {
        let mut state = self.state.lock().await;
        self.running(&mut state)
            .await
            .map(|running| running.sources.clone())
    }

    /// One `manga.source` call (`popular`, `latest`, `search`, `details`, `chapters`, `pages`,
    /// `image`), its params already shaped for the host. Restarts a crashed sidecar once.
    pub async fn call(&self, method: &str, mut params: Value) -> Result<Value, String> {
        *self.last_used.lock().await = Instant::now();
        for attempt in 0..2 {
            let mut state = self.state.lock().await;
            let running = match self.running(&mut state).await {
                Ok(running) => running,
                Err(error) => return Err(error),
            };
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            if let Some(object) = params.as_object_mut() {
                object.insert("id".into(), json!(id));
                object.insert("method".into(), json!(method));
            }
            let (tx, rx) = oneshot::channel();
            running.waiting.lock().await.insert(id, tx);
            let line = format!(
                "{}\n",
                Value::Object(params.as_object().cloned().unwrap_or_default())
            );
            if running.stdin.write_all(line.as_bytes()).await.is_err()
                || running.stdin.flush().await.is_err()
            {
                // The sidecar went away: drop it and try once more.
                *state = None;
                drop(state);
                if attempt == 0 {
                    continue;
                }
                return Err("the Mihon host stopped".into());
            }
            drop(state);
            match tokio::time::timeout(CALL_TIMEOUT, rx).await {
                Ok(Ok(answer)) => return read_answer(answer),
                Ok(Err(_)) => {
                    // The reader closed: the sidecar crashed.
                    *self.state.lock().await = None;
                    if attempt == 0 {
                        continue;
                    }
                    return Err("the Mihon host stopped before answering".into());
                }
                Err(_) => {
                    *self.state.lock().await = None;
                    return Err(format!(
                        "the source took longer than {}s",
                        CALL_TIMEOUT.as_secs()
                    ));
                }
            }
        }
        Err("the Mihon host couldn't be reached".into())
    }

    /// Stops the sidecar if it's running (on idle, or when the extension is removed).
    pub async fn stop(&self) {
        if let Some(mut running) = self.state.lock().await.take() {
            let _ = running.child.start_kill();
        }
    }

    async fn running<'a>(&self, state: &'a mut Option<Running>) -> Result<&'a mut Running, String> {
        if state.is_none() {
            *state = Some(self.start().await?);
        }
        Ok(state.as_mut().expect("just started"))
    }

    /// Converts the dex (if needed), starts the JVM and loads the extension.
    async fn start(&self) -> Result<Running, String> {
        let java = self
            .tools
            .java_path()
            .ok_or("the Java runtime isn't installed")?;
        let jar = self.converted_jar().await?;
        let mut child = command(&java)
            .arg(HEAP)
            .arg("-XX:+UseSerialGC")
            .arg("-Djava.awt.headless=true")
            .arg("-jar")
            .arg(self.tools.host_jar())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| format!("Raygun couldn't start the Mihon host: {error}"))?;
        let stdin = child.stdin.take().expect("piped");
        let stdout = child.stdout.take().expect("piped");
        if let Some(stderr) = child.stderr.take() {
            let package = self.load.package.clone();
            tauri::async_runtime::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!(target: "mihon_host", package = %package, "{line}");
                }
            });
        }
        let waiting: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>> = Arc::default();
        reader(stdout, waiting.clone());
        let mut running = Running {
            child,
            stdin,
            waiting,
            sources: Vec::new(),
        };
        let load = json!({
            "jar": jar.to_string_lossy(),
            "dataDir": self.load.work.join("data").to_string_lossy(),
            "packageName": self.load.package,
            "className": self.load.class,
            "userAgent": self.load.user_agent,
            "cookies": self.load.cookies,
        });
        running.sources = self
            .ask(&mut running, "load", load)
            .await?
            .as_array()
            .cloned()
            .ok_or("the Mihon host didn't list the extension's sources")?;
        Ok(running)
    }

    /// A single call on a sidecar being started (before it's in `state`).
    async fn ask(
        &self,
        running: &mut Running,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut object = params.as_object().cloned().unwrap_or_default();
        object.insert("id".into(), json!(id));
        object.insert("method".into(), json!(method));
        let (tx, rx) = oneshot::channel();
        running.waiting.lock().await.insert(id, tx);
        let line = format!("{}\n", Value::Object(object));
        running
            .stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|_| "the Mihon host didn't start".to_owned())?;
        running
            .stdin
            .flush()
            .await
            .map_err(|_| "the Mihon host didn't start".to_owned())?;
        match tokio::time::timeout(CALL_TIMEOUT, rx).await {
            Ok(Ok(answer)) => read_answer(answer),
            _ => Err("the Mihon host didn't answer when loading".into()),
        }
    }

    /// The APK's classes as a jar, converted by dex2jar once and kept by the APK's name.
    async fn converted_jar(&self) -> Result<PathBuf, String> {
        let jar = self.load.work.join("classes.jar");
        if jar.is_file() {
            return Ok(jar);
        }
        let java = self
            .tools
            .java_path()
            .ok_or("the Java runtime isn't installed")?;
        let lib = self.tools.dex2jar_dir().ok_or("dex2jar isn't installed")?;
        tokio::fs::create_dir_all(&self.load.work)
            .await
            .map_err(|error| error.to_string())?;
        let partial = jar.with_extension("part.jar");
        let output = command(&java)
            .arg("-cp")
            .arg(lib.join("*"))
            .arg(self.tools.dex2jar_main())
            .arg("--force")
            .arg("--output")
            .arg(&partial)
            .arg(&self.load.apk)
            .output()
            .await
            .map_err(|error| format!("dex2jar didn't run: {error}"))?;
        if !partial.is_file() {
            return Err(format!(
                "dex2jar couldn't read the extension: {}",
                String::from_utf8_lossy(&output.stderr)
                    .lines()
                    .next()
                    .unwrap_or_default()
            ));
        }
        tokio::fs::rename(&partial, &jar)
            .await
            .map_err(|error| error.to_string())?;
        Ok(jar)
    }
}

/// Reads the host's answers and wakes each waiting call by its id, on a background task that ends
/// when the pipe closes (the sidecar stopped).
fn reader(
    stdout: tokio::process::ChildStdout,
    waiting: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,
) {
    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(answer) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let Some(id) = answer.get("id").and_then(Value::as_u64) else {
                continue;
            };
            if let Some(sender) = waiting.lock().await.remove(&id) {
                let _ = sender.send(answer);
            }
        }
        // The pipe closed: fail every waiting call so it can restart.
        waiting.lock().await.clear();
    });
}

/// A host answer: its `result`, or an `error` turned into a message (a bot check is named so the
/// caller can route it to the pass-by-hand flow).
fn read_answer(answer: Value) -> Result<Value, String> {
    if let Some(result) = answer.get("result") {
        return Ok(result.clone());
    }
    if let Some(error) = answer.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("the source failed");
        return Err(message.to_owned());
    }
    Err("the Mihon host sent an answer Raygun couldn't read".into())
}

fn command(java: &Path) -> tokio::process::Command {
    #[cfg(windows)]
    {
        let mut command = tokio::process::Command::new(java);
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        command
    }
    #[cfg(not(windows))]
    tokio::process::Command::new(java)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_error_answer_becomes_its_message() {
        let answer = json!({ "id": 1, "error": { "message": "www.x.com wants a browser check", "type": "CloudflareException" } });
        assert_eq!(
            read_answer(answer).unwrap_err(),
            "www.x.com wants a browser check"
        );
        let answer = json!({ "id": 2, "result": { "items": [], "hasNextPage": false } });
        assert!(read_answer(answer).unwrap().get("items").is_some());
    }

    /// Runs a real extension's own code through the sidecar, end to end. Needs the tools in place
    /// (executing a converted APK, which the development sandbox blocks), so it's ignored and
    /// driven by the lead on a desktop:
    ///   MIHON_RUN_ROOT=<dir with tools/mihon/{jre,dex2jar}> MIHON_HOST_JAR=<mihon-host.jar> \
    ///   MIHON_RUN_APK=<checked .apk> MIHON_RUN_CLASS=<tachiyomi.extension.class> \
    ///   cargo test -p raygun-app --lib -- --ignored desktop_runs_a_real_extension --nocapture
    #[tokio::test]
    #[ignore = "runs a third-party extension's own code; the lead drives it"]
    async fn desktop_runs_a_real_extension() {
        let env = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
        let root = PathBuf::from(env("MIHON_RUN_ROOT"));
        let tools = Arc::new(Tools::new(
            &root,
            PathBuf::from(env("MIHON_HOST_JAR")),
            reqwest::Client::new(),
        ));
        assert!(
            tools.state().ready(),
            "the tools aren't in place: {:?}",
            tools.state()
        );
        let load = Load {
            apk: PathBuf::from(env("MIHON_RUN_APK")),
            work: root.join("run-test"),
            package: "eu.kanade.tachiyomi.extension.test".into(),
            class: env("MIHON_RUN_CLASS"),
            user_agent: "Raygun".into(),
            cookies: Vec::new(),
        };
        let sidecar = Sidecar::new(tools, load);
        let sources = sidecar
            .sources()
            .await
            .expect("the extension lists sources");
        let id = sources[0]["id"].as_str().expect("a source id").to_owned();
        println!("sources: {sources:?}");
        let page = sidecar
            .call("popular", json!({ "source": id, "page": 1 }))
            .await
            .expect("a popular page");
        let items = page["items"].as_array().expect("items");
        println!("{} popular titles, first {:?}", items.len(), items.first());
        assert!(!items.is_empty());
        sidecar.stop().await;
    }
}
