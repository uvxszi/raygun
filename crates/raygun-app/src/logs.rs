//! Raygun's log files: one a day, `logs/raygun.<YYYY-MM-DD>.log`, written by tracing-appender.
//! Days that are over are gzipped (`.log.gz`, a tenth of the size or less; any gzip tool reads
//! them), and only the latest days are kept in either form. The newest day's file stays plain:
//! the appender still writes it.

use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flate2::Compression;
use flate2::write::GzEncoder;

const PREFIX: &str = "raygun.";
const PLAIN: &str = ".log";
const PACKED: &str = ".log.gz";
/// A gzip being written; one left by a stop halfway goes at the next tidy.
const PART: &str = ".part";

/// How often a Raygun that keeps running (Android's services) tidies again.
const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// A day's log file: its date and whether it's gzipped.
fn day_of(name: &str) -> Option<(&str, bool)> {
    let rest = name.strip_prefix(PREFIX)?;
    if let Some(day) = rest.strip_suffix(PACKED) {
        return Some((day, true));
    }
    rest.strip_suffix(PLAIN).map(|day| (day, false))
}

/// Gzips `plain` beside itself, then removes it.
fn pack(plain: &Path) -> io::Result<()> {
    let name = plain
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let packed = plain.with_file_name(format!("{name}.gz"));
    let part = plain.with_file_name(format!("{name}.gz{PART}"));
    let mut encoder = GzEncoder::new(BufWriter::new(File::create(&part)?), Compression::best());
    io::copy(&mut File::open(plain)?, &mut encoder)?;
    encoder
        .finish()?
        .into_inner()
        .map_err(io::IntoInnerError::into_error)?;
    fs::rename(&part, &packed)?;
    fs::remove_file(plain)
}

/// The UTC date of `now` as the appender names its files (YYYY-MM-DD).
fn utc_day(now: SystemTime) -> String {
    // Days to a civil date (Howard Hinnant's `civil_from_days`).
    let days = now
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() / 86_400) as i64;
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Gzips every day's log in `dir` that's over (before `today` and not the newest: the appender
/// may still write it), and removes days beyond the latest `keep`.
pub fn tidy(dir: &Path, keep: usize, today: &str) -> io::Result<()> {
    let mut days: Vec<(String, std::path::PathBuf, bool)> = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if !entry.file_type()?.is_file() {
            continue;
        }
        if name.starts_with(PREFIX) && name.ends_with(PART) {
            let _ = fs::remove_file(entry.path());
            continue;
        }
        if let Some((day, packed)) = day_of(&name) {
            days.push((day.to_owned(), entry.path(), packed));
        }
    }
    // Newest first; a day's plain file before its gzip (left by a stop between the two steps).
    days.sort_by(|a, b| b.0.cmp(&a.0).then(a.2.cmp(&b.2)));
    let newest = days.first().map(|(day, _, _)| day.clone());
    let mut kept = 0;
    let mut last_day: Option<&str> = None;
    for (day, path, packed) in &days {
        if last_day != Some(day.as_str()) {
            last_day = Some(day);
            kept += 1;
        }
        if kept > keep {
            fs::remove_file(path)?;
        } else if !packed && Some(day) != newest.as_ref() && day.as_str() < today {
            pack(path)?;
        }
    }
    Ok(())
}

/// Tidies `dir` now and every few hours after, on a thread of its own.
pub fn keep_tidy(dir: std::path::PathBuf, keep: usize) {
    let started = std::thread::Builder::new()
        .name("log-tidy".into())
        .spawn(move || {
            loop {
                if let Err(error) = tidy(&dir, keep, &utc_day(SystemTime::now())) {
                    tracing::debug!(%error, "couldn't tidy the logs");
                }
                std::thread::sleep(EVERY);
            }
        });
    if let Err(error) = started {
        tracing::debug!(%error, "logs won't be tidied");
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use flate2::read::GzDecoder;
    use tempfile::TempDir;

    use super::*;

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    fn log_text(day: &str) -> String {
        (0..400)
            .map(|line| {
                format!(
                    "{day}T10:{:02}:00.000Z  INFO raygun_app::playback: loaded load_id={line}\n",
                    line % 60
                )
            })
            .collect()
    }

    #[test]
    fn days_that_ended_are_gzipped_and_read_back_whole() {
        let dir = TempDir::new().unwrap();
        for day in ["2026-11-02", "2026-11-03", "2026-11-07"] {
            fs::write(dir.path().join(format!("raygun.{day}.log")), log_text(day)).unwrap();
        }
        fs::write(dir.path().join("notes.txt"), "not a log").unwrap();

        tidy(dir.path(), 7, "2026-11-07").unwrap();
        assert_eq!(
            names(dir.path()),
            [
                "notes.txt",
                "raygun.2026-11-02.log.gz",
                "raygun.2026-11-03.log.gz",
                "raygun.2026-11-07.log"
            ]
        );
        let packed = fs::read(dir.path().join("raygun.2026-11-02.log.gz")).unwrap();
        assert!(packed.len() * 10 < log_text("2026-11-02").len());
        let mut text = String::new();
        GzDecoder::new(packed.as_slice())
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, log_text("2026-11-02"));

        // Again: nothing changes.
        tidy(dir.path(), 7, "2026-11-07").unwrap();
        assert_eq!(names(dir.path()).len(), 4);
    }

    #[test]
    fn todays_file_stays_plain_even_when_a_later_day_shows_up() {
        // The clock was ahead for a while: today's file isn't the newest name.
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("raygun.2026-11-07.log"), "today").unwrap();
        fs::write(dir.path().join("raygun.2026-12-01.log"), "ahead").unwrap();
        tidy(dir.path(), 7, "2026-11-07").unwrap();
        assert_eq!(
            names(dir.path()),
            ["raygun.2026-11-07.log", "raygun.2026-12-01.log"]
        );
    }

    #[test]
    fn days_are_named_as_the_appender_names_them() {
        let at = |secs: u64| utc_day(UNIX_EPOCH + Duration::from_secs(secs));
        assert_eq!(at(0), "1970-01-01");
        assert_eq!(at(951_782_400), "2000-02-29");
        assert_eq!(at(1_700_000_000), "2023-11-14");
        assert_eq!(at(1_762_646_400), "2025-11-09");
    }

    #[test]
    fn only_the_latest_days_are_kept() {
        let dir = TempDir::new().unwrap();
        for day in 1..=9 {
            let name = format!(
                "raygun.2026-11-0{day}.log{}",
                if day < 5 { ".gz" } else { "" }
            );
            fs::write(dir.path().join(name), "x").unwrap();
        }
        tidy(dir.path(), 3, "2026-11-09").unwrap();
        assert_eq!(
            names(dir.path()),
            [
                "raygun.2026-11-07.log.gz",
                "raygun.2026-11-08.log.gz",
                "raygun.2026-11-09.log"
            ]
        );
    }

    #[test]
    fn a_stop_halfway_is_finished_next_time() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("raygun.2026-11-01.log.gz.part"), "half").unwrap();
        // Gzipped, but the plain file wasn't removed yet.
        fs::write(dir.path().join("raygun.2026-11-02.log"), log_text("a")).unwrap();
        fs::write(dir.path().join("raygun.2026-11-02.log.gz"), "old").unwrap();
        fs::write(dir.path().join("raygun.2026-11-03.log"), "today").unwrap();
        tidy(dir.path(), 7, "2026-11-03").unwrap();
        assert_eq!(
            names(dir.path()),
            ["raygun.2026-11-02.log.gz", "raygun.2026-11-03.log"]
        );
        let mut text = String::new();
        GzDecoder::new(File::open(dir.path().join("raygun.2026-11-02.log.gz")).unwrap())
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, log_text("a"));
    }
}
