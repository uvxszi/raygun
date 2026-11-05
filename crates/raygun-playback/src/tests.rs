use std::collections::BTreeMap;

use raygun_model::{MediaKind, MediaSource};

use super::*;

fn input(title: &str) -> QueueItemInput {
    QueueItemInput {
        explicit: false,
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
fn queued_items_change_in_place() {
    let mut core = core_with(&["a", "b"]);
    core.play();
    let picked = MediaSource::YouTube {
        video_id: "v".into(),
    };
    let effects = core.update_items(|item| {
        if item.title != "b" {
            return false;
        }
        item.source = picked.clone();
        item.media_kind = MediaKind::Video;
        true
    });
    assert_eq!(effects, vec![Effect::QueueChanged]);
    assert_eq!(
        current_title(&core),
        Some("a".into()),
        "the current item stays"
    );
    let next = core.next();
    let source = next.iter().find_map(|effect| match effect {
        Effect::Load { item, .. } => Some(item.source.clone()),
        _ => None,
    });
    assert_eq!(source, Some(picked), "b loads its new source");
    assert!(core.update_items(|_| false).is_empty());
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

    // Each answers at once with the state it asked for (M46.16).
    assert_eq!(
        core.toggle(),
        vec![Effect::Engine(EngineCommand::Pause), Effect::StateChanged]
    );
    report(&mut core, EngineEvent::Paused);
    assert_eq!(
        core.toggle(),
        vec![Effect::Engine(EngineCommand::Play), Effect::StateChanged]
    );
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
fn clear_upcoming_keeps_the_current_item_and_what_played() {
    let mut core = core_with(&["a", "b", "c", "d"]);
    core.play_item(&core.queue().items[1].id.clone());
    report(&mut core, EngineEvent::Playing);

    let effects = core.clear_upcoming();

    assert_eq!(effects, vec![Effect::QueueChanged]);
    assert_eq!(order_titles(&core), ["a", "b"]);
    assert_eq!(current_title(&core).as_deref(), Some("b"));
    assert_eq!(core.state().status, PlaybackStatus::Playing);
    // Nothing comes next any more; the current song plays on.
    assert!(core.next().is_empty());
    assert!(core.clear_upcoming().is_empty());
}

#[test]
fn clear_upcoming_follows_the_shuffled_order() {
    let mut core = core_with(&["a", "b", "c", "d", "e"]);
    core.play_item(&core.queue().items[2].id.clone());
    core.set_shuffle(true);
    let before = order_titles(&core);
    assert_eq!(before[0], "c");

    core.clear_upcoming();

    assert_eq!(order_titles(&core), ["c"]);
    assert_eq!(core.queue().items.len(), 1);
    assert_eq!(current_title(&core).as_deref(), Some("c"));
}

#[test]
fn clear_upcoming_with_nothing_current_empties_the_queue() {
    let mut core = core_with(&["a", "b"]);

    let effects = core.clear_upcoming();

    assert!(effects.contains(&Effect::QueueChanged));
    assert!(core.queue().items.is_empty());
    assert!(PlaybackCore::new(1).clear_upcoming().is_empty());
}

#[test]
fn an_engine_that_says_it_plays_but_stands_still_is_nudged() {
    let mut core = core_with(&["a"]);
    core.play();
    report(&mut core, EngineEvent::Playing);
    progress(&mut core, 10_000.0);

    // The stretch starts, and moving on keeps it fresh.
    assert!(core.watchdog(0.0).is_empty());
    progress(&mut core, 11_000.0);
    assert!(core.watchdog(1_000.0).is_empty());
    assert!(core.watchdog(2_000.0).is_empty());
    assert!(core.watchdog(3_900.0).is_empty());
    assert!(core.started());

    // Three seconds standing still: told again, and no longer said to play.
    let effects = core.watchdog(4_100.0);
    assert_eq!(
        effects,
        vec![
            Effect::Engine(EngineCommand::Seek {
                position_ms: 11_000.0
            }),
            Effect::Engine(EngineCommand::Play),
            Effect::StateChanged,
        ]
    );
    assert_eq!(core.state().status, PlaybackStatus::Buffering);
    // Buffering after it played is left to the engine.
    assert!(core.watchdog(9_000.0).is_empty());
    report(&mut core, EngineEvent::Playing);
    assert_eq!(core.state().status, PlaybackStatus::Playing);
}

#[test]
fn a_ready_engine_that_never_starts_is_nudged_then_shown_paused() {
    let mut core = core_with(&["a"]);
    core.play();
    report(&mut core, EngineEvent::Buffering);
    // Waiting for its stream isn't the watchdog's business.
    assert!(core.watchdog(0.0).is_empty());
    assert!(core.watchdog(10_000.0).is_empty());

    // It has the stream (it says how long it is) but doesn't start.
    progress(&mut core, 0.0);
    assert!(core.watchdog(10_500.0).is_empty());
    assert_eq!(
        core.watchdog(13_600.0),
        vec![
            Effect::Engine(EngineCommand::Pause),
            Effect::Engine(EngineCommand::Play),
        ]
    );
    // The nudge's own pause doesn't count as the listener's.
    assert!(report(&mut core, EngineEvent::Paused).is_empty());
    assert_eq!(core.state().status, PlaybackStatus::Buffering);

    // Still nothing: it shows paused, so Play (a tap) starts it.
    let effects = core.watchdog(16_700.0);
    assert!(effects.contains(&Effect::Engine(EngineCommand::Pause)));
    assert_eq!(core.state().status, PlaybackStatus::Paused);
    assert!(core.held());
    assert!(core.watchdog(30_000.0).is_empty());

    core.play();
    report(&mut core, EngineEvent::Playing);
    assert_eq!(core.state().status, PlaybackStatus::Playing);
    assert!(core.started());
}

#[test]
fn a_nudged_engine_that_starts_plays_on() {
    let mut core = core_with(&["a"]);
    core.play();
    progress(&mut core, 0.0);
    core.watchdog(0.0);
    assert_eq!(core.watchdog(3_100.0).len(), 2);

    report(&mut core, EngineEvent::Playing);
    assert_eq!(core.state().status, PlaybackStatus::Playing);
    // A real pause after that is the listener's.
    report(&mut core, EngineEvent::Paused);
    assert_eq!(core.state().status, PlaybackStatus::Paused);
}

#[test]
fn the_watchdog_leaves_paused_and_new_loads_alone() {
    let mut core = core_with(&["a", "b"]);
    core.play();
    report(&mut core, EngineEvent::Playing);
    progress(&mut core, 5_000.0);
    core.pause();

    assert!(core.watchdog(0.0).is_empty());
    assert!(core.watchdog(10_000.0).is_empty());

    // A new load starts over: not started, nothing to nudge until it's ready.
    core.next();
    assert!(!core.started());
    assert!(core.watchdog(20_000.0).is_empty());
    assert!(core.watchdog(40_000.0).is_empty());
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

#[test]
fn replace_at_starts_where_listening_stopped() {
    let mut core = core_with(&["old"]);
    let effects = core.replace_at(vec![input("episode")], Some(0), false, 90_000.0);
    let start = effects.iter().find_map(|effect| match effect {
        Effect::Load { start_ms, item, .. } => Some((item.title.clone(), *start_ms)),
        _ => None,
    });
    assert_eq!(start, Some(("episode".to_owned(), 90_000.0)));
    assert_eq!(core.state().position_ms, 90_000.0);
    assert_eq!(current_title(&core).as_deref(), Some("episode"));

    let effects = core.replace_at(vec![input("again")], Some(0), false, -5.0);
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, Effect::Load { start_ms, .. } if *start_ms == 0.0))
    );
}

fn order_titles(core: &PlaybackCore) -> Vec<String> {
    let queue = core.queue();
    queue
        .order
        .iter()
        .map(|&index| queue.items[index as usize].title.clone())
        .collect()
}

#[test]
fn play_next_goes_right_after_the_current_item_in_order() {
    let mut core = core_with(&["a", "b", "c"]);
    core.play_item(&core.queue().items[0].id.clone());

    let effects = core.enqueue_next(vec![input("x"), input("y")]);

    assert_eq!(effects, vec![Effect::QueueChanged]);
    assert_eq!(order_titles(&core), ["a", "x", "y", "b", "c"]);
    assert_eq!(current_title(&core).as_deref(), Some("a"));
    core.next();
    assert_eq!(current_title(&core).as_deref(), Some("x"));
}

#[test]
fn moving_an_item_keeps_the_current_one_playing() {
    let mut core = core_with(&["a", "b", "c", "d"]);
    let ids: Vec<String> = core
        .queue()
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect();
    core.play_item(&ids[1]);

    // "d" before "b", then "a" to the end.
    core.move_item(&ids[3], Some(&ids[1]));
    assert_eq!(order_titles(&core), ["a", "d", "b", "c"]);
    core.move_item(&ids[0], None);
    assert_eq!(order_titles(&core), ["d", "b", "c", "a"]);
    assert_eq!(current_title(&core).as_deref(), Some("b"));
    core.next();
    assert_eq!(current_title(&core).as_deref(), Some("c"));
    // Unknown items change nothing.
    assert!(core.move_item("nope", None).is_empty());
}

#[test]
fn speed_is_kept_within_half_and_double() {
    let mut core = core_with(&["a"]);

    let effects = core.set_rate(1.5);
    assert!(effects.contains(&Effect::Engine(EngineCommand::SetRate { rate: 1.5 })));
    assert_eq!(core.state().rate, 1.5);
    core.set_rate(9.0);
    assert_eq!(core.state().rate, 2.0);
    assert!(core.set_rate(f64::NAN).is_empty());
}

#[test]
fn the_sleep_timer_pauses_or_stops_after_the_song() {
    let mut core = core_with(&["a", "b"]);
    core.play_item(&core.queue().items[0].id.clone());
    report(&mut core, EngineEvent::Playing);

    core.set_sleep(Some(1_000.0), false);
    assert_eq!(core.state().sleep_at, Some(1_000.0));
    let effects = core.sleep_now();
    assert!(effects.contains(&Effect::Engine(EngineCommand::Pause)));
    assert_eq!(core.state().sleep_at, None);

    // End of song: the next one loads paused, and the timer is spent.
    core.set_sleep(None, true);
    let effects = report(&mut core, EngineEvent::Ended);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        Effect::Load { autoplay: false, item, .. } if item.title == "b"
    )));
    assert!(!core.state().stop_after_current);
}

fn progress(core: &mut PlaybackCore, position_ms: f64) -> Vec<Effect> {
    report(
        core,
        EngineEvent::Progress {
            position_ms,
            duration_ms: Some(180_000.0),
        },
    )
}

fn faded(effects: &[Effect]) -> Option<(String, f64)> {
    effects.iter().find_map(|effect| match effect {
        Effect::Load { item, fade_ms, .. } => Some((item.title.clone(), *fade_ms)),
        _ => None,
    })
}

#[test]
fn crossfade_starts_the_next_song_before_the_end() {
    let mut core = core_with(&["a", "b"]);
    core.set_crossfade(5_000.0);
    core.play();
    report(&mut core, EngineEvent::Playing);
    assert_eq!(faded(&progress(&mut core, 170_000.0)), None);
    // 4 s left: the next song starts now, fading over what remains.
    assert_eq!(
        faded(&progress(&mut core, 176_000.0)),
        Some(("b".into(), 4_000.0))
    );
    assert_eq!(current_title(&core), Some("b".into()));
    // The old song's end arrives under its old load id and changes nothing.
    let old = core.load_id - 1;
    assert!(
        core.report(EngineReport {
            load_id: old,
            event: EngineEvent::Ended,
        })
        .is_empty()
    );
}

#[test]
fn crossfade_waits_for_the_end_trim_and_skips_where_the_song_restarts_or_stops() {
    let mut core = core_with(&["a", "b"]);
    core.set_crossfade(3_000.0);
    core.play();
    core.set_end(core.load_id, Some(120_000.0));
    report(&mut core, EngineEvent::Playing);
    assert_eq!(
        faded(&progress(&mut core, 118_000.0)),
        Some(("b".into(), 2_000.0))
    );

    // Nothing after the last song, unless the queue repeats.
    report(&mut core, EngineEvent::Playing);
    assert_eq!(faded(&progress(&mut core, 178_000.0)), None);
    core.set_repeat(RepeatMode::All);
    assert_eq!(
        faded(&progress(&mut core, 178_500.0)),
        Some(("a".into(), 1_500.0))
    );

    // Repeat one restarts the song; the sleep timer's end of song waits for the end.
    report(&mut core, EngineEvent::Playing);
    core.set_repeat(RepeatMode::One);
    assert_eq!(faded(&progress(&mut core, 179_000.0)), None);
    core.set_repeat(RepeatMode::Off);
    core.set_sleep(None, true);
    assert_eq!(faded(&progress(&mut core, 179_000.0)), None);

    // Off: songs end on their own.
    core.set_sleep(None, false);
    core.set_crossfade(0.0);
    assert_eq!(faded(&progress(&mut core, 179_500.0)), None);
}

#[test]
fn a_pause_holds_even_before_the_engine_answers() {
    let mut core = core_with(&["a", "b"]);
    core.play();
    assert_eq!(core.state().status, PlaybackStatus::Loading);

    // Paused while the song is still being found: the state says so at once.
    let effects = core.pause();
    assert!(effects.contains(&Effect::Engine(EngineCommand::Pause)));
    assert_eq!(core.state().status, PlaybackStatus::Paused);
    assert!(core.held());

    // An engine that starts anyway is paused again, and buffering doesn't look like playing.
    let effects = report(&mut core, EngineEvent::Playing);
    assert!(effects.contains(&Effect::Engine(EngineCommand::Pause)));
    assert_eq!(core.state().status, PlaybackStatus::Paused);
    assert!(report(&mut core, EngineEvent::Buffering).is_empty());
    assert_eq!(core.state().status, PlaybackStatus::Paused);

    // Play answers at once too, and lets the engine play.
    let effects = core.play();
    assert!(effects.contains(&Effect::Engine(EngineCommand::Play)));
    assert_eq!(core.state().status, PlaybackStatus::Loading);
    assert!(!core.held());
    report(&mut core, EngineEvent::Playing);
    assert_eq!(core.state().status, PlaybackStatus::Playing);

    // Skipping plays the next song even after a pause.
    core.pause();
    core.next();
    assert!(!core.held());
}

/// What an engine reports for a load: started, it buffers then plays; loaded paused, it says
/// paused (the native engine reports the player's whole state).
fn engine_answers(core: &mut PlaybackCore, autoplay: bool) {
    if autoplay {
        report(core, EngineEvent::Buffering);
        report(core, EngineEvent::Playing);
    } else {
        report(core, EngineEvent::Paused);
    }
}

#[test]
fn play_pressed_while_a_stream_is_found_plays_it_once_found() {
    // Reopened paused: its stream is found in the background, to load without playing.
    let mut core = PlaybackCore::new(3);
    let effects = core.restore(
        vec![input("a"), input("b")],
        vec![0, 1],
        0,
        42_000.0,
        false,
        RepeatMode::Off,
    );
    let asked = effects.iter().find_map(|effect| match effect {
        Effect::Load { autoplay, .. } => Some(*autoplay),
        _ => None,
    });
    assert_eq!(asked, Some(false));
    assert!(!core.plays_when_found());

    // Play comes before the stream does. Loading it as the load first asked (paused) made the
    // engine report paused, and the press came back as a pause.
    core.play();
    assert_eq!(core.state().status, PlaybackStatus::Loading);
    let starts = core.plays_when_found();
    assert!(starts);
    engine_answers(&mut core, starts);
    assert_eq!(core.state().status, PlaybackStatus::Playing);

    // A song paused while its stream is found loads paused, and stays so.
    core.next();
    core.pause();
    let starts = core.plays_when_found();
    assert!(!starts);
    engine_answers(&mut core, starts);
    assert_eq!(core.state().status, PlaybackStatus::Paused);
    assert!(core.held());

    // Nothing current, nothing to start.
    core.clear();
    assert!(!core.plays_when_found());
}

#[test]
fn play_and_pause_echoed_back_by_the_system_change_nothing() {
    // A media session that passes the engine's own commands back as the listener's (the play a
    // load sends, the pause it sends when it loads paused) must not move the player.
    let mut core = core_with(&["a"]);
    core.play();
    report(&mut core, EngineEvent::Buffering);
    let status = core.state().status;
    let effects = core.play();
    assert_eq!(
        core.state().status,
        status,
        "a play while loading keeps loading"
    );
    assert!(!effects.contains(&Effect::StateChanged));
    report(&mut core, EngineEvent::Playing);
    assert!(
        core.play()
            .iter()
            .all(|effect| *effect == Effect::Engine(EngineCommand::Play))
    );
    assert_eq!(core.state().status, PlaybackStatus::Playing);
    assert!(!core.held());
}

#[test]
fn a_saved_queue_comes_back_paused_where_it_was() {
    let mut core = PlaybackCore::new(3);
    let effects = core.restore(
        vec![input("a"), input("b"), input("c")],
        vec![2, 0, 1],
        0,
        42_000.0,
        true,
        RepeatMode::All,
    );
    let load = effects.iter().find_map(|effect| match effect {
        Effect::Load {
            item,
            start_ms,
            autoplay,
            ..
        } => Some((item.title.clone(), *start_ms, *autoplay)),
        _ => None,
    });
    assert_eq!(
        load,
        Some(("a".into(), 42_000.0, false)),
        "paused, where it was"
    );
    assert!(core.held());
    let state = core.state();
    assert!(state.shuffle);
    assert_eq!(state.repeat, RepeatMode::All);
    assert_eq!(core.queue().order, vec![2, 0, 1]);

    // A broken order keeps the queue's own.
    let mut core = PlaybackCore::new(3);
    core.restore(
        vec![input("a"), input("b")],
        vec![0, 0],
        1,
        0.0,
        true,
        RepeatMode::Off,
    );
    assert_eq!(core.queue().order, vec![0, 1]);
    assert!(!core.state().shuffle);
    assert_eq!(current_title(&core), Some("b".into()));
}

fn album_context() -> PlayContext {
    PlayContext {
        kind: "album".into(),
        name: "Album".into(),
        subtitle: None,
        artwork_url: None,
        to: "/music/album".into(),
        search: BTreeMap::from([("id".into(), "1".into())]),
        played_at: 0.0,
    }
}

#[test]
fn the_queue_keeps_where_it_was_played_from_until_it_is_replaced() {
    let mut core = PlaybackCore::new(1);
    core.replace(vec![input("a"), input("b")], Some(0), false);
    assert_eq!(
        core.set_context(Some(album_context())),
        vec![Effect::QueueChanged]
    );
    assert!(
        core.set_context(Some(album_context())).is_empty(),
        "unchanged"
    );
    assert_eq!(core.queue().context, Some(album_context()));

    // Songs coming and going keep it.
    core.enqueue_next(vec![input("c")]);
    core.next();
    assert_eq!(core.queue().context, Some(album_context()));

    // A new queue without a context forgets it.
    core.replace(vec![input("d")], Some(0), false);
    assert_eq!(core.queue().context, None);

    core.set_context(Some(album_context()));
    core.clear();
    assert_eq!(core.queue().context, None);
    core.enqueue(vec![input("e")], true);
    assert_eq!(core.queue().context, None, "an emptied queue starts over");
}

#[test]
fn a_queue_emptied_one_item_at_a_time_has_no_context() {
    let mut core = PlaybackCore::new(1);
    core.replace(vec![input("a")], Some(0), false);
    core.set_context(Some(album_context()));
    let id = core.queue().items[0].id.clone();
    core.remove(&id);
    assert_eq!(core.queue().context, None);
    assert!(core.context().is_none());
}

#[test]
fn looking_for_another_source_shows_until_a_stream_plays() {
    let mut core = core_with(&["a", "b"]);
    core.play();
    report(&mut core, EngineEvent::Playing);
    let load_id = core.load_id;

    // The stream stopped working: the service looks for another, and the player says so.
    assert_eq!(core.set_looking(load_id, true), vec![Effect::StateChanged]);
    let state = core.state();
    assert!(state.looking_for_source);
    assert_eq!(state.status, PlaybackStatus::Loading);
    assert!(!core.started(), "the next stream is watched from its start");
    assert!(core.set_looking(load_id, true).is_empty(), "no change");
    assert!(
        core.set_looking(load_id + 1, false).is_empty(),
        "another load's"
    );

    // Another stream plays: the note goes.
    report(&mut core, EngineEvent::Playing);
    assert!(!core.state().looking_for_source);
    assert_eq!(core.state().status, PlaybackStatus::Playing);

    // A pause stays a pause while it looks.
    core.pause();
    core.set_looking(load_id, true);
    assert_eq!(core.state().status, PlaybackStatus::Paused);
    // A new load starts plain.
    core.next();
    assert!(!core.state().looking_for_source);
}

#[test]
fn a_song_nothing_played_is_named_until_one_plays() {
    let mut core = core_with(&["a", "b"]);
    core.play();
    let first = core.current_item().map(|item| item.id.clone());
    report(
        &mut core,
        EngineEvent::Error {
            message: "No source could play a".into(),
        },
    );
    assert_eq!(core.state().skipped_item_id, first);
    assert_eq!(current_title(&core), Some("b".into()));
    // The next song plays: the skip is over.
    report(&mut core, EngineEvent::Playing);
    assert_eq!(core.state().skipped_item_id, None);
    assert_eq!(core.state().error, None);
}
