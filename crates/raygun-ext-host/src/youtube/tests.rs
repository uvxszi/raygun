//! YouTube answers (trimmed from real ones in tests/fixtures/youtube) read, and the order things
//! are asked in, against a fake network.

use std::cell::RefCell;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::parse::{self, Format};
use super::*;

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/youtube")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

fn json(name: &str) -> Value {
    serde_json::from_str(&fixture(name)).unwrap()
}

const VISITOR: &str = "CgtBNkRFQkhCZXZ2ZyiI8svXBjIKCgJVUxIEGgAgUw%3D%3D";

/// What `www.youtube.com/sw.js_data` answers, with the visitor token where YouTube puts it.
fn sw_data() -> String {
    format!(
        r#")]}}'
[["data",null,[[["a","b",1,null,null,null,null,null,null,null,null,null,null,"{VISITOR}"]]]]]"#
    )
}

/// A network that answers from `answer` and keeps what was sent.
struct Fake<F: Fn(&Request) -> (u16, String)> {
    answer: F,
    sent: RefCell<Vec<Request>>,
}

impl<F: Fn(&Request) -> (u16, String)> Fake<F> {
    fn new(answer: F) -> Self {
        Self {
            answer,
            sent: RefCell::new(Vec::new()),
        }
    }

    fn urls(&self) -> Vec<String> {
        self.sent
            .borrow()
            .iter()
            .map(
                |request| match request.body.as_deref().and_then(client_name) {
                    Some(client) => format!("{} {client}", request.url),
                    None => request.url.clone(),
                },
            )
            .collect()
    }
}

impl<F: Fn(&Request) -> (u16, String)> Http for Fake<F> {
    async fn send(&self, request: Request) -> Result<Response, String> {
        let (status, body) = (self.answer)(&request);
        self.sent.borrow_mut().push(request);
        Ok(Response { status, body })
    }
}

fn client_name(body: &str) -> Option<String> {
    let body: Value = serde_json::from_str(body).ok()?;
    body.pointer("/context/client/clientName")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn header<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

#[test]
fn song_rows_name_their_artists_album_and_length() {
    let songs = parse::music_search(&json("music-search-songs.json"), VideoKind::Song);
    assert_eq!(songs.len(), 2, "the row without a video is left out");
    let first = &songs[0];
    assert_eq!(first.id, "wU26xVT_vBU");
    assert_eq!(first.title, "One More Time (Radio Edit)");
    assert_eq!(first.channel.as_deref(), Some("Daft Punk"));
    assert_eq!(first.album.as_deref(), Some("One More Time"));
    assert_eq!(first.duration_ms, Some(321_000));
    assert_eq!(first.kind, VideoKind::Song);
    // The largest picture, upscaled to the player's size.
    assert_eq!(
        first.thumbnail_url.as_deref(),
        Some("https://yt3.googleusercontent.com/one=w544-h544-l90-rj")
    );
    assert_eq!(first.thumbnails.len(), 3);
    assert_eq!(
        songs[1].channel.as_deref(),
        Some("Daft Punk, Pharrell Williams")
    );
    assert_eq!(songs[1].duration_ms, Some(370_000));
}

#[test]
fn video_rows_name_their_uploader_without_counts_or_topic() {
    let videos = parse::music_search(&json("music-search-videos.json"), VideoKind::Video);
    let named: Vec<_> = videos
        .iter()
        .map(|video| {
            (
                video.id.as_str(),
                video.channel.as_deref(),
                video.duration_ms,
            )
        })
        .collect();
    assert_eq!(
        named,
        [
            ("FGBhQbmPwH8", Some("Daft Punk"), Some(322_000)),
            ("wU26xVT_vBU", Some("Daft Punk"), Some(321_000)),
            ("oFl5Dh5d8kU", Some("Jonny Ross Music"), Some(268_000)),
        ]
    );
    assert_eq!(
        videos[0].thumbnail_url.as_deref(),
        Some("https://i.ytimg.com/vi/FGBhQbmPwH8/sddefault.jpg"),
        "video pictures keep their address"
    );
}

#[test]
fn searches_are_answered_in_the_querys_script() {
    assert_eq!(clients::search_language("Daft Punk One More Time"), "en");
    assert_eq!(clients::search_language("Rosalía Despechá"), "en");
    assert_eq!(clients::search_language("米津玄師 \"Lemon\""), "ja");
    assert_eq!(clients::search_language("ヨルシカ ただ君に晴れ"), "ja");
    assert_eq!(clients::search_language("周杰倫 晴天"), "ja");
    assert_eq!(clients::search_language("아이유 좋은 날"), "ko");
    let request = clients::music_search(clients::MUSIC_VERSION, "아이유 좋은 날", clients::SONGS);
    let body: Value = serde_json::from_str(request.body.as_deref().unwrap()).unwrap();
    assert_eq!(body["context"]["client"]["hl"], "ko");
}

#[test]
fn clocks_and_topic_channels() {
    assert_eq!(parse::clock_ms("3:21"), Some(201_000));
    assert_eq!(parse::clock_ms(" 1:02:03 "), Some(3_723_000));
    assert_eq!(parse::clock_ms("LIVE"), None);
    assert_eq!(parse::without_topic("Daft Punk - Topic"), "Daft Punk");
    assert_eq!(parse::without_topic("Topical Storm"), "Topical Storm");
}

#[test]
fn player_answers_give_formats_details_and_refusals() {
    let answer = json("player.json");
    assert_eq!(parse::playability(&answer), parse::Playability::Ok);
    let formats = parse::formats(&answer);
    assert_eq!(formats.len(), 9);
    let opus = formats
        .iter()
        .find(|format| format.itag == 251 && !format.is_drc)
        .unwrap();
    assert_eq!(opus.kbps(), 150);
    assert_eq!(opus.container(), Some("webm"));
    assert_eq!(opus.codec().as_deref(), Some("opus"));
    assert_eq!(opus.content_length, Some(5_319_766));
    // A ciphered format has no plain address.
    assert!(
        formats
            .iter()
            .any(|format| format.itag == 258 && format.url.is_none())
    );
    let details = parse::details(&answer);
    assert_eq!(details.channel.as_deref(), Some("Daft Punk"));
    assert_eq!(details.duration_ms, Some(320_000));
    assert_eq!(details.expires_in_seconds, Some(21_540));

    let refused = |status: &str, reason: &str| {
        parse::playability(&serde_json::json!({
            "playabilityStatus": { "status": status, "reason": reason }
        }))
    };
    assert_eq!(
        refused("ERROR", "This video has been removed by the uploader"),
        parse::Playability::Gone(
            "YouTube can't play it: This video has been removed by the uploader".into()
        )
    );
    assert_eq!(
        refused("LOGIN_REQUIRED", "Sign in to confirm your age"),
        parse::Playability::Refused("YouTube wants a sign-in to play it (age-restricted)".into())
    );
    assert!(matches!(
        refused("LOGIN_REQUIRED", "Sign in to confirm you're not a bot"),
        parse::Playability::Refused(_)
    ));
}

fn picked(prefs: StreamPrefs) -> (u32, bool) {
    let formats = parse::formats(&json("player.json"));
    let format: &Format = pick_format(&formats, prefs).unwrap();
    (
        format.itag,
        format.url.as_deref().unwrap().contains("xtags"),
    )
}

#[test]
fn formats_are_picked_by_bitrate_and_what_decodes() {
    // The best there is: Opus, never the stable-volume or dubbed copies.
    assert_eq!(picked(StreamPrefs::default()), (251, false));
    // Nearest the target.
    let near = |kbps: f64| StreamPrefs {
        target_kbps: Some(kbps),
        ..StreamPrefs::default()
    };
    assert_eq!(picked(near(64.0)), (249, false));
    assert_eq!(picked(near(80.0)), (250, false));
    // Raygun decodes AAC itself; the original language's.
    let decodable = StreamPrefs {
        decodable: true,
        ..StreamPrefs::default()
    };
    assert_eq!(picked(decodable), (140, false));
    assert_eq!(
        picked(StreamPrefs {
            target_kbps: Some(48.0),
            decodable: true,
            ..StreamPrefs::default()
        }),
        (139, false)
    );
}

/// An audio format of a video with dubbed tracks, as YouTube lists them asked for in English.
fn track(itag: u32, kbps: u64, name: &str, default: bool, content: &str) -> serde_json::Value {
    serde_json::json!({
        "itag": itag,
        "url": format!("https://rr1---sn-x.googlevideo.com/videoplayback?itag={itag}&xtags=acont%3D{content}%3Alang%3Dx"),
        "mimeType": "audio/webm; codecs=\"opus\"",
        "bitrate": kbps * 1000,
        "audioTrack": { "displayName": name, "id": format!("{content}.{itag}"), "audioIsDefault": default }
    })
}

#[test]
fn a_dubbed_videos_original_audio_plays_unless_youtubes_default_is_wanted() {
    let answer = serde_json::json!({ "streamingData": { "adaptiveFormats": [
        track(251, 160, "English (United States) (dubbed-auto)", true, "dubbed-auto"),
        track(250, 70, "Spanish (Spain) original", false, "original"),
        track(249, 50, "Spanish (Spain) descriptive", false, "descriptive"),
    ] } });
    let formats = parse::formats(&answer);
    assert!(formats[1].original && !formats[1].dubbed);
    assert!(formats[0].dubbed && !formats[0].original);
    assert!(formats[2].descriptive);
    let pick = |prefs: StreamPrefs| pick_format(&formats, prefs).unwrap().itag;
    assert_eq!(pick(StreamPrefs::default()), 250);
    assert_eq!(
        pick(StreamPrefs {
            default_track: true,
            ..StreamPrefs::default()
        }),
        251
    );

    // Only the tags say which is the original; the names are plain.
    let answer = serde_json::json!({ "streamingData": { "adaptiveFormats": [
        track(251, 160, "English", true, "dubbed"),
        track(250, 70, "Japanese", false, "original"),
    ] } });
    let formats = parse::formats(&answer);
    assert_eq!(
        pick_format(&formats, StreamPrefs::default()).unwrap().itag,
        250
    );
}

#[test]
fn ids_come_from_addresses_and_ids() {
    for text in [
        "wU26xVT_vBU",
        "https://www.youtube.com/watch?v=wU26xVT_vBU&list=PL123",
        "music.youtube.com/watch?v=wU26xVT_vBU",
        "https://youtu.be/wU26xVT_vBU?t=3",
        "https://m.youtube.com/shorts/wU26xVT_vBU",
    ] {
        assert_eq!(video_id(text).as_deref(), Some("wU26xVT_vBU"), "{text}");
    }
    assert_eq!(video_id("https://example.com/watch?v=wU26xVT_vBU"), None);
    assert_eq!(video_id("not a video"), None);
    for text in [
        "PLSbDLF8wQ3oIkmgwFI3HmEYV-qhLwLVHx",
        "VLPLSbDLF8wQ3oIkmgwFI3HmEYV-qhLwLVHx",
        "https://www.youtube.com/playlist?list=PLSbDLF8wQ3oIkmgwFI3HmEYV-qhLwLVHx",
        "https://music.youtube.com/watch?v=x&list=PLSbDLF8wQ3oIkmgwFI3HmEYV-qhLwLVHx",
    ] {
        assert_eq!(
            playlist_id(text).as_deref(),
            Some("PLSbDLF8wQ3oIkmgwFI3HmEYV-qhLwLVHx"),
            "{text}"
        );
    }
    assert_eq!(
        playlist_id("https://example.com/playlist?list=PL12345678"),
        None
    );
}

#[test]
fn visitor_data_is_read_from_the_service_worker_data() {
    assert_eq!(
        parse::visitor_in_sw_data(&sw_data()).as_deref(),
        Some(VISITOR)
    );
    // Moved elsewhere in the data: found anyway.
    let moved = format!(r#")]}}'[["x",["{VISITOR}"]]]"#);
    assert_eq!(parse::visitor_in_sw_data(&moved).as_deref(), Some(VISITOR));
    assert_eq!(parse::visitor_in_sw_data("<html>"), None);
}

#[test]
fn playlist_pages_list_videos_and_the_next_page() {
    let first = json("playlist.json");
    let (title, author) = parse::playlist_header(&first);
    assert_eq!(
        title.as_deref(),
        Some("Daft Punk - Mix Clips | Vevo Playlist")
    );
    assert_eq!(author.as_deref(), Some("Daft Punk"));
    let videos = parse::playlist_videos(&first);
    let named: Vec<_> = videos
        .iter()
        .map(|video| {
            (
                video.id.as_str(),
                video.channel.as_deref(),
                video.duration_ms,
            )
        })
        .collect();
    assert_eq!(
        named,
        [
            ("imBlPXbAv6E", Some("Daft Punk"), Some(260_000)),
            ("A2VpR8HahKc", Some("Daft Punk"), Some(3_723_000)),
        ],
        "playlists inside are no videos"
    );
    assert_eq!(
        videos[0].title,
        "Daft Punk - Infinity Repeating (2013 Demo) (feat. Julian Casablancas+The Voidz)"
    );
    assert_eq!(
        videos[0].thumbnail_url.as_deref(),
        Some("https://i.ytimg.com/vi/imBlPXbAv6E/hqdefault.jpg")
    );
    // The item list's page, not the sections'.
    assert_eq!(parse::next_page(&first).as_deref(), Some("ITEMS-PAGE-2"));

    let next = json("playlist-next.json");
    let more = parse::playlist_videos(&next);
    let named: Vec<_> = more
        .iter()
        .map(|video| {
            (
                video.id.as_str(),
                video.channel.as_deref(),
                video.duration_ms,
            )
        })
        .collect();
    assert_eq!(
        named,
        [
            ("A2VpR8HahKc", Some("Daft Punk"), None),
            ("gAjR4_CbPpQ", Some("Daft Punk"), Some(225_000)),
            ("zzzzzzzzzzz", None, None),
        ]
    );
    assert_eq!(parse::next_page(&next), None);
}

#[test]
fn newer_playlist_headers_and_mixes() {
    let page = serde_json::json!({
        "header": { "pageHeaderRenderer": {
            "pageTitle": "Road trip",
            "content": { "pageHeaderViewModel": { "metadata": { "contentMetadataViewModel": {
                "metadataRows": [{ "metadataParts": [{ "avatarStack": { "avatarStackViewModel": {
                    "text": { "content": "by Someone - Topic" } } } }] }]
            } } } }
        } }
    });
    assert_eq!(
        parse::playlist_header(&page),
        (Some("Road trip".into()), Some("Someone".into()))
    );
    // A mix's watch page, as `next` answers it.
    let watch = serde_json::json!({ "contents": { "twoColumnWatchNextResults": {
        "playlist": { "playlist": {
            "title": "Mix - Daft Punk",
            "ownerName": { "simpleText": "YouTube" },
            "playlistId": "RDwU26xVT_vBU",
            "contents": [
                { "playlistPanelVideoRenderer": {
                    "videoId": "FGBhQbmPwH8", "title": { "simpleText": "One More Time" },
                    "longBylineText": { "runs": [{ "text": "Daft Punk" }] },
                    "lengthText": { "simpleText": "5:22" } } }
            ]
        } },
        "secondaryResults": { "secondaryResults": { "results": [
            { "lockupViewModel": { "contentId": "aaaaaaaaaaa", "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
              "metadata": { "lockupMetadataViewModel": { "title": { "content": "Related" } } } } }
        ] } }
    } } });
    let videos = parse::mix_videos(&watch);
    assert_eq!(videos.len(), 1, "related videos aren't the mix's");
    assert_eq!(videos[0].channel.as_deref(), Some("Daft Punk"));
    assert_eq!(videos[0].duration_ms, Some(322_000));
    assert_eq!(
        parse::mix_header(&watch),
        (Some("Mix - Daft Punk".into()), Some("YouTube".into()))
    );
}

/// Answers like YouTube does today: ANDROID_VR's files stop after their first megabyte,
/// VISIONOS's play to the end.
fn youtube_today(request: &Request) -> (u16, String) {
    if request.url.ends_with("/sw.js_data") {
        return (200, sw_data());
    }
    if request.url.contains("/youtubei/v1/player") {
        let client = request.body.as_deref().and_then(client_name).unwrap();
        return (200, fixture("player.json").replace("CLIENT", &client));
    }
    if request.url.contains("googlevideo.com") {
        let range = header(request, "Range").unwrap_or_default();
        let start: u64 = range
            .trim_start_matches("bytes=")
            .split('-')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let gated = request.url.contains("c=ANDROID_VR") || request.url.contains("c=IOS");
        return if gated && start >= 1_000_000 {
            (403, String::new())
        } else {
            (206, "ab".into())
        };
    }
    (404, String::new())
}

#[tokio::test]
async fn streams_come_from_the_first_client_whose_file_plays_to_its_end() {
    let youtube = YouTube::new(None);
    let http = Fake::new(youtube_today);
    let stream = youtube
        .stream(
            &http,
            "https://youtu.be/wU26xVT_vBU",
            StreamPrefs::default(),
        )
        .await
        .unwrap();
    assert_eq!(stream.client, "VISIONOS");
    assert!(stream.url.contains("c=VISIONOS") && stream.url.contains("itag=251"));
    assert_eq!(
        stream.headers.get("User-Agent").map(String::as_str),
        Some(clients::PLAYER_CLIENTS[1].user_agent)
    );
    assert_eq!(stream.container.as_deref(), Some("webm"));
    assert_eq!(stream.codec.as_deref(), Some("opus"));
    assert_eq!(stream.bitrate_kbps, Some(150));
    assert_eq!(stream.duration_ms, Some(320_000));
    assert_eq!(stream.channel.as_deref(), Some("Daft Punk"));
    let expires = stream.expires_at.unwrap() - now_ms();
    assert!((21_000_000..=21_540_000).contains(&expires), "{expires}");
    assert_eq!(
        http.urls(),
        [
            "https://www.youtube.com/sw.js_data".to_owned(),
            "https://www.youtube.com/youtubei/v1/player?prettyPrint=false ANDROID_VR".to_owned(),
            "https://rr2---sn-x.googlevideo.com/videoplayback?expire=1794325370&c=ANDROID_VR&itag=251"
                .to_owned(),
            "https://www.youtube.com/youtubei/v1/player?prettyPrint=false VISIONOS".to_owned(),
            "https://rr2---sn-x.googlevideo.com/videoplayback?expire=1794325370&c=VISIONOS&itag=251"
                .to_owned(),
        ]
    );
    {
        let sent = http.sent.borrow();
        // The file's last two bytes, asked as the client.
        assert_eq!(header(&sent[2], "Range"), Some("bytes=5319764-5319765"));
        assert_eq!(
            header(&sent[2], "User-Agent"),
            Some(clients::PLAYER_CLIENTS[0].user_agent)
        );
        // The player request carries the visitor data.
        assert_eq!(header(&sent[1], "X-Goog-Visitor-Id"), Some(VISITOR));
        let body: Value = serde_json::from_str(sent[1].body.as_deref().unwrap()).unwrap();
        assert_eq!(body["videoId"], "wU26xVT_vBU");
        assert_eq!(body["context"]["client"]["visitorData"], VISITOR);
        assert_eq!(body["context"]["client"]["clientVersion"], "1.65.10");
    }

    // The client that worked goes first from then on; the visitor data is kept.
    let http = Fake::new(youtube_today);
    let again = youtube
        .stream(&http, "wU26xVT_vBU", StreamPrefs::default())
        .await
        .unwrap();
    assert_eq!(again.client, "VISIONOS");
    assert_eq!(
        http.urls()[0],
        "https://www.youtube.com/youtubei/v1/player?prettyPrint=false VISIONOS"
    );
    assert_eq!(http.urls().len(), 2);
}

#[tokio::test]
async fn a_removed_video_fails_at_once() {
    let youtube = YouTube::new(None);
    let http = Fake::new(|request: &Request| {
        if request.url.ends_with("/sw.js_data") {
            return (200, sw_data());
        }
        (
            200,
            r#"{"playabilityStatus":{"status":"ERROR","reason":"Video unavailable"}}"#.into(),
        )
    });
    let error = youtube
        .stream(&http, "wU26xVT_vBU", StreamPrefs::default())
        .await
        .unwrap_err();
    assert_eq!(error, "YouTube can't play it: Video unavailable");
    assert_eq!(http.sent.borrow().len(), 2, "no other client is asked");
}

#[tokio::test]
async fn streams_rest_after_videos_in_a_row_gave_none() {
    let youtube = YouTube::new(None);
    // Every client's files stop early.
    let gated = |request: &Request| {
        if request.url.contains("googlevideo.com") {
            return (403, String::new());
        }
        youtube_today(request)
    };
    for _ in 0..FAILURES_TO_REST {
        let http = Fake::new(gated);
        let error = youtube
            .stream(&http, "wU26xVT_vBU", StreamPrefs::default())
            .await
            .unwrap_err();
        assert_eq!(error, "YouTube didn't give a stream for it");
        let asked = http
            .urls()
            .iter()
            .filter(|url| url.contains("/youtubei/v1/player"))
            .count();
        assert_eq!(asked, PLAYER_CLIENTS.len(), "every client is asked");
    }
    let http = Fake::new(gated);
    let error = youtube
        .stream(&http, "wU26xVT_vBU", StreamPrefs::default())
        .await
        .unwrap_err();
    assert_eq!(error, "YouTube isn't giving streams right now");
    assert!(http.sent.borrow().is_empty());

    // A video YouTube refuses (age-restricted) says why, and isn't held against the clients.
    let youtube = YouTube::new(None);
    for _ in 0..=FAILURES_TO_REST {
        let http = Fake::new(|request: &Request| {
            if request.url.contains("/youtubei/v1/player") {
                return (
                    200,
                    r#"{"playabilityStatus":{"status":"LOGIN_REQUIRED","reason":"Sign in to confirm your age"}}"#
                        .into(),
                );
            }
            youtube_today(request)
        });
        let error = youtube
            .stream(&http, "wU26xVT_vBU", StreamPrefs::default())
            .await
            .unwrap_err();
        assert_eq!(error, "YouTube wants a sign-in to play it (age-restricted)");
    }
}

/// yt-dlp as the app runs it, answering from `printed` and keeping its arguments.
fn fake_ytdlp(printed: &'static str, asked: Arc<Mutex<Vec<Vec<String>>>>) -> YtDlp {
    Arc::new(move |args| {
        asked.lock().unwrap().push(args);
        Box::pin(async move { Ok(printed.to_owned()) })
    })
}

#[tokio::test]
async fn yt_dlp_answers_when_youtube_gives_nothing() {
    const PRINTED: &str = r#"{"id":"wU26xVT_vBU","title":"One More Time (Radio Edit)","duration":320.0,"url":"https://rr1---sn-y.googlevideo.com/videoplayback?expire=4102444800&itag=251","ext":"webm","acodec":"opus","abr":135.2,"channel":"Daft Punk - Topic","artists":["Daft Punk"],"album":"One More Time","upload_date":"20000101","http_headers":{"User-Agent":"Mozilla/5.0 yt-dlp"}}"#;
    let asked = Arc::new(Mutex::new(Vec::new()));
    let youtube = YouTube::new(Some(fake_ytdlp(PRINTED, asked.clone())));
    let http = Fake::new(|_: &Request| (500, String::new()));
    let stream = youtube
        .stream(&http, "wU26xVT_vBU", StreamPrefs::default())
        .await
        .unwrap();
    assert_eq!(stream.client, "yt-dlp");
    assert_eq!(stream.bitrate_kbps, Some(135));
    assert_eq!(stream.expires_at, Some(4_102_444_800_000));
    assert_eq!(stream.channel.as_deref(), Some("Daft Punk"));
    assert_eq!(stream.artists, ["Daft Punk"]);
    assert_eq!(
        stream.headers.get("User-Agent").map(String::as_str),
        Some("Mozilla/5.0 yt-dlp")
    );
    let asked = asked.lock().unwrap();
    let args = &asked[0];
    // The address comes after `--`, so nothing a source passes is read as an option.
    assert_eq!(args[args.len() - 2], "--");
    assert_eq!(
        args[args.len() - 1],
        "https://www.youtube.com/watch?v=wU26xVT_vBU"
    );
}

#[test]
fn yt_dlp_lists_are_read() {
    let search = ytdlp::parse_search(
        r#"{"id":"FGBhQbmPwH8","title":"Daft Punk - One More Time (Official Video)","duration":322,"channel":"Daft Punk","thumbnails":[{"url":"https://i.ytimg.com/vi/FGBhQbmPwH8/hqdefault.jpg","height":360,"width":480}]}
not json
{"id":"oFl5Dh5d8kU","title":"Cover","duration":null,"uploader":"Jonny Ross Music - Topic"}"#,
    );
    assert_eq!(search.len(), 2);
    assert_eq!(search[0].duration_ms, Some(322_000));
    assert_eq!(
        search[0].thumbnail_url.as_deref(),
        Some("https://i.ytimg.com/vi/FGBhQbmPwH8/hqdefault.jpg")
    );
    assert_eq!(search[1].channel.as_deref(), Some("Jonny Ross Music"));

    let playlist = ytdlp::parse_playlist(
        r#"{"id":"FGBhQbmPwH8","title":"One More Time","duration":322,"channel":"Daft Punk","playlist_title":"Mix Clips","playlist_uploader":"Daft Punk"}
{"id":"gAjR4_CbPpQ","title":"Harder","channel":null}"#,
        "PLSbDLF8wQ3oIkmgwFI3HmEYV-qhLwLVHx",
    )
    .unwrap();
    assert_eq!(playlist.title, "Mix Clips");
    assert_eq!(playlist.author.as_deref(), Some("Daft Punk"));
    assert_eq!(playlist.videos.len(), 2);
    assert_eq!(playlist.videos[1].channel, None);
    assert!(ytdlp::parse_playlist("", "PL1234567890").is_err());
}

/// Answers a search: songs for the songs chip, videos for the videos chip.
fn music_today(request: &Request) -> (u16, String) {
    let body = request.body.as_deref().unwrap_or_default();
    if body.contains(clients::SONGS) {
        (200, fixture("music-search-songs.json"))
    } else if body.contains(clients::VIDEOS) {
        (200, fixture("music-search-videos.json"))
    } else {
        (404, String::new())
    }
}

#[tokio::test]
async fn searches_give_songs_then_videos_once_each() {
    let youtube = YouTube::new(None);
    let http = Fake::new(music_today);
    let found = youtube
        .search(&http, "Daft Punk \"One More Time\"", Some(4))
        .await
        .unwrap();
    let ids: Vec<_> = found
        .iter()
        .map(|video| (video.id.as_str(), video.kind))
        .collect();
    assert_eq!(
        ids,
        [
            ("wU26xVT_vBU", VideoKind::Song),
            ("h5EofwRzit0", VideoKind::Song),
            ("FGBhQbmPwH8", VideoKind::Video),
            ("oFl5Dh5d8kU", VideoKind::Video),
        ],
        "the song's own video isn't listed twice"
    );
    {
        let sent = http.sent.borrow();
        assert_eq!(sent.len(), 2);
        assert!(sent.iter().all(|request| {
            request.url == "https://music.youtube.com/youtubei/v1/search?prettyPrint=false"
                && header(request, "X-YouTube-Client-Name") == Some("67")
        }));
        let body: Value = serde_json::from_str(sent[0].body.as_deref().unwrap()).unwrap();
        assert_eq!(body["query"], "Daft Punk \"One More Time\"");
    }

    // One chip failing still gives the other's.
    let http = Fake::new(|request: &Request| {
        if request
            .body
            .as_deref()
            .unwrap_or_default()
            .contains(clients::SONGS)
        {
            (500, String::new())
        } else {
            music_today(request)
        }
    });
    let found = youtube.search(&http, "one more time", None).await.unwrap();
    assert_eq!(found.len(), 3);

    let http = Fake::new(|_: &Request| (429, String::new()));
    let error = youtube.search(&http, "x", None).await.unwrap_err();
    assert!(error.contains("limiting requests"), "{error}");
}

#[tokio::test]
async fn playlists_are_read_page_by_page_in_one_session() {
    let youtube = YouTube::new(None);
    let http = Fake::new(|request: &Request| {
        if request.url.ends_with("/sw.js_data") {
            return (200, sw_data());
        }
        let body = request.body.as_deref().unwrap_or_default();
        if body.contains("\"browseId\":\"VLPLSbDLF8wQ3oIkmgwFI3HmEYV-qhLwLVHx\"") {
            (200, fixture("playlist.json"))
        } else if body.contains("\"continuation\":\"ITEMS-PAGE-2\"") {
            (200, fixture("playlist-next.json"))
        } else {
            (404, String::new())
        }
    });
    let playlist = youtube
        .playlist(
            &http,
            "https://www.youtube.com/playlist?list=PLSbDLF8wQ3oIkmgwFI3HmEYV-qhLwLVHx",
        )
        .await
        .unwrap();
    assert_eq!(playlist.id, "PLSbDLF8wQ3oIkmgwFI3HmEYV-qhLwLVHx");
    assert_eq!(playlist.title, "Daft Punk - Mix Clips | Vevo Playlist");
    let ids: Vec<_> = playlist
        .videos
        .iter()
        .map(|video| video.id.as_str())
        .collect();
    assert_eq!(
        ids,
        ["imBlPXbAv6E", "A2VpR8HahKc", "gAjR4_CbPpQ", "zzzzzzzzzzz"],
        "a video on both pages is listed once"
    );
    let sent = http.sent.borrow();
    assert_eq!(sent.len(), 3);
    // Further pages need the session's visitor data, or YouTube answers the first again.
    let next: Value = serde_json::from_str(sent[2].body.as_deref().unwrap()).unwrap();
    assert_eq!(next["context"]["client"]["visitorData"], VISITOR);
    assert_eq!(header(&sent[2], "X-Goog-Visitor-Id"), Some(VISITOR));
}
