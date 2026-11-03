use std::collections::BTreeMap;

use raygun_model::{MediaSource, TrackQuery};

use super::*;

fn query(title: &str, artist: &str, duration_ms: Option<u32>) -> TrackQuery {
    TrackQuery {
        title: title.into(),
        artist: Some(artist.into()),
        album: None,
        duration_ms,
    }
}

fn candidate(id: &str, title: &str, artist: Option<&str>, duration_ms: Option<u32>) -> Candidate {
    Candidate {
        provider_id: "dev.example/stream".into(),
        provider_name: "Example".into(),
        id: id.into(),
        title: title.into(),
        artist: artist.map(Into::into),
        duration_ms,
        quality: None,
        bitrate_kbps: None,
        source: Some(MediaSource::Url {
            url: format!("https://example.org/{id}.mp3"),
            headers: BTreeMap::new(),
        }),
    }
}

fn best(query: &TrackQuery, candidates: Vec<Candidate>) -> Option<String> {
    rank(query, candidates, &History::default())
        .first()
        .map(|scored| scored.candidate.id.clone())
}

#[test]
fn normalizes_titles_and_keys() {
    assert_eq!(normalize("Beyoncé & JAY-Z"), "beyonce and jay z");
    assert_eq!(normalize("  Don't   Stop! "), "don t stop");
    assert_eq!(
        core_title("Hey Jude (Remastered 2015) [Official Video]"),
        "hey jude"
    );
    assert_eq!(core_title("Señorita feat. Camila Cabello"), "senorita");
    assert_eq!(
        track_key(&query(
            "What a Wonderful World (Mono)",
            "Louis Armstrong",
            None
        )),
        "louis armstrong|what a wonderful world"
    );
    // The same track spelled differently shares a key.
    assert_eq!(
        track_key(&query(
            "WHAT A WONDERFUL WORLD",
            "Louis  Armstrong",
            Some(1)
        )),
        track_key(&query("What a Wonderful World", "louis armstrong", None))
    );
}

#[test]
fn prefers_the_studio_recording_over_variants() {
    let wanted = query("What a Wonderful World", "Louis Armstrong", Some(137_000));
    let picked = best(
        &wanted,
        vec![
            candidate(
                "live",
                "What a Wonderful World (Live)",
                Some("Louis Armstrong"),
                Some(140_000),
            ),
            candidate(
                "cover",
                "What a Wonderful World - piano cover",
                Some("Some Pianist"),
                Some(150_000),
            ),
            candidate(
                "karaoke",
                "What a Wonderful World Karaoke",
                None,
                Some(137_000),
            ),
            candidate(
                "studio",
                "What A Wonderful World",
                Some("Louis Armstrong"),
                Some(137_800),
            ),
        ],
    );
    assert_eq!(picked.as_deref(), Some("studio"));
}

#[test]
fn asked_for_variants_are_not_penalized() {
    let wanted = query("Wonderwall (Live)", "Oasis", None);
    let live = candidate(
        "live",
        "Wonderwall (Live at Knebworth)",
        Some("Oasis"),
        None,
    );
    let studio = candidate("studio", "Wonderwall", Some("Oasis"), None);
    let history = History::default();
    assert!(score(&wanted, &live, &history) >= score(&wanted, &studio, &history) - 5);
}

#[test]
fn artist_names_in_titles_count() {
    // YouTube-style "Artist - Title" with no separate artist field.
    let wanted = query("Bohemian Rhapsody", "Queen", Some(354_000));
    let picked = best(
        &wanted,
        vec![
            candidate(
                "other",
                "Bohemian Rhapsody",
                Some("A Tribute Band"),
                Some(354_000),
            ),
            candidate(
                "yt",
                "Queen - Bohemian Rhapsody (Official Video)",
                None,
                Some(355_000),
            ),
        ],
    );
    assert_eq!(picked.as_deref(), Some("yt"));
}

#[test]
fn duration_mismatches_lose_and_poor_matches_are_dropped() {
    let wanted = query("Clair de Lune", "Claude Debussy", Some(300_000));
    let picked = best(
        &wanted,
        vec![
            candidate(
                "long",
                "Clair de Lune",
                Some("Claude Debussy"),
                Some(600_000),
            ),
            candidate(
                "right",
                "Clair de Lune",
                Some("Claude Debussy"),
                Some(302_000),
            ),
        ],
    );
    assert_eq!(picked.as_deref(), Some("right"));

    let unrelated = vec![candidate(
        "x",
        "Something Else Entirely",
        Some("Nobody"),
        Some(10_000),
    )];
    assert_eq!(best(&wanted, unrelated), None);
}

#[test]
fn embeds_are_the_fallback() {
    let wanted = query("Take Five", "Dave Brubeck", None);
    let mut embed = candidate("embed", "Take Five", Some("Dave Brubeck"), None);
    embed.source = Some(MediaSource::YouTube {
        video_id: "vmDDOFXSgAs".into(),
    });
    let direct = candidate("direct", "Take Five", Some("Dave Brubeck"), None);
    assert_eq!(
        best(&wanted, vec![embed, direct]).as_deref(),
        Some("direct")
    );
}

#[test]
fn history_shapes_the_ranking() {
    let wanted = query("So What", "Miles Davis", None);
    let first = candidate("a", "So What", Some("Miles Davis"), None);
    let mut second = candidate("b", "So What", Some("Miles Davis"), None);
    second.provider_id = "dev.other/stream".into();

    // Equal scores keep provider order.
    assert_eq!(
        best(&wanted, vec![first.clone(), second.clone()]).as_deref(),
        Some("a")
    );

    // A verified pick wins; a failed one is skipped.
    let mut history = History::default();
    history.verified.insert(second.key());
    let ranked = rank(&wanted, vec![first.clone(), second.clone()], &history);
    assert_eq!(ranked[0].candidate.id, "b");

    let mut history = History::default();
    history.failed.insert(first.key());
    let ranked = rank(&wanted, vec![first.clone(), second.clone()], &history);
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].candidate.id, "b");

    // A provider that keeps failing drops below one that works.
    let mut history = History::default();
    history.providers.insert(
        first.provider_id.clone(),
        ProviderRecord {
            successes: 0,
            failures: 6,
        },
    );
    history.providers.insert(
        second.provider_id.clone(),
        ProviderRecord {
            successes: 6,
            failures: 0,
        },
    );
    let ranked = rank(&wanted, vec![first, second], &history);
    assert_eq!(ranked[0].candidate.id, "b");
    assert!(ranked[0].score - ranked[1].score >= 15);
}

#[test]
fn a_one_word_title_needs_some_sign_of_the_artist() {
    // "i can't swim" by nobody named is no answer for BTS's "SWIM" (M46.10).
    let swim = query("SWIM", "BTS", None);
    assert_eq!(
        best(&swim, vec![candidate("other", "i can't swim", None, None)]),
        None
    );
    assert_eq!(
        best(
            &swim,
            vec![
                candidate("other", "i can't swim", None, None),
                candidate("bts", "BTS - SWIM", None, None),
            ]
        ),
        Some("bts".into())
    );
    // A whole title with nothing about its artist still plays.
    assert_eq!(
        best(
            &query("Hello, Dolly", "Louis Armstrong", None),
            vec![candidate("dolly", "Hello, Dolly", None, None)]
        ),
        Some("dolly".into())
    );
}

fn at(id: &str, quality: &str) -> Candidate {
    Candidate {
        quality: Some(quality.into()),
        ..candidate(id, "Song", Some("Band"), Some(200_000))
    }
}

#[test]
fn quality_labels_name_bitrates() {
    assert_eq!(label_kbps("64Kbps MP3"), Some(64));
    assert_eq!(label_kbps("320 kbps"), Some(320));
    assert_eq!(label_kbps("MP3 128k"), Some(128));
    assert_eq!(label_kbps("FLAC"), Some(LOSSLESS_KBPS));
    assert_eq!(label_kbps("24bit Flac"), Some(LOSSLESS_KBPS));
    assert_eq!(label_kbps("YouTube"), None);
    assert_eq!(label_kbps("VBR MP3"), None);
    assert_eq!(label_kbps("2024 remaster"), None);
}

#[test]
fn the_same_song_is_picked_at_the_quality_asked_for() {
    let wanted = query("Song", "Band", Some(200_000));
    let offered = || {
        vec![
            at("hq", "320 kbps"),
            at("low", "64Kbps MP3"),
            at("mid", "128k"),
            at("flac", "FLAC"),
        ]
    };
    let first = |target| {
        rank_for(&wanted, offered(), &History::default(), target)
            .first()
            .map(|scored| scored.candidate.id.clone())
    };
    assert_eq!(first(QualityTarget::Kbps(64)).as_deref(), Some("low"));
    assert_eq!(first(QualityTarget::Kbps(128)).as_deref(), Some("mid"));
    assert_eq!(first(QualityTarget::Kbps(320)).as_deref(), Some("hq"));
    assert_eq!(first(QualityTarget::Best).as_deref(), Some("flac"));
    // Without a target the providers' order stands.
    assert_eq!(first(QualityTarget::Any).as_deref(), Some("hq"));
}

#[test]
fn quality_never_beats_a_better_match() {
    let wanted = query("Song", "Band", Some(200_000));
    let wrong_length = Candidate {
        quality: Some("64Kbps MP3".into()),
        ..candidate("low", "Song", Some("Band"), Some(150_000))
    };
    let ranked = rank_for(
        &wanted,
        vec![at("hq", "320 kbps"), wrong_length],
        &History::default(),
        QualityTarget::Kbps(64),
    );
    assert_eq!(ranked[0].candidate.id, "hq");
}

#[test]
fn a_remembered_pick_gives_way_to_a_better_fitting_bitrate() {
    let wanted = query("Song", "Band", Some(200_000));
    let mut history = History::default();
    history.verified.insert(at("hq", "320 kbps").key());
    let ranked = rank_for(
        &wanted,
        vec![at("hq", "320 kbps"), at("low", "64Kbps MP3")],
        &history,
        QualityTarget::Kbps(64),
    );
    assert_eq!(ranked[0].candidate.id, "low");
    assert!(QualityTarget::Kbps(64).fits(Some(96)));
    assert!(!QualityTarget::Kbps(64).fits(Some(320)));
    assert!(QualityTarget::Kbps(192).fits(None));
    assert!(!QualityTarget::Best.fits(Some(128)));
}

#[test]
fn imports_need_the_artist_to_agree() {
    let query = TrackQuery {
        title: "Not A Real Song".into(),
        artist: Some("Qwzx Imaginary Band".into()),
        album: None,
        duration_ms: None,
    };
    let by = |artist: Option<&str>, title: &str| Candidate {
        provider_id: "p".into(),
        provider_name: "P".into(),
        id: title.into(),
        title: title.into(),
        artist: artist.map(str::to_owned),
        duration_ms: None,
        quality: None,
        bitrate_kbps: None,
        source: None,
    };
    assert!(!artist_agrees(
        &query,
        &by(Some("Someone Else"), "Not a Real Song")
    ));
    assert!(artist_agrees(
        &query,
        &by(Some("Qwzx Imaginary Band"), "Not a Real Song")
    ));
    assert!(artist_agrees(
        &query,
        &by(None, "Qwzx Imaginary Band - Not a Real Song")
    ));
    assert!(artist_agrees(
        &TrackQuery {
            artist: None,
            ..query.clone()
        },
        &by(Some("Anyone"), "Not a Real Song")
    ));
}
