//! How many threads a voice model runs on. Phones mix fast and slow cores: a model's work is
//! shared out evenly, so a thread on a slow core holds the others up, and voices are made on the
//! fast cores only. One core is left for the rest of the app (the reader, the player).

/// Threads for a voice model on this device.
pub fn threads() -> u32 {
    let cores = std::thread::available_parallelism().map_or(2, std::num::NonZero::get);
    let fast = core_speeds().map_or(cores, |speeds| fast_cores(&speeds));
    pick(cores, fast, cfg!(target_os = "android"))
}

/// `fast` of `cores`, less one for the app, at most 4 on a phone (more add little and drain the
/// battery) and 8 elsewhere.
fn pick(cores: usize, fast: usize, phone: bool) -> u32 {
    let most = if phone { 4 } else { 8 };
    fast.min(cores.saturating_sub(1)).clamp(1, most) as u32
}

/// How fast each core is, as the kernel says: its capacity (which counts what the core is, not
/// only its clock), else its highest clock.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(not(any(target_os = "android", target_os = "linux")), allow(dead_code))]
enum Speeds {
    Capacity(Vec<u64>),
    Clock(Vec<u64>),
}

/// The cores at least about half as fast as the fastest (by capacity), or at three quarters of
/// its clock: Snapdragon 855's four Cortex-A76 cores of eight, Tensor's four of eight, a
/// Dimensity 700's two.
fn fast_cores(speeds: &Speeds) -> usize {
    let (values, share) = match speeds {
        Speeds::Capacity(values) => (values, 0.45),
        Speeds::Clock(values) => (values, 0.75),
    };
    let fastest = values.iter().copied().max().unwrap_or(0) as f64;
    values
        .iter()
        .filter(|value| **value as f64 >= fastest * share)
        .count()
        .max(1)
}

#[cfg(any(target_os = "android", target_os = "linux"))]
fn core_speeds() -> Option<Speeds> {
    let read = |path: String| -> Option<Vec<u64>> {
        let cores = std::thread::available_parallelism().ok()?.get();
        (0..cores)
            .map(|core| {
                std::fs::read_to_string(path.replace("{}", &core.to_string()))
                    .ok()?
                    .trim()
                    .parse()
                    .ok()
            })
            .collect()
    };
    read("/sys/devices/system/cpu/cpu{}/cpu_capacity".into())
        .map(Speeds::Capacity)
        .or_else(|| {
            read("/sys/devices/system/cpu/cpu{}/cpufreq/cpuinfo_max_freq".into()).map(Speeds::Clock)
        })
}

/// Desktop processors' cores are alike (or the system says nothing of them).
#[cfg(not(any(target_os = "android", target_os = "linux")))]
fn core_speeds() -> Option<Speeds> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_voices_on_the_fast_cores_only() {
        // Snapdragon 855 (Pixel 4): four Cortex-A55, three A76, one faster A76.
        let pixel_4 = Speeds::Capacity(vec![261, 261, 261, 261, 871, 871, 871, 1024]);
        assert_eq!(fast_cores(&pixel_4), 4);
        assert_eq!(pick(8, fast_cores(&pixel_4), true), 4);
        // Tensor G1: four A55, two A76, two X1.
        let tensor = Speeds::Capacity(vec![160, 160, 160, 160, 498, 498, 1024, 1024]);
        assert_eq!(pick(8, fast_cores(&tensor), true), 4);
        // Dimensity 700: six A55 at nearly the clock of its two A76; their capacity tells.
        let dimensity = Speeds::Capacity(vec![382, 382, 382, 382, 382, 382, 1024, 1024]);
        assert_eq!(pick(8, fast_cores(&dimensity), true), 2);
        // Clocks only: 1.78 GHz cores aren't fast beside 2.84 GHz ones.
        let clocks = Speeds::Clock(vec![
            1_785_600, 1_785_600, 1_785_600, 1_785_600, 2_419_200, 2_419_200, 2_419_200, 2_841_600,
        ]);
        assert_eq!(fast_cores(&clocks), 4);
    }

    #[test]
    fn leaves_a_core_for_the_app() {
        // The emulator's four alike cores: three threads.
        assert_eq!(pick(4, 4, true), 3);
        assert_eq!(pick(2, 2, true), 1);
        assert_eq!(pick(1, 1, true), 1);
        // Desktops: up to eight.
        assert_eq!(pick(16, 16, false), 8);
        assert_eq!(pick(6, 6, false), 5);
        // Ten fast cores of a phone still make four.
        assert_eq!(pick(10, 10, true), 4);
        assert!(threads() >= 1);
    }
}
