//! Recording what Raygun plays (M46.10), for songs only YouTube has: its embed gives no file. On
//! Windows the capture is WASAPI process loopback over the main window's WebView2 browser
//! process and its children, where the embed's sound plays (the audio service), so nothing else
//! on the computer is recorded. Raygun's own process id hears nothing, though the browser process
//! is its child: Windows doesn't carry the loopback down to it (`examples/loopback_probe.rs`
//! shows both). The engine converts to float 48 kHz stereo; sound is written to a WAV only while
//! the recorded song plays (`set_writing`), so pauses and loading leave no gaps.
//!
//! Android would need AudioPlaybackCapture: a MediaProjection prompt each session and a
//! foreground service. Not done yet; the desktop app records.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::AppHandle;

/// Whether this build can record what it plays.
pub const SUPPORTED: bool = cfg!(windows);

/// The process whose tree plays Raygun's sound: the main window's WebView2 browser process.
#[cfg(windows)]
pub async fn sound_process(app: &AppHandle) -> Result<u32, String> {
    use tauri::Manager;

    let window = app
        .get_webview_window("main")
        .ok_or("Raygun's window is closed")?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .with_webview(move |platform| {
            let mut pid = 0u32;
            // SAFETY: `with_webview` runs this on the UI thread with the window's controller, and
            // `pid` outlives the call that writes it.
            let found = unsafe {
                platform
                    .controller()
                    .CoreWebView2()
                    .and_then(|webview| webview.BrowserProcessId(&mut pid))
            };
            let _ = sender.send(found.map(|()| pid).map_err(|error| error.to_string()));
        })
        .map_err(|error| error.to_string())?;
    tokio::time::timeout(std::time::Duration::from_secs(5), receiver)
        .await
        .map_err(|_| "The window didn't say where its sound plays".to_owned())?
        .map_err(|_| "The window didn't say where its sound plays".to_owned())?
}

#[cfg(not(windows))]
pub async fn sound_process(_app: &AppHandle) -> Result<u32, String> {
    Err("Recording what plays needs the desktop app for now".into())
}

/// What a finished capture wrote.
pub struct CaptureStats {
    pub path: PathBuf,
}

/// A running capture.
pub struct Capture {
    writing: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<Result<(), String>>>,
    path: PathBuf,
}

impl Capture {
    /// Starts capturing `process` and its children into a float WAV at `path`; nothing is written
    /// until `set_writing`.
    pub fn start(path: &Path, process: u32) -> Result<Self, String> {
        let writing = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (ready, started) = std::sync::mpsc::channel();
        let thread = {
            let (writing, stop, path) = (writing.clone(), stop.clone(), path.to_owned());
            std::thread::Builder::new()
                .name("raygun-record".into())
                .spawn(move || capture(&path, process, &writing, &stop, &ready))
                .map_err(|error| error.to_string())?
        };
        // Wait for the capture to open, so a failure shows before the song starts.
        match started.recv() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                let _ = thread.join();
                return Err(error);
            }
            Err(_) => {
                return Err(thread
                    .join()
                    .ok()
                    .and_then(Result::err)
                    .unwrap_or_else(|| "The recorder stopped".into()));
            }
        }
        Ok(Self {
            writing,
            stop,
            thread: Some(thread),
            path: path.to_owned(),
        })
    }

    /// Keeps sound (the recorded song plays) or drops it (paused, loading, another item).
    pub fn set_writing(&self, writing: bool) {
        self.writing.store(writing, Ordering::SeqCst);
    }

    /// Stops and closes the file. Blocking.
    pub fn finish(mut self) -> Result<CaptureStats, String> {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| "The recorder crashed".to_owned())??;
        }
        Ok(CaptureStats {
            path: self.path.clone(),
        })
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

#[cfg(windows)]
fn capture(
    path: &Path,
    process: u32,
    writing: &AtomicBool,
    stop: &AtomicBool,
    ready: &std::sync::mpsc::Sender<Result<(), String>>,
) -> Result<(), String> {
    use std::collections::VecDeque;

    use wasapi::{AudioClient, Direction, SampleType, StreamMode, WaveFormat, initialize_mta};

    const RATE: u32 = 48_000;
    const CHANNELS: u16 = 2;

    let opened = (|| {
        let _ = initialize_mta();
        let format = WaveFormat::new(
            32,
            32,
            &SampleType::Float,
            RATE as usize,
            CHANNELS.into(),
            None,
        );
        let mut client = AudioClient::new_application_loopback_client(process, true)
            .map_err(|error| format!("Windows refused to record Raygun's sound ({error})"))?;
        client
            .initialize_client(
                &format,
                &Direction::Capture,
                &StreamMode::EventsShared {
                    autoconvert: true,
                    buffer_duration_hns: 0,
                },
            )
            .map_err(|error| format!("Windows refused to record Raygun's sound ({error})"))?;
        let event = client
            .set_get_eventhandle()
            .map_err(|error| error.to_string())?;
        let capture = client
            .get_audiocaptureclient()
            .map_err(|error| error.to_string())?;
        let spec = hound::WavSpec {
            channels: CHANNELS,
            sample_rate: RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let writer = hound::WavWriter::create(path, spec).map_err(|error| error.to_string())?;
        client.start_stream().map_err(|error| error.to_string())?;
        Ok::<_, String>((client, event, capture, writer))
    })();
    let (client, event, capture, mut writer) = match opened {
        Ok(opened) => {
            let _ = ready.send(Ok(()));
            opened
        }
        Err(error) => {
            let _ = ready.send(Err(error.clone()));
            return Err(error);
        }
    };
    let mut queue: VecDeque<u8> = VecDeque::new();
    let mut result = Ok(());
    while !stop.load(Ordering::SeqCst) {
        // Nothing arrives while nothing plays; wake now and then to see if it's time to stop.
        let _ = event.wait_for_event(250);
        loop {
            match capture.get_next_packet_size() {
                Ok(Some(frames)) if frames > 0 => {}
                Ok(_) => break,
                Err(error) => {
                    result = Err(error.to_string());
                    break;
                }
            }
            if let Err(error) = capture.read_from_device_to_deque(&mut queue) {
                result = Err(error.to_string());
                break;
            }
        }
        if result.is_err() {
            break;
        }
        let keep = writing.load(Ordering::SeqCst);
        // Whole frames only (two float samples), so the channels never swap.
        let whole = queue.len() - queue.len() % 8;
        let bytes: Vec<u8> = queue.drain(..whole).collect();
        if keep {
            for sample in bytes.as_chunks::<4>().0 {
                let value = f32::from_le_bytes(*sample);
                if let Err(error) = writer.write_sample(value) {
                    result = Err(error.to_string());
                    break;
                }
            }
        }
    }
    let _ = client.stop_stream();
    writer.finalize().map_err(|error| error.to_string())?;
    result
}

#[cfg(not(windows))]
fn capture(
    _path: &Path,
    _process: u32,
    _writing: &AtomicBool,
    _stop: &AtomicBool,
    ready: &std::sync::mpsc::Sender<Result<(), String>>,
) -> Result<(), String> {
    let message = "Recording what plays needs the desktop app for now".to_owned();
    let _ = ready.send(Err(message.clone()));
    Err(message)
}
