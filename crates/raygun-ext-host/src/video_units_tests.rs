use std::collections::BTreeMap;

use super::*;

fn line(name: &str, units: &[(&str, &str)]) -> Line {
    Line {
        name: name.to_owned(),
        units: units
            .iter()
            .map(|(name, id)| ((*name).to_owned(), (*id).to_owned()))
            .collect(),
    }
}

/// Each episode as (id, name, number, season) with its servers as (id, name).
type Read = Vec<(
    (String, String, Option<f64>, Option<String>),
    Vec<(String, String)>,
)>;

fn read(units: Vec<Unit>) -> Read {
    units
        .into_iter()
        .map(|unit| {
            (
                (
                    unit.episode.id,
                    unit.episode.name,
                    unit.episode.number,
                    unit.episode.volume,
                ),
                unit.servers
                    .unwrap_or_default()
                    .into_iter()
                    .map(|server| (server.id, server.name))
                    .collect(),
            )
        })
        .collect()
}

fn link(url: &str, label: &str, quality: Option<u32>) -> VideoLink {
    VideoLink {
        url: url.to_owned(),
        label: Some(label.to_owned()),
        quality,
        headers: BTreeMap::new(),
        format: None,
        subtitles: Vec::new(),
        chapters: Vec::new(),
        live: false,
    }
}

fn plain_server(id: &str, name: &str, videos: Option<Vec<VideoLink>>) -> VideoServer {
    VideoServer {
        id: id.to_owned(),
        name: name.to_owned(),
        videos,
    }
}

fn s(text: &str) -> String {
    text.to_owned()
}

#[test]
fn names_tell_episodes_from_qualities() {
    for (name, key) in [
        ("Episode 1", Some("E1")),
        ("EP 02", Some("E2")),
        ("第3集", Some("E3")),
        ("S01E04", Some("S1E4")),
        ("05", Some("E5")),
        ("1080p", None),
        ("720P", None),
        ("1080", None),
        ("4K", None),
        ("Full HD", None),
        ("1080p Full HD", None),
        ("高清", None),
        ("Line 2", None),
        ("Server 3", None),
        ("Watch Now", None),
        ("", None),
    ] {
        assert_eq!(episode_key(name).as_deref(), key, "{name}");
    }
    assert!(episodes_named(&["Episode 1", "Episode 2", "1080p"]).is_some());
    assert!(episodes_named(&["1080p", "720p", "480p"]).is_none());
    assert!(episodes_named(&["Episode 1", "1080p", "720p"]).is_none());
    assert!(
        episodes_named(&["Episode 1 1080p", "Episode 1 720p"]).is_none(),
        "one episode in two qualities is no list of episodes"
    );
}

#[test]
fn lines_listing_the_same_episodes_are_servers_even_when_named_differently() {
    let units = from_lines(vec![
        line("Line 1", &[("第01集", "a1"), ("第02集", "a2")]),
        line("Line 2", &[("01", "b1"), ("02", "b2"), ("03", "b3")]),
    ]);
    assert_eq!(
        read(units),
        vec![
            (
                (s("a1"), s("第01集"), Some(1.0), None),
                vec![(s("a1"), s("Line 1")), (s("b1"), s("Line 2"))]
            ),
            (
                (s("a2"), s("第02集"), Some(2.0), None),
                vec![(s("a2"), s("Line 1")), (s("b2"), s("Line 2"))]
            ),
            (
                (s("b3"), s("03"), Some(3.0), None),
                vec![(s("b3"), s("Line 2"))]
            ),
        ]
    );
}

#[test]
fn lines_named_as_episodes_hold_their_qualities() {
    let units = from_lines(vec![
        line("第1集", &[("1080P", "p1a"), ("720P", "p1b")]),
        line("第2集", &[("1080P", "p2a"), ("线路2", "p2b")]),
        line("第3集", &[("", "p3a")]),
    ]);
    assert_eq!(
        read(units),
        vec![
            (
                (s("p1a"), s("第1集"), Some(1.0), None),
                vec![(s("p1a"), s("1080P")), (s("p1b"), s("720P"))]
            ),
            (
                (s("p2a"), s("第2集"), Some(2.0), None),
                vec![(s("p2a"), s("1080P")), (s("p2b"), s("线路2"))]
            ),
            (
                (s("p3a"), s("第3集"), Some(3.0), None),
                vec![(s("p3a"), s("第3集"))]
            ),
        ]
    );
}

#[test]
fn units_that_are_only_qualities_make_one_episode() {
    let units = from_lines(vec![line("Play", &[("1080P", "hd"), ("720P", "sd")])]);
    assert_eq!(
        read(units),
        vec![(
            (s("hd"), s("Play"), Some(1.0), None),
            vec![(s("hd"), s("1080P")), (s("sd"), s("720P"))]
        )]
    );
    let lines = from_lines(vec![
        line("1080p Full HD", &[("auto", "x1")]),
        line("720p SD", &[("auto", "x2")]),
    ]);
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0].servers.as_ref().unwrap()[1].name,
        "720p SD · auto",
        "several lines name their servers"
    );
}

#[test]
fn lines_named_as_seasons_are_seasons_and_quality_lines_list_talks() {
    let seasons = read(from_lines(vec![
        line("Season 1", &[("Episode 1", "s1e1"), ("Episode 2", "s1e2")]),
        line("Season 2", &[("Episode 1", "s2e1")]),
    ]));
    assert_eq!(
        seasons
            .iter()
            .map(|(episode, _)| (episode.0.as_str(), episode.3.as_deref()))
            .collect::<Vec<_>>(),
        [
            ("s1e1", Some("1")),
            ("s1e2", Some("1")),
            ("s2e1", Some("2"))
        ]
    );
    // de.ccc.media: lines are qualities, their units talks with names of their own.
    let talks = read(from_lines(vec![
        line("1080p Full HD", &[("Opening", "t1"), ("Keynote", "t2")]),
        line("720p SD", &[("Opening", "u1"), ("Keynote", "u2")]),
    ]));
    assert_eq!(
        talks,
        vec![
            (
                (s("t1"), s("Opening"), Some(1.0), None),
                vec![(s("t1"), s("1080p Full HD")), (s("u1"), s("720p SD"))]
            ),
            (
                (s("t2"), s("Keynote"), Some(2.0), None),
                vec![(s("t2"), s("1080p Full HD")), (s("u2"), s("720p SD"))]
            ),
        ]
    );
}

fn lone() -> MangaChapter {
    episode("movie", "Full", None)
}

#[test]
fn a_lone_units_servers_named_as_episodes_become_episodes() {
    let servers = vec![
        plain_server("a", "EP 01", None),
        plain_server("b", "EP 02", None),
        plain_server("c", "EP 02", None),
    ];
    let split = split_unit(&lone(), &servers, None).unwrap();
    let ids: Vec<(&str, &str, Option<f64>)> = split
        .iter()
        .map(|episode| (episode.id.as_str(), episode.name.as_str(), episode.number))
        .collect();
    assert_eq!(
        ids,
        [
            ("movie#raygun-episode=E1", "EP 01", Some(1.0)),
            ("movie#raygun-episode=E2", "EP 02", Some(2.0)),
        ]
    );
    let (unit, key) = split_id(&split[1].id);
    assert_eq!((unit, key), ("movie", Some("E2")));
    let of_two = servers_of_episode(servers, "E2");
    assert_eq!(
        of_two
            .iter()
            .map(|server| server.id.as_str())
            .collect::<Vec<_>>(),
        ["b", "c"],
        "the episode's own servers, with their ids as the source gave them"
    );
}

#[test]
fn a_lone_units_videos_named_as_episodes_become_episodes_with_their_qualities() {
    let videos = vec![
        link(
            "https://cdn.example/1-hd.m3u8",
            "Episode 1 1080p",
            Some(1080),
        ),
        link("https://cdn.example/1-sd.m3u8", "Episode 1 720p", Some(720)),
        link("https://cdn.example/2.m3u8", "Episode 2", None),
        link("https://cdn.example/3.m3u8", "S01E03", Some(480)),
    ];
    let servers = vec![plain_server("srv", "Main", None)];
    let split = split_unit(&lone(), &servers, Some(&videos)).unwrap();
    assert_eq!(split.len(), 3);
    assert_eq!(split[2].id, "movie#raygun-episode=S1E3");
    assert_eq!(split[2].volume.as_deref(), Some("1"));

    let shown = servers_of_episode(servers, "E1");
    assert_eq!(shown[0].id, "srv#raygun-episode=E1");
    let (server, key) = split_id(&shown[0].id);
    assert_eq!((server, key), ("srv", Some("E1")));
    let first: Vec<(String, Option<String>)> = videos_of_episode(videos.clone(), "E1")
        .into_iter()
        .map(|video| (video.url, video.label))
        .collect();
    assert_eq!(
        first,
        [
            (s("https://cdn.example/1-hd.m3u8"), Some(s("1080p"))),
            (s("https://cdn.example/1-sd.m3u8"), Some(s("720p"))),
        ],
        "an episode's videos are named by their quality"
    );

    // Servers that came with their videos are cut to the episode's.
    let resolved = vec![plain_server("r", "Main", Some(videos))];
    let two = servers_of_episode(resolved.clone(), "E2");
    assert_eq!(two[0].videos.as_ref().unwrap().len(), 1);
    assert!(split_unit(&lone(), &resolved, None).is_some());
}

#[test]
fn resolutions_stay_qualities() {
    let videos = vec![
        link("https://cdn.example/1080.mp4", "1080p", Some(1080)),
        link("https://cdn.example/720.mp4", "720", Some(720)),
        link("https://cdn.example/480.mp4", "SD", Some(480)),
    ];
    let servers = vec![
        plain_server("a", "Server 1", Some(videos.clone())),
        plain_server("b", "Server 2", None),
    ];
    assert_eq!(split_unit(&lone(), &servers, Some(&videos)), None);
    assert_eq!(videos_of_episode(videos.clone(), "E1"), videos);
}

#[test]
fn answers_are_kept_once_and_servers_by_episode() {
    let memory = Memory::default();
    memory.keep_answer("p", "servers", "x", &serde_json::json!({ "servers": [] }));
    assert!(memory.take_answer("p", "videos", "x").is_none());
    assert!(memory.take_answer("p", "servers", "x").is_some());
    assert!(
        memory.take_answer("p", "servers", "x").is_none(),
        "taken once"
    );

    let units = from_lines(vec![line("A", &[("1", "u1")]), line("B", &[("1", "u2")])]);
    memory.keep_servers("p", &units);
    assert_eq!(memory.servers("p", "u1").unwrap().len(), 2);
    assert!(memory.servers("q", "u1").is_none());
}
