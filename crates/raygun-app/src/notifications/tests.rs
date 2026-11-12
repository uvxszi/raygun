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
        speed: None,
        error: None,
        abilities: Abilities {
            pause: false,
            cancel: true,
            retry: phase == Phase::Failed,
            ..Abilities::default()
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
        speed: None,
        error: None,
        abilities: Abilities {
            pause: true,
            cancel: true,
            retry: phase == Phase::Failed,
            ..Abilities::default()
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
    // A batch of several has no one file to open.
    assert_eq!(finished.lone, None);
}

#[test]
fn a_lone_finished_download_is_named_for_its_open_button() {
    let file = |phase| Download {
        kind: Kind::Files,
        title: "manual.pdf".into(),
        subtitle: Some("example.org".into()),
        route: Kind::Files.route().into(),
        ..song("f1", phase, 0.0)
    };
    let mut tracker = Tracker::default();
    tracker.update(0.0, &[file(Phase::Running)]);
    let end = tracker.update(1_000.0, &[file(Phase::Done)]);
    let finished = end.finished.expect("a notice");
    assert_eq!(finished.title, "manual.pdf");
    assert_eq!(finished.lone.as_deref(), Some("files:f1"));
    // Only the platform adds what it opens; the key itself isn't sent.
    assert!(
        serde_json::to_value(&finished)
            .unwrap()
            .get("lone")
            .is_none()
    );

    // One that failed has nothing to open.
    let mut tracker = Tracker::default();
    tracker.update(0.0, &[file(Phase::Running)]);
    let end = tracker.update(1_000.0, &[file(Phase::Failed)]);
    assert_eq!(end.finished.expect("a notice").lone, None);
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
fn a_capture_shows_like_any_download() {
    let mut tracker = Tracker::default();
    let mut capture = song("a", Phase::Running, 0.0);
    (capture.unit, capture.done, capture.total) = (Unit::Fraction, 0.4, Some(1.0));
    let update = tracker.update(0.0, &[capture]);
    let summary = update.plan.summary.expect("a notification");
    assert_eq!(summary.title, "Song a");
    assert_eq!(summary.progress, Some(0.4));
    assert_eq!(summary.actions[0].id, "cancel");
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

fn song_download(state: raygun_model::SongDownloadState) -> raygun_model::SongDownload {
    raygun_model::SongDownload {
        id: "s".into(),
        track_key: "bts|swim".into(),
        title: "Swim".into(),
        artist: Some("BTS".into()),
        album: None,
        duration_ms: Some(180_000),
        artwork_url: None,
        state,
        kind: None,
        provider: None,
        format: None,
        done_bytes: 0.0,
        total_bytes: None,
        fraction: None,
        bytes_per_second: None,
        eta_ms: None,
        error: None,
        error_detail: None,
        replacing: false,
        problem: None,
        unlisted: false,
        queued_at: 1.0,
        finished_at: None,
    }
}

#[test]
fn a_song_tapped_to_download_ends_with_its_notice_and_nothing_left_loading() {
    use raygun_model::SongDownloadState as State;

    let mut tracker = Tracker::default();
    // Queued, then looking for a source: the bar can't say how far yet.
    let queued = super::songs(vec![song_download(State::Queued)], false);
    let first = tracker.update(0.0, &queued);
    assert_eq!(first.plan.summary.unwrap().title, "1 waiting to download");
    let finding = super::songs(vec![song_download(State::Downloading)], false);
    let update = tracker.update(1_000.0, &finding);
    let summary = update.plan.summary.expect("a notification");
    assert_eq!(summary.title, "Swim");
    assert_eq!(summary.text, "BTS · Finding a source");
    assert_eq!(summary.progress, None);
    assert!(summary.ongoing && update.plan.foreground);
    let ids: Vec<&str> = summary.actions.iter().map(|action| action.id).collect();
    assert_eq!(ids, ["pause", "cancel"]);

    // From YouTube, then its bytes.
    let mut from = song_download(State::Downloading);
    from.provider = Some("YouTube".into());
    let update = tracker.update(2_000.0, &super::songs(vec![from.clone()], false));
    assert_eq!(update.plan.summary.unwrap().text, "BTS · From YouTube");
    (from.done_bytes, from.total_bytes) = (1_000_000.0, Some(4_000_000.0));
    let update = tracker.update(3_000.0, &super::songs(vec![from], false));
    let summary = update.plan.summary.unwrap();
    assert_eq!(summary.text, "BTS · 1.0 of 4.0 MB");
    assert_eq!(summary.progress, Some(0.25));

    // It fails: the running notification goes, the notice says why, nothing keeps running.
    let mut failed = song_download(State::Failed);
    failed.error = Some("No source has this song.".into());
    let end = tracker.update(4_000.0, &super::songs(vec![failed.clone()], false));
    assert_eq!(end.plan, Plan::default(), "nothing is left loading");
    assert!(!end.plan.foreground);
    let notice = end.finished.expect("a notice");
    assert_eq!(notice.title, "Swim didn't download");
    assert_eq!(notice.text, "No source has this song.");
    assert_eq!(notice.actions[0].id, "retry");
    // The next turns have nothing more to show.
    let after = tracker.update(5_000.0, &super::songs(vec![failed], false));
    assert_eq!(after.plan, Plan::default());
    assert!(after.finished.is_none());
}

#[test]
fn late_progress_never_brings_an_ended_or_removed_song_back() {
    use super::Report;
    use raygun_model::SongDownloadState as State;

    // Not in the last list: removed (its worker's last report) or new; the list decides.
    assert_eq!(
        super::takes_report(None, State::Downloading),
        Report::ReadList
    );
    assert_eq!(
        super::takes_report(Some(State::Queued), State::Downloading),
        Report::Take
    );
    assert_eq!(
        super::takes_report(Some(State::Downloading), State::Done),
        Report::Take
    );
    assert_eq!(
        super::takes_report(Some(State::Failed), State::Downloading),
        Report::Drop,
        "a report sent before it failed"
    );
    assert_eq!(
        super::takes_report(Some(State::Done), State::Downloading),
        Report::Drop
    );
    assert_eq!(
        super::takes_report(Some(State::Failed), State::Done),
        Report::Take
    );
}

#[test]
fn cancelling_one_of_several_leaves_only_the_others() {
    let mut tracker = Tracker::default();
    tracker.update(
        0.0,
        &[
            song("a", Phase::Running, 1_000.0),
            song("b", Phase::Running, 1_000.0),
        ],
    );
    // "a" is cancelled: it leaves its service's list.
    let after = tracker.update(1_000.0, &[song("b", Phase::Running, 2_000.0)]);
    let summary = after.plan.summary.expect("b still shows");
    assert_eq!(summary.title, "Song b");
    assert!(after.plan.items.is_empty(), "a's own notification is gone");
    // "b" finishes: the notice is about b alone.
    let end = tracker.update(2_000.0, &[song("b", Phase::Done, 5_000_000.0)]);
    assert_eq!(end.plan, Plan::default());
    let notice = end.finished.expect("a notice");
    assert_eq!(notice.title, "Song b");
}

#[test]
fn cancelling_the_last_one_clears_everything_without_a_notice() {
    let mut tracker = Tracker::default();
    tracker.update(0.0, &[song("a", Phase::Running, 1_000.0)]);
    tracker.update(1_000.0, &[song("a", Phase::Paused, 1_000.0)]);
    let end = tracker.update(2_000.0, &[]);
    assert_eq!(end.plan, Plan::default());
    assert!(!end.plan.foreground);
    assert!(end.finished.is_none());
}

#[test]
fn the_first_turn_always_goes_out_so_a_killed_run_leaves_nothing() {
    // After a restart nothing downloads: the empty plan is still sent, which clears what the
    // last process left in the shade.
    let mut tracker = Tracker::default();
    let first = tracker.update(0.0, &[]);
    assert_eq!(first.plan, Plan::default());
    assert!(should_send(None, 0.0, &first));
    let next = tracker.update(1_000.0, &[]);
    assert!(!should_send(Some((0.0, &first.plan)), 1_000.0, &next));
}

#[test]
fn a_plan_that_stays_the_same_is_sent_again_now_and_then() {
    let mut tracker = Tracker::default();
    let first = tracker.update(0.0, &[song("a", Phase::Paused, 0.0)]);
    let plan = first.plan.clone();
    let soon = tracker.update(5_000.0, &[song("a", Phase::Paused, 0.0)]);
    assert_eq!(soon.plan, plan);
    assert!(!should_send(Some((0.0, &plan)), 5_000.0, &soon));
    let later = tracker.update(RESEND_MS, &[song("a", Phase::Paused, 0.0)]);
    assert!(should_send(Some((0.0, &plan)), RESEND_MS, &later));
}

#[test]
fn a_queue_that_never_starts_lets_go_of_the_foreground() {
    let mut tracker = Tracker::default();
    let first = tracker.update(0.0, &[song("a", Phase::Queued, 0.0)]);
    assert!(first.plan.foreground);
    assert!(first.plan.summary.as_ref().is_some_and(|s| s.ongoing));
    let stuck = tracker.update(6.0 * 60.0 * 1000.0, &[song("a", Phase::Queued, 0.0)]);
    assert!(!stuck.plan.foreground);
    let summary = stuck.plan.summary.expect("still told");
    assert!(!summary.ongoing, "it can be swiped away");
    // It starts after all: back in the foreground.
    let moving = tracker.update(
        6.0 * 60.0 * 1000.0 + 1_000.0,
        &[song("a", Phase::Running, 10.0)],
    );
    assert!(moving.plan.foreground);
}

#[test]
fn a_paused_song_queue_shows_paused_instead_of_waiting_for_good() {
    use raygun_model::SongDownloadState as State;

    let mut tracker = Tracker::default();
    let list = super::songs(
        vec![song_download(State::Queued), {
            let mut other = song_download(State::Queued);
            other.id = "t".into();
            other
        }],
        true,
    );
    let update = tracker.update(0.0, &list);
    let summary = update.plan.summary.expect("a notification");
    assert_eq!(summary.title, "2 songs paused");
    assert!(!summary.ongoing, "no bar, and it can be swiped away");
    assert!(!update.plan.foreground);
    assert_eq!(summary.actions[0].id, "resume");
    // The paused songs can all be called off from there.
    assert_eq!(summary.actions[1].id, "cancelAll");
    assert_eq!(summary.actions[1].keys, ["songs:s", "songs:t"]);
}

fn song_numbered(id: &str, state: raygun_model::SongDownloadState) -> raygun_model::SongDownload {
    raygun_model::SongDownload {
        id: id.into(),
        ..song_download(state)
    }
}

#[test]
fn songs_with_others_waiting_are_skipped_or_all_cancelled_from_their_notification() {
    use raygun_model::SongDownloadState as State;

    let mut tracker = Tracker::default();
    let list = super::songs(
        vec![
            song_numbered("a", State::Downloading),
            song_numbered("b", State::Queued),
            song_numbered("c", State::Queued),
        ],
        false,
    );
    let update = tracker.update(0.0, &list);
    let summary = update.plan.summary.expect("a notification");
    let ids: Vec<&str> = summary.actions.iter().map(|action| action.id).collect();
    assert_eq!(ids, ["pause", "skip", "cancelAll"], "three at most");
    assert_eq!(summary.actions[1].label, "Skip");
    assert_eq!(
        summary.actions[1].keys,
        ["songs:a"],
        "Skip is the one downloading"
    );
    assert_eq!(summary.actions[2].label, "Cancel all");
    let mut all = summary.actions[2].keys.clone();
    all.sort();
    assert_eq!(
        all,
        ["songs:a", "songs:b", "songs:c"],
        "Cancel all is every one"
    );

    // The last one left has a Cancel of its own.
    let update = tracker.update(
        1_000.0,
        &super::songs(vec![song_numbered("c", State::Downloading)], false),
    );
    let ids: Vec<String> = update
        .plan
        .summary
        .expect("a notification")
        .actions
        .iter()
        .map(|action| action.id.to_owned())
        .collect();
    assert_eq!(ids, ["pause", "cancel"]);
}

#[test]
fn a_recording_shows_how_far_it_is_and_the_time_it_takes_with_stop() {
    use raygun_model::SongDownloadState as State;

    let mut tracker = Tracker::default();
    let mut recording = song_numbered("r", State::Downloading);
    recording.kind = Some(raygun_model::SongDownloadKind::Recording);
    recording.provider = Some("YouTube".into());
    recording.fraction = Some(0.25);
    let file = song_numbered("f", State::Downloading);
    let update = tracker.update(0.0, &super::songs(vec![recording.clone(), file], false));
    let shown = update
        .plan
        .items
        .iter()
        .find(|shown| shown.key == "songs:r")
        .expect("the recording's own notification");
    assert_eq!(shown.title, "Swim");
    assert_eq!(shown.text, "Recording · BTS · 25%");
    // A recording runs as the song plays: three quarters of 3:00 to go.
    assert_eq!(shown.info.as_deref(), Some("about 3 min left"));
    assert_eq!(shown.progress, Some(0.25));
    let actions: Vec<(&str, &str)> = shown
        .actions
        .iter()
        .map(|action| (action.id, action.label))
        .collect();
    assert_eq!(actions, [("stop", "Stop")]);
    assert_eq!(shown.actions[0].keys, ["songs:r"]);
    assert!(update.plan.foreground, "it keeps Raygun running");

    // Alone, it's the notification itself.
    recording.fraction = Some(0.5);
    let update = tracker.update(1_000.0, &super::songs(vec![recording], false));
    let summary = update.plan.summary.expect("a notification");
    assert_eq!(summary.text, "Recording · BTS · 50%");
    assert_eq!(summary.info.as_deref(), Some("about 2 min left"));
}

#[test]
fn downloads_held_by_battery_saver_say_so_without_a_bar() {
    let mut tracker = Tracker::default();
    let downloads: Vec<Download> = [
        song("a", Phase::Queued, 0.0),
        chapter("1", Phase::Queued, 0.0),
    ]
    .into_iter()
    .map(|download| super::hold(download, true))
    .collect();
    let update = tracker.update(0.0, &downloads);
    let summary = update.plan.summary.expect("a notification");
    assert_eq!(summary.title, "Waiting for battery saver to end");
    assert_eq!(summary.text, "1 song, 1 chapter");
    assert_eq!(summary.progress, None);
    assert!(!summary.ongoing);
    assert!(!update.plan.foreground, "nothing to keep running for");
    // A running one carries on; the held ones show beside it.
    let downloads: Vec<Download> = [
        song("a", Phase::Running, 10.0),
        song("b", Phase::Queued, 0.0),
    ]
    .into_iter()
    .map(|download| super::hold(download, true))
    .collect();
    let update = tracker.update(1_000.0, &downloads);
    assert_eq!(update.plan.summary.unwrap().title, "1 downloading");
    assert_eq!(
        update.plan.items[1].title,
        "Waiting for battery saver to end"
    );
    // Battery saver off: they wait as usual.
    let free = super::hold(song("b", Phase::Queued, 0.0), false);
    assert_eq!(free.phase, Phase::Queued);
    // Kinds that don't wait in battery saver aren't held.
    let mut model = song("m", Phase::Queued, 0.0);
    model.kind = Kind::Models;
    assert_eq!(super::hold(model, true).phase, Phase::Queued);
}

#[test]
fn queues_paused_and_held_together_arent_zero_downloading() {
    let mut tracker = Tracker::default();
    let downloads = [
        chapter("1", Phase::Paused, 3.0),
        super::hold(song("a", Phase::Queued, 0.0), true),
    ];
    let update = tracker.update(0.0, &downloads);
    let summary = update.plan.summary.expect("a summary");
    assert_eq!(summary.title, "2 downloads waiting");
    assert!(!summary.ongoing);
    assert_eq!(update.plan.items.len(), 2);
    assert!(!update.plan.foreground);
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
        Kind::Files,
    ] {
        assert_eq!(Kind::from_tag(kind.tag()), Some(kind));
    }
    assert_eq!(Kind::from_tag("other"), None);
}

fn install(id: &str, state: raygun_model::ExtensionInstallState) -> raygun_model::ExtensionInstall {
    raygun_model::ExtensionInstall {
        repo_id: "repo".into(),
        extension_id: id.into(),
        name: format!("Ext {id}"),
        state,
        phase: None,
        error: (state == raygun_model::ExtensionInstallState::Failed)
            .then(|| "the repository no longer lists it".into()),
        finished_at: None,
        batch: 1,
    }
}

fn installs(
    states: &[raygun_model::ExtensionInstallState],
    running: bool,
) -> raygun_model::ExtensionInstallStatus {
    use raygun_model::ExtensionInstallState as State;
    let items: Vec<_> = states
        .iter()
        .enumerate()
        .map(|(index, state)| install(&index.to_string(), *state))
        .collect();
    let count = |state: State| states.iter().filter(|s| **s == state).count() as u32;
    raygun_model::ExtensionInstallStatus {
        batch: 1,
        total: states.len() as u32,
        done: count(State::Done),
        failed: count(State::Failed),
        cancelled: 0,
        running,
        current: items
            .iter()
            .find(|item| item.state == State::Installing)
            .map(|item| item.name.clone()),
        phase: running.then_some(raygun_model::ExtensionInstallPhase::Converting),
        items,
    }
}

#[test]
fn extension_installs_show_their_place_in_the_batch_with_a_cancel() {
    use raygun_model::ExtensionInstallState as State;
    let mut tracker = Tracker::default();
    let status = installs(
        &[State::Done, State::Failed, State::Installing, State::Queued],
        true,
    );
    let update = tracker.update(0.0, &[extension_installs(&status).unwrap()]);
    let summary = update.plan.summary.expect("a notification");
    assert_eq!(summary.title, "Installing extensions · 3 of 4");
    assert_eq!(summary.text, "Ext 2 · Converting");
    assert_eq!(summary.progress, Some(0.5));
    assert!(update.plan.foreground);
    assert_eq!(summary.actions.len(), 1);
    assert_eq!(summary.actions[0].id, "cancel");
    assert_eq!(summary.actions[0].keys, ["extensions:batch-1"]);

    // The last one: nothing left to cancel.
    let status = installs(
        &[State::Done, State::Failed, State::Done, State::Installing],
        true,
    );
    let update = tracker.update(1_000.0, &[extension_installs(&status).unwrap()]);
    assert!(update.plan.summary.unwrap().actions.is_empty());

    let status = installs(
        &[State::Done, State::Failed, State::Done, State::Done],
        false,
    );
    let update = tracker.update(2_000.0, &[extension_installs(&status).unwrap()]);
    assert!(update.plan.summary.is_none());
    let finished = update.finished.expect("a summary");
    assert_eq!(finished.title, "Extensions installed");
    assert_eq!(finished.text, "3 installed · 1 failed");
    assert!(finished.failed);
    assert_eq!(finished.actions[0].id, "retry");
    assert_eq!(finished.actions[0].keys, ["extensions:batch-1"]);
}

#[test]
fn a_lone_extension_install_ends_with_its_name() {
    use raygun_model::ExtensionInstallState as State;
    let mut tracker = Tracker::default();
    let running = installs(&[State::Installing], true);
    tracker.update(0.0, &[extension_installs(&running).unwrap()]);
    let failed = installs(&[State::Failed], false);
    let finished = tracker
        .update(1_000.0, &[extension_installs(&failed).unwrap()])
        .finished
        .unwrap();
    assert_eq!(finished.title, "Ext 0 didn't install");
    assert_eq!(finished.text, "the repository no longer lists it");

    // A batch cancelled before anything ended says nothing.
    let mut status = installs(&[], false);
    status.cancelled = 3;
    assert!(extension_installs(&status).is_none());
}

#[test]
fn extension_installs_beside_downloads_add_their_summary() {
    use raygun_model::ExtensionInstallState as State;
    let mut tracker = Tracker::default();
    let running = installs(&[State::Installing, State::Queued], true);
    let update = tracker.update(
        0.0,
        &[
            extension_installs(&running).unwrap(),
            song("a", Phase::Running, 0.0),
        ],
    );
    assert_eq!(update.plan.items.len(), 2);
    let ended = installs(&[State::Done, State::Done], false);
    let finished = tracker
        .update(
            1_000.0,
            &[
                extension_installs(&ended).unwrap(),
                song("a", Phase::Done, 5_000_000.0),
            ],
        )
        .finished
        .unwrap();
    assert_eq!(finished.title, "Song a");
    assert_eq!(finished.text, "Downloaded · 2 installed");
}

/// A list of sources being added, its items in these states.
fn source_list(states: &[raygun_model::SourceQueueState]) -> raygun_model::SourceQueueStatus {
    use raygun_model::SourceQueueState as State;
    let items: Vec<raygun_model::SourceQueueItem> = states
        .iter()
        .enumerate()
        .map(|(index, state)| raygun_model::SourceQueueItem {
            id: index as u32 + 1,
            address: format!("s{index}.example"),
            state: *state,
            kind: None,
            format: None,
            server: None,
            name: (*state != State::Waiting).then(|| format!("Source {index}")),
            url: None,
            configure_url: None,
            error: (*state == State::Failed)
                .then(|| "Couldn't reach s.example\nconnection refused".to_owned()),
            batch: 1,
        })
        .collect();
    let count = |state: State| states.iter().filter(|s| **s == state).count() as u32;
    let running = states
        .iter()
        .any(|state| matches!(state, State::Waiting | State::Checking | State::Adding));
    raygun_model::SourceQueueStatus {
        batch: 1,
        total: states.len() as u32,
        added: count(State::Added),
        already_added: count(State::AlreadyAdded),
        needs_sign_in: count(State::NeedsSignIn),
        needs_setup: count(State::NeedsSetup),
        failed: count(State::Failed),
        cancelled: count(State::Cancelled),
        running,
        paused: false,
        loading: false,
        current: items
            .iter()
            .find(|item| matches!(item.state, State::Checking | State::Adding))
            .and_then(|item| item.name.clone()),
        items,
    }
}

#[test]
fn a_list_of_sources_shows_its_place_then_its_summary() {
    use raygun_model::SourceQueueState as State;
    let mut tracker = Tracker::default();
    let status = source_list(&[State::Added, State::Checking, State::Waiting]);
    let update = tracker.update(0.0, &[source_queue(&status).unwrap()]);
    let summary = update.plan.summary.expect("a notification");
    assert_eq!(summary.title, "Adding sources · 2 of 3");
    assert_eq!(summary.text, "Source 1 · Checking");
    assert!(update.plan.foreground);
    assert_eq!(summary.actions[0].id, "cancel");
    assert_eq!(summary.actions[0].keys, ["sources:list-1"]);

    let status = source_list(&[State::Added, State::Failed, State::NeedsSignIn]);
    let update = tracker.update(1_000.0, &[source_queue(&status).unwrap()]);
    assert!(update.plan.summary.is_none());
    let finished = update.finished.expect("a summary");
    assert_eq!(finished.title, "Sources added");
    assert_eq!(finished.text, "1 added · 1 to sign in to · 1 failed");
    assert_eq!(finished.route, "/sources");
    assert_eq!(finished.actions[0].id, "retry");
    assert_eq!(finished.actions[0].keys, ["sources:list-1"]);

    // One address: its name, and the issue's sentence.
    let lone = source_queue(&source_list(&[State::Failed])).unwrap();
    assert_eq!(lone.title, "Source 0 wasn't added");
    assert_eq!(lone.subtitle.as_deref(), Some("Couldn't reach s.example"));
    // Cancelled before anything ended: nothing to say.
    assert!(source_queue(&source_list(&[State::Cancelled, State::Cancelled])).is_none());
}

/// What a desktop notification reports when clicked: its action and the notification it showed.
fn clicked(action: &str, extra: serde_json::Value) -> tauri_plugin_notification::ActionPerformed {
    serde_json::from_value(serde_json::json!({
        "actionId": action,
        "inputValue": null,
        "notification": { "id": 7, "title": "New from Nina Simone", "extra": extra },
    }))
    .unwrap()
}

#[test]
fn a_click_on_a_notification_opens_the_route_it_carries() {
    let route = "/artist?name=Nina+Simone\n/music/album?provider=dz&id=1";
    assert_eq!(
        super::tapped_route(&clicked("tap", serde_json::json!({ "route": route }))).as_deref(),
        Some(route)
    );
    // Its buttons, and notifications without a route, open nothing.
    assert_eq!(
        super::tapped_route(&clicked("cancel", serde_json::json!({ "route": route }))),
        None
    );
    assert_eq!(
        super::tapped_route(&clicked("tap", serde_json::json!({}))),
        None
    );
    assert_eq!(
        super::tapped_route(&clicked("tap", serde_json::json!({ "route": "" }))),
        None
    );
}

/// Every line of every notification in `update` says something.
fn assert_nothing_blank(update: &Update) {
    for shown in update.plan.summary.iter().chain(update.plan.items.iter()) {
        assert!(!shown.title.trim().is_empty(), "a title in {shown:?}");
        assert!(!shown.text.trim().is_empty(), "a text in {shown:?}");
    }
    if let Some(finished) = &update.finished {
        assert!(!finished.title.trim().is_empty(), "{finished:?}");
        assert!(!finished.text.trim().is_empty(), "{finished:?}");
    }
}

#[test]
fn downloads_without_a_title_or_progress_still_say_what_they_are() {
    let untitled = |id: &str, phase: Phase| Download {
        title: " ".into(),
        subtitle: None,
        total: None,
        ..song(id, phase, 0.0)
    };
    let mut tracker = Tracker::default();
    let update = tracker.update(0.0, &[untitled("a", Phase::Running)]);
    assert_nothing_blank(&update);
    let summary = update.plan.summary.unwrap();
    assert_eq!(summary.title, "Song");
    assert_eq!(summary.text, "Downloading");

    // Several, one paused, one held by battery saver.
    let update = tracker.update(
        1_000.0,
        &[
            untitled("a", Phase::Running),
            untitled("b", Phase::Running),
            Download {
                title: String::new(),
                ..chapter("1", Phase::Paused, 0.0)
            },
            untitled("c", Phase::Held),
        ],
    );
    assert_nothing_blank(&update);
    assert!(update.plan.items.iter().any(|item| item.text == "Chapter"));

    // The batch ends: one saved, one failed without a reason.
    let update = tracker.update(
        2_000.0,
        &[untitled("a", Phase::Done), untitled("b", Phase::Failed)],
    );
    assert_nothing_blank(&update);
    let finished = update.finished.unwrap();
    assert_eq!(finished.title, "1 downloaded, 1 didn't");
    assert_eq!(finished.text, "Not downloaded: Song");
}

#[test]
fn a_lone_extension_install_that_worked_says_so() {
    use raygun_model::ExtensionInstallState as State;
    let mut tracker = Tracker::default();
    tracker.update(
        0.0,
        &[extension_installs(&installs(&[State::Installing], true)).unwrap()],
    );
    let update = tracker.update(
        1_000.0,
        &[extension_installs(&installs(&[State::Done], false)).unwrap()],
    );
    assert_nothing_blank(&update);
    let finished = update.finished.unwrap();
    assert_eq!(finished.title, "Ext 0 installed");
    assert_eq!(finished.text, "Ready to use");
}

#[test]
fn notices_say_something_on_both_lines_or_arent_shown() {
    let notice = |title: &str, text: &str| super::Notice {
        id: 1,
        channel: &super::RELEASES,
        title: title.into(),
        text: text.into(),
        route: "/".into(),
    };
    let filled = notice("New from Nina Simone", " ").filled().unwrap();
    assert_eq!(filled.title, "New from Nina Simone");
    assert_eq!(filled.text, "Releases from artists you follow");
    let filled = notice("", "Little Girl Blue").filled().unwrap();
    assert_eq!(filled.title, "New releases");
    assert_eq!(filled.text, "Little Girl Blue");
    assert!(notice(" ", "").filled().is_none());
}

#[test]
fn notices_tell_the_plugin_their_channel_and_route() {
    let notice = super::Notice {
        id: super::notice_id("dz/1"),
        channel: &super::RELEASES,
        title: "New from Nina Simone".into(),
        text: "Little Girl Blue".into(),
        route: "/artist?name=Nina+Simone".into(),
    };
    let json = notice.to_json();
    assert_eq!(json["channel"], "raygun.releases");
    assert_eq!(json["channelName"], "New releases");
    assert_eq!(json["title"], "New from Nina Simone");
    assert_eq!(json["text"], "Little Girl Blue");
    assert_eq!(json["route"], "/artist?name=Nina+Simone");
    assert_eq!(json["id"], super::notice_id("dz/1"));
}

#[test]
fn a_model_downloading_can_be_cancelled_from_its_notification() {
    use raygun_model::{AiModelProgress, AiModelState};
    let progress = |state: AiModelState| AiModelProgress {
        id: "whisper-tiny".into(),
        state,
        received_bytes: 50_000_000.0,
        total_bytes: 150_000_000.0,
        error: (state == AiModelState::Failed).then(|| "HTTP 503".to_owned()),
    };
    let name = || "Speech to text (Whisper tiny)".to_owned();
    let running = super::model_download(&progress(AiModelState::Downloading), name()).unwrap();
    assert!(running.abilities.cancel && !running.abilities.retry);
    let mut tracker = Tracker::default();
    let update = tracker.update(0.0, std::slice::from_ref(&running));
    let summary = update.plan.summary.expect("a notification");
    let actions: Vec<(&str, Vec<String>)> = summary
        .actions
        .iter()
        .map(|action| (action.id, action.keys.clone()))
        .collect();
    assert_eq!(
        actions,
        [("cancel", vec!["models:whisper-tiny".to_owned()])]
    );

    // Cancelled: it goes, with no notice that it didn't download.
    assert!(super::model_download(&progress(AiModelState::Cancelled), name()).is_none());
    let update = tracker.update(1_000.0, &[]);
    assert!(update.plan.summary.is_none());
    assert!(update.finished.is_none());

    let failed = super::model_download(&progress(AiModelState::Failed), name()).unwrap();
    assert!(failed.abilities.retry && !failed.abilities.cancel);
}

// ---- Channels ----

#[test]
fn only_news_channels_alert() {
    for channel in super::CHANNELS {
        let alerts = channel.id != super::PLAYBACK.id;
        assert_eq!(channel.alerts, alerts, "{}", channel.id);
    }
    const { assert!(super::RELEASES.alerts) };
    const { assert!(!super::PLAYBACK.alerts) };
}

#[test]
fn quiet_channels_never_reuse_a_retired_id() {
    for channel in super::CHANNELS {
        assert!(
            !super::RETIRED_CHANNELS.contains(&channel.id),
            "{}",
            channel.id
        );
    }
    // The plugin's downloads channel was made quiet from the start.
    assert!(!super::RETIRED_CHANNELS.contains(&"raygun.downloads"));
}

#[test]
fn notices_tell_the_plugin_whether_they_alert() {
    let notice = |channel| super::Notice {
        id: 1,
        channel,
        title: "Title".into(),
        text: "Text".into(),
        route: "/".into(),
    };
    let quiet = notice(&super::PLAYBACK).to_json();
    assert_eq!(quiet["alerts"], false);
    assert_eq!(quiet["channel"], "raygun.playback.quiet");
    assert_eq!(quiet["retiredChannels"][0], "raygun.playback");
    assert_eq!(notice(&super::RELEASES).to_json()["alerts"], true);
}
