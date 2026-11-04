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
