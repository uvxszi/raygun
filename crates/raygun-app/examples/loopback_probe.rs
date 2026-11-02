//! Checks that WASAPI process loopback hears a process tree (M46.10 recording): captures the given
//! process id for a few seconds and prints how much sound arrived.
//! `cargo run -p raygun-app --example loopback_probe -- <pid> [seconds]`

#[cfg(windows)]
fn main() {
    use std::collections::VecDeque;

    use wasapi::{AudioClient, Direction, SampleType, StreamMode, WaveFormat, initialize_mta};

    let mut args = std::env::args().skip(1);
    let pid: u32 = args
        .next()
        .and_then(|arg| arg.parse().ok())
        .expect("a process id");
    let seconds: u64 = args.next().and_then(|arg| arg.parse().ok()).unwrap_or(5);
    let _ = initialize_mta();
    let format = WaveFormat::new(32, 32, &SampleType::Float, 48_000, 2, None);
    let mut client = AudioClient::new_application_loopback_client(pid, true).expect("client");
    client
        .initialize_client(
            &format,
            &Direction::Capture,
            &StreamMode::EventsShared {
                autoconvert: true,
                buffer_duration_hns: 0,
            },
        )
        .expect("initialize");
    let event = client.set_get_eventhandle().expect("event");
    let capture = client.get_audiocaptureclient().expect("capture client");
    client.start_stream().expect("start");
    let mut queue: VecDeque<u8> = VecDeque::new();
    let (mut packets, mut silent_waits) = (0u64, 0u64);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
    while std::time::Instant::now() < deadline {
        if event.wait_for_event(250).is_err() {
            silent_waits += 1;
        }
        while let Ok(Some(frames)) = capture.get_next_packet_size() {
            if frames == 0 {
                break;
            }
            packets += 1;
            capture.read_from_device_to_deque(&mut queue).expect("read");
        }
    }
    let _ = client.stop_stream();
    let bytes: Vec<u8> = queue.into_iter().collect();
    let samples: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|sample| f32::from_le_bytes(*sample))
        .collect();
    let peak = samples
        .iter()
        .fold(0f32, |peak, value| peak.max(value.abs()));
    let loud = samples.iter().filter(|value| value.abs() > 0.001).count();
    println!(
        "packets={packets} timeouts={silent_waits} samples={} seconds={:.1} loud={loud} peak={peak}",
        samples.len(),
        samples.len() as f64 / 96_000.0
    );
}

#[cfg(not(windows))]
fn main() {
    eprintln!("process loopback is Windows only");
}
