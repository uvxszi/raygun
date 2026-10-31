use std::collections::BTreeMap;

use raygun_model::{MediaKind, MediaSource};

use super::*;

fn input(title: &str) -> QueueItemInput {
    QueueItemInput {
        title: title.into(),
        artist: None,
        album: None,
        duration_ms: Some(180_000),
        artwork_url: None,
        media_kind: MediaKind::Audio,
        source: MediaSource::Url {
            url: format!("https://example.com/{title}.mp3"),
            headers: BTreeMap::new(),
        },
    }
}

fn core_with(titles: &[&str]) -> PlaybackCore {
    let mut core = PlaybackCore::new(7);
    core.enqueue(titles.iter().map(|title| input(title)).collect(), false);
    core
}

fn current_title(core: &PlaybackCore) -> Option<String> {
    core.current_item().map(|item| item.title.clone())
}

fn loaded(effects: &[Effect]) -> Option<(u32, String)> {
    effects.iter().find_map(|effect| match effect {
        Effect::Load { load_id, item, .. } => Some((*load_id, item.title.clone())),
        _ => None,
    })
}

fn report(core: &mut PlaybackCore, event: EngineEvent) -> Vec<Effect> {
    let load_id = core.load_id;
    core.report(EngineReport { load_id, event })
}

#[test]
fn enqueue_without_play_now_only_changes_the_queue() {
    let mut core = PlaybackCore::new(1);

    let effects = core.enqueue(vec![input("a"), input("b")], false);

    assert_eq!(effects, vec![Effect::QueueChanged]);
    assert_eq!(core.queue().items.len(), 2);
    assert_eq!(core.state().status, PlaybackStatus::Idle);
    assert_eq!(current_title(&core), None);
}

#[test]
fn enqueue_with_play_now_loads_the_first_new_item() {
    let mut core = core_with(&["a"]);

    let effects = core.enqueue(vec![input("b"), input("c")], true);

    assert_eq!(loaded(&effects).map(|(_, title)| title), Some("b".into()));
    assert_eq!(core.state().status, PlaybackStatus::Loading);
    assert_eq!(current_title(&core), Some("b".into()));
}

#[test]
fn play_on_a_fresh_queue_starts_at_the_beginning() {
    let mut core = core_with(&["a", "b"]);

    let effects = core.play();

    assert_eq!(loaded(&effects).map(|(_, title)| title), Some("a".into()));
}

#[test]
fn engine_reports_drive_status_and_position() {
    let mut core = core_with(&["a"]);
    core.play();

    report(&mut core, EngineEvent::Playing);
    report(
        &mut core,
        EngineEvent::Progress {
            position_ms: 12_500.0,
            duration_ms: Some(200_000.0),
        },
    );

    let state = core.state();
    assert_eq!(state.status, PlaybackStatus::Playing);
    assert_eq!(state.position_ms, 12_500.0);
    assert_eq!(state.duration_ms, Some(200_000.0));
}

#[test]
fn reports_from_a_replaced_track_are_ignored() {
    let mut core = core_with(&["a", "b"]);
    let (old_load, _) = loaded(&core.play()).unwrap();
    core.next();

    let effects = core.report(EngineReport {
        load_id: old_load,
        event: EngineEvent::Ended,
    });

    assert!(effects.is_empty());
    assert_eq!(current_title(&core), Some("b".into()));
}

#[test]
fn toggle_pauses_while_playing_and_resumes_while_paused() {
    let mut core = core_with(&["a"]);
    core.play();
    report(&mut core, EngineEvent::Playing);

    assert_eq!(core.toggle(), vec![Effect::Engine(EngineCommand::Pause)]);
    report(&mut core, EngineEvent::Paused);
    assert_eq!(core.toggle(), vec![Effect::Engine(EngineCommand::Play)]);
}

#[test]
fn ended_advances_to_the_next_item() {
    let mut core = core_with(&["a", "b"]);
    core.play();

    let effects = report(&mut core, EngineEvent::Ended);

    assert_eq!(loaded(&effects).map(|(_, title)| title), Some("b".into()));
}

#[test]
fn ended_on_the_last_item_stops_without_repeat() {
    let mut core = core_with(&["a"]);
    core.play();

    let effects = report(&mut core, EngineEvent::Ended);

    assert_eq!(effects, vec![Effect::StateChanged]);
    assert_eq!(core.state().status, PlaybackStatus::Ended);
    assert_eq!(core.state().position_ms, 180_000.0);
}

#[test]
fn play_after_the_queue_ended_restarts_the_last_item() {
    let mut core = core_with(&["a"]);
    core.play();
    report(&mut core, EngineEvent::Ended);

    let effects = core.play();

    assert_eq!(loaded(&effects).map(|(_, title)| title), Some("a".into()));
}

#[test]
fn repeat_all_wraps_around() {
    let mut core = core_with(&["a", "b"]);
    core.set_repeat(RepeatMode::All);
    core.play();
    core.next();

    let effects = report(&mut core, EngineEvent::Ended);

    assert_eq!(loaded(&effects).map(|(_, title)| title), Some("a".into()));
}

#[test]
fn repeat_one_replays_on_end_but_skip_still_moves_on() {
    let mut core = core_with(&["a", "b"]);
    core.set_repeat(RepeatMode::One);
    core.play();

    let replay = report(&mut core, EngineEvent::Ended);
    assert_eq!(loaded(&replay).map(|(_, title)| title), Some("a".into()));

    let skip = core.next();
    assert_eq!(loaded(&skip).map(|(_, title)| title), Some("b".into()));
}

#[test]
fn next_at_the_end_without_repeat_does_nothing() {
    let mut core = core_with(&["a"]);
    core.play();

    assert!(core.next().is_empty());
}

#[test]
fn previous_restarts_the_track_after_three_seconds() {
    let mut core = core_with(&["a", "b"]);
    core.play();
    core.next();
    report(
        &mut core,
        EngineEvent::Progress {
            position_ms: 45_000.0,
            duration_ms: None,
        },
    );

    let effects = core.previous();

    assert_eq!(
        effects.first(),
        Some(&Effect::Engine(EngineCommand::Seek { position_ms: 0.0 }))
    );
    assert_eq!(current_title(&core), Some("b".into()));
}

#[test]
fn previous_near_the_start_goes_back_one_item() {
    let mut core = core_with(&["a", "b"]);
    core.play();
    core.next();

    let effects = core.previous();

    assert_eq!(loaded(&effects).map(|(_, title)| title), Some("a".into()));
}

#[test]
fn previous_after_the_queue_ended_replays_the_last_item() {
    let mut core = core_with(&["a", "b"]);
    core.play();
    core.next();
    report(&mut core, EngineEvent::Ended);

    let effects = core.previous();

    assert_eq!(loaded(&effects).map(|(_, title)| title), Some("b".into()));
    assert!(matches!(
        effects.first(),
        Some(Effect::Load { autoplay: true, .. })
    ));
}

#[test]
fn seek_is_clamped_to_the_track() {
    let mut core = core_with(&["a"]);
    core.play();

    core.seek(-50.0);
    assert_eq!(core.state().position_ms, 0.0);
    core.seek(999_999.0);
    assert_eq!(core.state().position_ms, 180_000.0);
}

#[test]
fn volume_is_clamped_and_forwarded() {
    let mut core = core_with(&["a"]);

    let effects = core.set_volume(1.7);

    assert_eq!(core.state().volume, 1.0);
    assert_eq!(
        effects.first(),
        Some(&Effect::Engine(EngineCommand::SetVolume { volume: 1.0 }))
    );
}

#[test]
fn errors_skip_ahead_and_stop_when_everything_failed() {
    let mut core = core_with(&["a", "b"]);
    core.set_repeat(RepeatMode::All);
    core.play();

    let first_failure = report(
        &mut core,
        EngineEvent::Error {
            message: "403".into(),
        },
    );
    assert_eq!(
        loaded(&first_failure).map(|(_, title)| title),
        Some("b".into())
    );

    let second_failure = report(
        &mut core,
        EngineEvent::Error {
            message: "403".into(),
        },
    );
    assert!(second_failure.contains(&Effect::Engine(EngineCommand::Stop)));
    assert_eq!(core.state().status, PlaybackStatus::Error);
    assert_eq!(core.state().error.as_deref(), Some("403"));
}

#[test]
fn a_long_queue_stops_after_a_few_failures_in_a_row() {
    let mut core = core_with(&["a", "b", "c", "d", "e", "f"]);
    core.play();
    let fail = |core: &mut PlaybackCore| {
        report(
            core,
            EngineEvent::Error {
                message: "no source".into(),
            },
        )
    };

    assert_eq!(
        loaded(&fail(&mut core)).map(|(_, title)| title),
        Some("b".into())
    );
    assert_eq!(
        loaded(&fail(&mut core)).map(|(_, title)| title),
        Some("c".into())
    );
    let third = fail(&mut core);
    assert_eq!(
        loaded(&third),
        None,
        "three failures in a row stop playback"
    );
    assert!(third.contains(&Effect::Engine(EngineCommand::Stop)));
    assert_eq!(core.state().status, PlaybackStatus::Error);
}

#[test]
fn a_successful_play_resets_the_error_streak() {
    let mut core = core_with(&["a", "b", "c"]);
    core.play();
    report(
        &mut core,
        EngineEvent::Error {
            message: "x".into(),
        },
    );
    report(&mut core, EngineEvent::Playing);

    let effects = report(
        &mut core,
        EngineEvent::Error {
            message: "y".into(),
        },
    );

    assert_eq!(loaded(&effects).map(|(_, title)| title), Some("c".into()));
}

#[test]
fn shuffle_keeps_the_current_item_and_covers_every_item() {
    let mut core = core_with(&["a", "b", "c", "d", "e"]);
    core.play();
    core.next();

    core.set_shuffle(true);

    let queue = core.queue();
    assert_eq!(current_title(&core), Some("b".into()));
    assert_eq!(queue.order[0], 1, "current item moves to the front");
    let mut order = queue.order.clone();
    order.sort();
    assert_eq!(order, vec![0, 1, 2, 3, 4]);
}

#[test]
fn unshuffle_restores_queue_order_and_keeps_playing() {
    let mut core = core_with(&["a", "b", "c", "d"]);
    core.play();
    core.set_shuffle(true);
    core.next();
    let playing = current_title(&core);

    core.set_shuffle(false);

    assert_eq!(core.queue().order, vec![0, 1, 2, 3]);
    assert_eq!(current_title(&core), playing);
}

#[test]
fn enqueue_while_shuffled_never_inserts_before_the_current_item() {
    let mut core = core_with(&["a", "b", "c"]);
    core.play();
    core.set_shuffle(true);

    core.enqueue(vec![input("d"), input("e")], false);

    let queue = core.queue();
    assert_eq!(queue.order[0], 0, "current item stays first");
    assert_eq!(queue.order.len(), 5);
}

#[test]
fn play_item_jumps_to_that_item() {
    let mut core = core_with(&["a", "b", "c"]);
    let target = core.queue().items[2].id.clone();

    let effects = core.play_item(&target);

    assert_eq!(loaded(&effects).map(|(_, title)| title), Some("c".into()));
}

#[test]
fn removing_an_earlier_item_keeps_the_current_one() {
    let mut core = core_with(&["a", "b", "c"]);
    core.play();
    core.next();
    let first = core.queue().items[0].id.clone();

    core.remove(&first);

    assert_eq!(current_title(&core), Some("b".into()));
    assert_eq!(core.queue().order, vec![0, 1]);
}

#[test]
fn removing_the_current_item_moves_to_the_next() {
    let mut core = core_with(&["a", "b"]);
    core.play();
    report(&mut core, EngineEvent::Playing);
    let current = core.queue().items[0].id.clone();

    let effects = core.remove(&current);

    assert_eq!(loaded(&effects).map(|(_, title)| title), Some("b".into()));
}

#[test]
fn removing_the_last_playing_item_stops() {
    let mut core = core_with(&["a"]);
    core.play();
    let current = core.queue().items[0].id.clone();

    let effects = core.remove(&current);

    assert!(effects.contains(&Effect::Engine(EngineCommand::Stop)));
    assert_eq!(core.state().status, PlaybackStatus::Idle);
    assert_eq!(current_title(&core), None);
}

#[test]
fn clear_empties_the_queue_and_stops() {
    let mut core = core_with(&["a", "b"]);
    core.play();

    let effects = core.clear();

    assert!(effects.contains(&Effect::Engine(EngineCommand::Stop)));
    assert!(core.queue().items.is_empty());
    assert_eq!(core.state().current_item_id, None);
}

#[test]
fn every_load_gets_a_new_id() {
    let mut core = core_with(&["a", "b"]);

    let (first, _) = loaded(&core.play()).unwrap();
    let (second, _) = loaded(&core.next()).unwrap();

    assert!(second > first);
}

#[test]
fn replace_swaps_the_queue_and_plays_the_start_item() {
    let mut core = core_with(&["old-a", "old-b"]);
    core.play();

    let effects = core.replace(vec![input("x"), input("y"), input("z")], Some(1), false);

    assert_eq!(loaded(&effects).map(|(_, title)| title), Some("y".into()));
    assert!(effects.contains(&Effect::QueueChanged));
    let queue = core.queue();
    assert_eq!(queue.items.len(), 3);
    assert_eq!(queue.order, vec![0, 1, 2]);
    assert!(!core.state().shuffle);
}

#[test]
fn replace_with_shuffle_starts_with_the_chosen_item() {
    let mut core = PlaybackCore::new(3);
    let titles: Vec<String> = (0..10).map(|index| format!("t{index}")).collect();

    let effects = core.replace(
        titles.iter().map(|title| input(title)).collect(),
        Some(4),
        true,
    );

    assert_eq!(loaded(&effects).map(|(_, title)| title), Some("t4".into()));
    let queue = core.queue();
    assert_eq!(queue.order[0], 4);
    let mut sorted = queue.order.clone();
    sorted.sort();
    assert_eq!(sorted, (0..10).collect::<Vec<u32>>());
    assert!(core.state().shuffle);
}

#[test]
fn replace_with_shuffle_and_no_start_picks_any_item() {
    let mut core = PlaybackCore::new(11);

    let effects = core.replace(vec![input("a"), input("b"), input("c")], None, true);

    assert!(loaded(&effects).is_some());
    assert_eq!(core.queue().order.len(), 3);
}

#[test]
fn replace_with_nothing_clears() {
    let mut core = core_with(&["a"]);
    core.play();

    let effects = core.replace(Vec::new(), None, false);

    assert!(effects.contains(&Effect::Engine(EngineCommand::Stop)));
    assert!(core.queue().items.is_empty());
}
