use super::model::*;

fn song(id: &str, phase: Phase, done: f64) -> Download {
    Download {
        kind: Kind::Songs,
        id: id.into(),
        title: format!("Song {id}"),
        subtitle: Some("Artist".into()),
        phase,
        done,
        total: Some(5_000_000.0),
        unit: Unit::Bytes,
        bytes: None,
        left_ms: None,
        error: None,
        abilities: Abilities {
            pause: false,
            cancel: true,
            retry: phase == Phase::Failed,
        },
        route: Kind::Songs.route().into(),
    }
}

fn chapter(id: &str, phase: Phase, pages: f64) -> Download {
    Download {
        kind: Kind::Chapters,
        id: id.into(),
        title: "Manga".into(),
        subtitle: Some(format!("Chapter {id}")),
        phase,
        done: pages,
        total: Some(20.0),
        unit: Unit::Pages,
        bytes: None,
        left_ms: None,
        error: None,
        abilities: Abilities {
            pause: true,
            cancel: true,
            retry: phase == Phase::Failed,
        },
        route: Kind::Chapters.route().into(),
    }
}

#[test]
fn a_lone_download_is_one_notification_with_its_speed_and_time_left() {
    let mut tracker = Tracker::default();
    tracker.update(0.0, &[song("a", Phase::Running, 1_000_000.0)]);
    let update = tracker.update(2_000.0, &[song("a", Phase::Running, 2_000_000.0)]);
    let summary = update.plan.summary.expect("a notification");
    assert!(update.plan.items.is_empty());
    assert!(update.plan.foreground);
    assert_eq!(summary.title, "Song a");
    assert_eq!(summary.text, "Artist · 2.0 of 5.0 MB");
    // 500 KB/s, 3 MB to go: six seconds, said as about 10 s.
    assert_eq!(summary.info.as_deref(), Some("500 KB/s · about 10 s left"));
    assert_eq!(summary.progress, Some(0.4));
    assert_eq!(summary.actions.len(), 1);
    assert_eq!(summary.actions[0].id, "cancel");
    assert_eq!(summary.actions[0].keys, vec!["songs:a".to_owned()]);
    assert_eq!(summary.route, "/downloads?tab=music");
}

#[test]
fn several_downloads_are_grouped_under_a_summary() {
    let mut tracker = Tracker::default();
    let downloads = [
        song("a", Phase::Running, 1_000_000.0),
        chapter("1", Phase::Running, 5.0),
        chapter("2", Phase::Queued, 0.0),
    ];
    tracker.update(0.0, &downloads);
    let moved = [
        song("a", Phase::Running, 3_000_000.0),
        chapter("1", Phase::Running, 10.0),
        chapter("2", Phase::Queued, 0.0),
    ];
    let update = tracker.update(2_000.0, &moved);
    let summary = update.plan.summary.expect("a summary");
    assert_eq!(summary.title, "2 downloading");
    assert_eq!(summary.text, "1 song, 1 chapter");
    // One waits, so no time for the whole.
    assert_eq!(summary.info.as_deref(), Some("1 waiting"));
    assert_eq!(summary.route, "/downloads");
    assert_eq!(update.plan.items.len(), 2);
    let chapter = &update.plan.items[1];
    assert_eq!(chapter.text, "Chapter 1 · 10 of 20 pages");
    assert_eq!(chapter.info.as_deref(), Some("about 5 s left"));
    let ids: Vec<&str> = chapter.actions.iter().map(|action| action.id).collect();
    assert_eq!(ids, ["pause", "cancel"]);
}

#[test]
fn a_paused_queue_is_one_notification_that_resumes_it() {
    let mut tracker = Tracker::default();
    let update = tracker.update(
        0.0,
        &[
            chapter("1", Phase::Paused, 3.0),
            chapter("2", Phase::Paused, 0.0),
        ],
    );
    let summary = update.plan.summary.expect("a notification");
    assert_eq!(summary.title, "2 chapters paused");
    assert!(!summary.ongoing);
    assert!(!update.plan.foreground);
    assert_eq!(summary.actions[0].id, "resume");
    assert_eq!(summary.actions[0].keys.len(), 2);
}

#[test]
fn the_batch_ends_with_one_notice() {
    let mut tracker = Tracker::default();
    tracker.update(
        0.0,
        &[
            song("a", Phase::Running, 0.0),
            song("b", Phase::Queued, 0.0),
        ],
    );
    let middle = tracker.update(
        1_000.0,
        &[song("a", Phase::Done, 5e6), song("b", Phase::Running, 0.0)],
    );
    assert!(middle.finished.is_none());
    let end = tracker.update(
        2_000.0,
        &[song("a", Phase::Done, 5e6), song("b", Phase::Done, 5e6)],
    );
    assert_eq!(end.plan, Plan::default());
    let finished = end.finished.expect("a notice");
    assert_eq!(finished.title, "2 downloads finished");
    assert_eq!(finished.text, "2 songs");
    assert!(!finished.failed);
    assert_eq!(finished.route, "/downloads?tab=music");
    // Already said: the next update has nothing to add.
    let after = tracker.update(3_000.0, &[song("a", Phase::Done, 5e6)]);
    assert!(after.finished.is_none());
}

#[test]
fn failures_are_told_with_a_retry() {
    let mut tracker = Tracker::default();
    tracker.update(0.0, &[song("a", Phase::Running, 0.0)]);
    let mut failed = song("a", Phase::Failed, 0.0);
    failed.error = Some("Couldn't reach the source".into());
    let end = tracker.update(1_000.0, &[failed]);
    let finished = end.finished.expect("a notice");
    assert_eq!(finished.title, "Song a didn't download");
    assert_eq!(finished.text, "Couldn't reach the source");
    assert!(finished.failed);
    assert_eq!(finished.actions[0].id, "retry");
    assert_eq!(finished.actions[0].keys, vec!["songs:a".to_owned()]);
}

#[test]
fn a_removed_download_is_no_news() {
    let mut tracker = Tracker::default();
    tracker.update(0.0, &[song("a", Phase::Running, 0.0)]);
    let end = tracker.update(1_000.0, &[]);
    assert!(end.finished.is_none());
    assert_eq!(end.plan, Plan::default());
}

#[test]
fn done_chapters_reported_once_still_count_at_the_end() {
    let mut tracker = Tracker::default();
    tracker.update(
        0.0,
        &[
            chapter("1", Phase::Running, 0.0),
            chapter("2", Phase::Queued, 0.0),
        ],
    );
    // Chapter 1 finished and left the queue: it's reported done once.
    tracker.update(
        1_000.0,
        &[
            chapter("1", Phase::Done, 20.0),
            chapter("2", Phase::Running, 2.0),
        ],
    );
    tracker.update(2_000.0, &[chapter("2", Phase::Running, 10.0)]);
    let end = tracker.update(3_000.0, &[chapter("2", Phase::Done, 20.0)]);
    let finished = end.finished.expect("a notice");
    assert_eq!(finished.title, "2 downloads finished");
    assert_eq!(finished.text, "2 chapters");
}

#[test]
fn mixed_results_say_what_didnt_download() {
    let mut tracker = Tracker::default();
    tracker.update(
        0.0,
        &[
            song("a", Phase::Running, 0.0),
            song("b", Phase::Queued, 0.0),
        ],
    );
    let end = tracker.update(
        1_000.0,
        &[song("a", Phase::Done, 5e6), song("b", Phase::Failed, 0.0)],
    );
    let finished = end.finished.expect("a notice");
    assert_eq!(finished.title, "1 downloaded, 1 didn't");
    assert_eq!(finished.text, "Not downloaded: Song b");
}

#[test]
fn a_recording_counts_down_with_its_song() {
    let mut tracker = Tracker::default();
    let mut recording = song("a", Phase::Recording, 0.0);
    recording.left_ms = Some(151_000.0);
    let update = tracker.update(0.0, &[recording]);
    let summary = update.plan.summary.expect("a notification");
    assert_eq!(summary.title, "Recording Song a");
    assert_eq!(summary.info.as_deref(), Some("2:31 left"));
    assert_eq!(summary.actions[0].id, "stop");
}

#[test]
fn downloads_that_start_later_are_told_apart_from_the_first_ones() {
    let mut tracker = Tracker::default();
    // Running at the first update: carried over from the last run.
    let first = tracker.update(0.0, &[song("a", Phase::Running, 0.0)]);
    assert!(first.started.is_empty());
    let later = tracker.update(
        1_000.0,
        &[
            song("a", Phase::Running, 10.0),
            song("b", Phase::Queued, 0.0),
        ],
    );
    assert_eq!(later.started, vec!["songs:b".to_owned()]);
}

#[test]
fn a_stalled_download_stops_keeping_raygun_running() {
    let mut tracker = Tracker::default();
    tracker.update(0.0, &[song("a", Phase::Running, 100.0)]);
    let still = tracker.update(5_000.0, &[song("a", Phase::Running, 100.0)]);
    assert!(still.plan.foreground);
    let stalled = tracker.update(11.0 * 60.0 * 1000.0, &[song("a", Phase::Running, 100.0)]);
    assert!(!stalled.plan.foreground);
    assert!(stalled.plan.summary.is_some());
}

#[test]
fn only_waiting_downloads_show_as_waiting() {
    let mut tracker = Tracker::default();
    let update = tracker.update(
        0.0,
        &[song("a", Phase::Queued, 0.0), song("b", Phase::Queued, 0.0)],
    );
    let summary = update.plan.summary.expect("a notification");
    assert_eq!(summary.title, "2 waiting to download");
    assert!(update.plan.foreground);
}

#[test]
fn sizes_speeds_and_times_read_like_androids() {
    assert_eq!(size_text(820.0), "820 B");
    assert_eq!(size_text(820_000.0), "820 KB");
    assert_eq!(size_text(4_200_000.0), "4.2 MB");
    assert_eq!(size_text(1_300_000_000.0), "1.3 GB");
    assert_eq!(speed_text(1_200_000.0), "1.2 MB/s");
    assert_eq!(left_text(3_000.0), "about 5 s left");
    assert_eq!(left_text(31_000.0), "about 35 s left");
    assert_eq!(left_text(90_000.0), "about 2 min left");
    assert_eq!(left_text(3_600_000.0), "about 1 h left");
    assert_eq!(left_text(3_900_000.0), "about 1 h 5 min left");
    assert_eq!(clock(151_000.0), "2:31");
    assert_eq!(clock(3_725_000.0), "1:02:05");
}

#[test]
fn keys_name_their_kind() {
    assert_eq!(key(Kind::Videos, "x"), "videos:x");
    for kind in [
        Kind::Songs,
        Kind::Videos,
        Kind::Chapters,
        Kind::Podcasts,
        Kind::Exports,
        Kind::Models,
        Kind::Update,
        Kind::YtDlp,
    ] {
        assert_eq!(Kind::from_tag(kind.tag()), Some(kind));
    }
    assert_eq!(Kind::from_tag("other"), None);
}
