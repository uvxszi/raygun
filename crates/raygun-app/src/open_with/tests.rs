use std::path::PathBuf;

use super::playlist::{self, Base, Entry};
use super::route::{self, Kind, Location, StreamFormat, TextTarget};
use super::{Handed, Handover, KEPT_COPIES, argument, copy_key, store_copy};

const HLS: Kind = Kind::Streaming(StreamFormat::Hls);
const DASH: Kind = Kind::Streaming(StreamFormat::Dash);

#[test]
fn documents_go_by_their_type_then_their_name() {
    let document = "content://com.android.providers.media.documents/document/video%3A42";
    assert_eq!(
        route::kind_of(document, Some("video/mp4"), None),
        Kind::Video
    );
    assert_eq!(
        route::kind_of(document, Some("audio/mpeg"), None),
        Kind::Audio
    );
    assert_eq!(
        route::kind_of(document, Some("audio/flac; charset=binary"), None),
        Kind::Audio
    );
    // A file manager that doesn't know the type: the name says.
    assert_eq!(
        route::kind_of(document, Some("application/octet-stream"), Some("Film.MKV")),
        Kind::Video
    );
    assert_eq!(
        route::kind_of(document, None, Some("song.flac")),
        Kind::Audio
    );
    assert_eq!(
        route::kind_of(document, Some("*/*"), Some("Mix.m3u")),
        Kind::Playlist
    );
    // Containers for either: the name decides, else what they usually hold.
    assert_eq!(
        route::kind_of(document, Some("application/ogg"), Some("clip.ogv")),
        Kind::Video
    );
    assert_eq!(
        route::kind_of(document, Some("application/ogg"), None),
        Kind::Audio
    );
    assert_eq!(
        route::kind_of(document, Some("application/mp4"), Some("book.m4a")),
        Kind::Audio
    );
    assert_eq!(
        route::kind_of(document, Some("image/jpeg"), Some("cover.jpg")),
        Kind::Other
    );
    assert_eq!(route::kind_of(document, None, None), Kind::Other);
}

#[test]
fn a_documents_address_can_name_it() {
    let named =
        "content://com.android.externalstorage.documents/document/primary%3AMusic%2FTrip.m3u";
    assert_eq!(route::kind_of(named, None, None), Kind::Playlist);
    assert_eq!(
        route::kind_of(
            "content://media/external/audio/media/42",
            Some("audio/x-wav"),
            None
        ),
        Kind::Audio
    );
}

#[test]
fn playlists_and_streams_are_told_apart() {
    // From the web, `.m3u8` and Apple's type are HLS; a file of either is read first.
    assert_eq!(
        route::kind_of("https://cdn.example/live/master.m3u8?token=1", None, None),
        HLS
    );
    assert_eq!(
        route::kind_of(
            "https://cdn.example/live",
            Some("application/vnd.apple.mpegurl"),
            None
        ),
        HLS
    );
    assert_eq!(
        route::kind_of(
            "https://cdn.example/live.m3u8",
            Some("application/x-mpegURL"),
            None
        ),
        HLS
    );
    assert_eq!(
        route::kind_of(
            "https://radio.example/station.m3u",
            Some("audio/x-mpegurl"),
            None
        ),
        Kind::Playlist
    );
    assert_eq!(
        route::kind_of("https://radio.example/listen.pls", None, None),
        Kind::Playlist
    );
    assert_eq!(
        route::kind_of(
            "content://downloads/1",
            Some("application/vnd.apple.mpegurl"),
            Some("list.m3u8")
        ),
        Kind::Playlist
    );
    assert_eq!(
        route::kind_of("https://cdn.example/a/manifest.mpd", None, None),
        DASH
    );
    assert_eq!(
        route::kind_of(
            "https://cdn.example/a/manifest",
            Some("application/dash+xml"),
            None
        ),
        DASH
    );
}

#[test]
fn web_addresses_go_by_their_path() {
    assert_eq!(
        route::kind_of("https://cdn.example/v1.2/movie.mp4?x=1#t=3", None, None),
        Kind::Video
    );
    assert_eq!(
        route::kind_of("HTTP://cdn.example/song.MP3", None, None),
        Kind::Audio
    );
    assert_eq!(
        route::kind_of("https://example.com/watch?v=abc", None, None),
        Kind::Other
    );
    assert_eq!(
        route::kind_of("https://example.com/", None, None),
        Kind::Other
    );
    // `.ts` on the web is as likely a script; on the device it's a recording.
    assert_eq!(
        route::kind_of("https://example.com/app.ts", None, None),
        Kind::Other
    );
    assert_eq!(
        route::kind_of("file:///storage/emulated/0/DCIM/rec.ts", None, None),
        Kind::Video
    );
    assert_eq!(
        route::kind_of("mailto:someone@example.com", None, None),
        Kind::Other
    );
    assert_eq!(
        route::kind_of("not an address.mp3", None, None),
        Kind::Other
    );
}

#[test]
fn addresses_are_located() {
    assert_eq!(
        route::location("HTTPS://Example.com/a.mp4"),
        Some(Location::Web("https://Example.com/a.mp4".into()))
    );
    assert_eq!(
        route::location("file:///storage/emulated/0/Music/My%20Song.mp3"),
        Some(Location::File(PathBuf::from(
            "/storage/emulated/0/Music/My Song.mp3"
        )))
    );
    assert_eq!(
        route::location("file://localhost/sdcard/a.mkv"),
        Some(Location::File(PathBuf::from("/sdcard/a.mkv")))
    );
    assert_eq!(
        route::file_path("file:///C:/Users/me/Videos/a%20b.mp4"),
        Some(PathBuf::from("C:/Users/me/Videos/a b.mp4"))
    );
    // Another computer's file.
    assert_eq!(route::location("file://server/share/a.mp4"), None);
    assert_eq!(
        route::location("content://media/external/video/media/7"),
        Some(Location::Document(
            "content://media/external/video/media/7".into()
        ))
    );
    assert_eq!(route::location("ftp://example.com/a.mp3"), None);
}

#[cfg(windows)]
#[test]
fn windows_paths_are_files() {
    assert_eq!(
        route::kind_of(r"C:\Music\Album\01 Intro.flac", None, None),
        Kind::Audio
    );
    assert_eq!(
        route::location(r"C:\Videos\a.mp4"),
        Some(Location::File(PathBuf::from(r"C:\Videos\a.mp4")))
    );
}

#[test]
fn shared_text_goes_to_its_link_or_a_search() {
    assert_eq!(
        route::text_target("Watch this! https://youtu.be/dQw4w9WgXcQ?si=abc."),
        Some(TextTarget::Search(
            "https://youtu.be/dQw4w9WgXcQ?si=abc".into()
        ))
    );
    assert_eq!(
        route::text_target("(stream: https://cdn.example/live/master.m3u8)"),
        Some(TextTarget::Media {
            url: "https://cdn.example/live/master.m3u8".into(),
            kind: HLS,
        })
    );
    assert_eq!(
        route::text_target("https://cdn.example/ep/episode-12.mp3"),
        Some(TextTarget::Media {
            url: "https://cdn.example/ep/episode-12.mp3".into(),
            kind: Kind::Audio,
        })
    );
    assert_eq!(
        route::text_target("My mix https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M?si=1"),
        Some(TextTarget::Import(
            "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M?si=1".into()
        ))
    );
    assert_eq!(
        route::text_target("  lo-fi   beats\n to study  "),
        Some(TextTarget::Search("lo-fi beats to study".into()))
    );
    assert_eq!(route::text_target(" \n "), None);
    let long = "word ".repeat(100);
    let Some(TextTarget::Search(query)) = route::text_target(&long) else {
        panic!("words are searched");
    };
    assert!(query.chars().count() <= 200);
}

#[test]
fn links_keep_their_own_brackets() {
    assert_eq!(
        route::first_link("see https://en.example.org/wiki/Song_(band)."),
        Some("https://en.example.org/wiki/Song_(band)".into())
    );
    assert_eq!(route::first_link("just https://"), None);
    assert_eq!(route::first_link("no link here"), None);
}

#[test]
fn names_and_titles() {
    assert_eq!(
        route::title_of(Some("Holiday 2024.mp4"), "content://x/1"),
        Some("Holiday 2024".into())
    );
    assert_eq!(
        route::title_of(None, "https://cdn.example/show/Episode%201.mp4?x=1"),
        Some("Episode 1".into())
    );
    assert_eq!(
        route::title_of(None, "file:///storage/emulated/0/Movies/Trip.mkv"),
        Some("Trip".into())
    );
    assert_eq!(
        route::title_of(None, "content://media/external/video/media/42"),
        None
    );
    assert_eq!(
        route::shown_name(None, "https://cdn.example/a/b%20c.mp3?x=1"),
        "b c.mp3"
    );
    assert_eq!(
        route::shown_name(Some(" Song.mp3 "), "content://x/1"),
        "Song.mp3"
    );
    assert_eq!(route::shown_name(None, "content://x/1"), "content://x/1");
}

#[test]
fn copies_are_named_safely_with_an_extension() {
    assert_eq!(
        route::copy_name(Some("Live: Part 1/2?.mp3"), Some("audio/mpeg")),
        "Live_ Part 1_2_.mp3"
    );
    assert_eq!(
        route::copy_name(Some("track"), Some("audio/flac")),
        "track.flac"
    );
    assert_eq!(
        route::copy_name(Some("voice.note"), Some("audio/ogg")),
        "voice.note.ogg"
    );
    assert_eq!(route::copy_name(Some("..."), None), "Song.mp3");
    assert_eq!(route::copy_name(None, Some("audio/mp4")), "Song.m4a");
    let long = format!("{}.opus", "a".repeat(300));
    let named = route::copy_name(Some(&long), None);
    assert!(named.ends_with(".opus") && named.chars().count() <= 105);
}

#[test]
fn m3u_playlists_keep_their_names_and_order() {
    let text = "\u{feff}#EXTM3U\r\n#EXTINF:215,Daft Punk - One More Time\r\nMusic/01.mp3\r\n\r\n# a comment\r\n#EXTINF:-1,Radio One\r\nhttp://radio.example:8000/live\r\nplain.flac\r\n";
    assert_eq!(
        playlist::entries(text),
        vec![
            Entry {
                location: "Music/01.mp3".into(),
                title: Some("One More Time".into()),
                artist: Some("Daft Punk".into()),
                duration_ms: Some(215_000),
            },
            Entry {
                location: "http://radio.example:8000/live".into(),
                title: Some("Radio One".into()),
                artist: None,
                duration_ms: None,
            },
            Entry {
                location: "plain.flac".into(),
                ..Entry::default()
            },
        ]
    );
}

#[test]
fn iptv_attributes_may_hold_commas() {
    let text = "#EXTM3U\n#EXTINF:-1 tvg-name=\"News, Live\" group-title=\"TV\",News Channel\nhttps://tv.example/news.m3u8\n";
    let entries = playlist::entries(text);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title.as_deref(), Some("News Channel"));
}

#[test]
fn pls_playlists_go_by_number() {
    let text = "[playlist]\nFile2=https://radio.example/two\nTitle2=Second\nFile1=https://radio.example/one.mp3\nTitle1=Artist - First\nLength1=-1\nLength2=180\nNumberOfEntries=2\nVersion=2\n";
    assert_eq!(
        playlist::entries(text),
        vec![
            Entry {
                location: "https://radio.example/one.mp3".into(),
                title: Some("First".into()),
                artist: Some("Artist".into()),
                duration_ms: None,
            },
            Entry {
                location: "https://radio.example/two".into(),
                title: Some("Second".into()),
                artist: None,
                duration_ms: Some(180_000),
            },
        ]
    );
}

#[test]
fn hls_is_told_from_a_list_of_songs() {
    assert!(playlist::is_hls(
        "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=800000\nlow/index.m3u8\n"
    ));
    assert!(playlist::is_hls(
        "#EXTM3U\n#EXT-X-TARGETDURATION:6\n#EXTINF:6.0,\nseg0.ts\n"
    ));
    assert!(!playlist::is_hls("#EXTM3U\n#EXTINF:200,A - B\nsong.mp3\n"));
}

#[test]
fn entries_are_found_beside_their_playlist() {
    let web = Base::Web(url::Url::parse("https://radio.example/lists/all.m3u").unwrap());
    assert_eq!(
        playlist::resolve("../streams/one.mp3", &web),
        Some(Location::Web(
            "https://radio.example/streams/one.mp3".into()
        ))
    );
    assert_eq!(
        playlist::resolve("https://other.example/two.aac", &web),
        Some(Location::Web("https://other.example/two.aac".into()))
    );
    let folder = Base::Folder(PathBuf::from("/storage/emulated/0/Music"));
    assert_eq!(
        playlist::resolve(r"Album\01 Intro.mp3", &folder),
        Some(Location::File(PathBuf::from(
            "/storage/emulated/0/Music/Album/01 Intro.mp3"
        )))
    );
    assert_eq!(
        playlist::resolve("./02.mp3", &folder),
        Some(Location::File(PathBuf::from(
            "/storage/emulated/0/Music/02.mp3"
        )))
    );
    assert_eq!(playlist::resolve("03.mp3", &Base::Nowhere), None);
}

#[test]
fn shared_documents_have_paths_in_shared_storage() {
    assert_eq!(
        crate::local_files::document_path(
            "content://com.android.externalstorage.documents/document/primary%3AMusic%2FTrip.m3u"
        )
        .as_deref(),
        Some("/storage/emulated/0/Music/Trip.m3u")
    );
    assert_eq!(
        crate::local_files::document_path(
            "content://com.android.providers.downloads.documents/document/raw%3A%2Fstorage%2Femulated%2F0%2FDownload%2Fa.pls"
        )
        .as_deref(),
        Some("/storage/emulated/0/Download/a.pls")
    );
    assert_eq!(
        crate::local_files::document_path(
            "content://com.android.providers.downloads.documents/document/msf%3A1000"
        ),
        None
    );
    // Folders still map as before.
    assert_eq!(
        crate::local_files::tree_path(
            "content://com.android.externalstorage.documents/tree/primary%3AMusic"
        )
        .as_deref(),
        Some("/storage/emulated/0/Music")
    );
}

#[test]
fn the_plugins_handover_reads() {
    let handover: Handover = serde_json::from_str(
        r#"{"items":[{"uri":"content://x/1","mime":"audio/mpeg","name":"a.mp3","size":12}],"text":"hi"}"#,
    )
    .unwrap();
    assert_eq!(handover.items.len(), 1);
    assert_eq!(handover.items[0].kind(), Kind::Audio);
    assert_eq!(handover.items[0].size, Some(12));
    assert_eq!(handover.text.as_deref(), Some("hi"));
    assert_eq!(handover.subject, None);
    let bare: Handover = serde_json::from_str(r#"{"items":[]}"#).unwrap();
    assert!(bare.items.is_empty() && bare.text.is_none());
}

#[test]
fn command_line_files_and_addresses() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("song.mp3"), b"ID3").unwrap();
    let relative = argument("song.mp3", dir.path()).expect("a file beside the working folder");
    assert_eq!(relative.name.as_deref(), Some("song.mp3"));
    assert_eq!(relative.kind(), Kind::Audio);
    let absolute = dir.path().join("song.mp3").to_string_lossy().into_owned();
    assert!(argument(&format!("\"{absolute}\""), dir.path()).is_some());
    assert!(argument("missing.mp3", dir.path()).is_none());
    assert!(argument("--flag", dir.path()).is_none());
    let web = argument("https://cdn.example/live.m3u8", dir.path()).unwrap();
    assert_eq!(web.kind(), HLS);
}

#[test]
fn copies_are_kept_for_the_same_document_and_pruned() {
    let root = tempfile::tempdir().unwrap();
    let key = copy_key("content://x/1", Some(3));
    assert_eq!(key, copy_key("content://x/1", Some(3)));
    assert_ne!(key, copy_key("content://x/1", Some(4)));
    let source = root.path().join("source.bin");
    std::fs::write(&source, b"abc").unwrap();
    let open = || std::fs::File::open(&source).map_err(|error| error.to_string());
    let copy = store_copy(&root.path().join("opened"), &key, "a.mp3", Some(3), open).unwrap();
    assert_eq!(std::fs::read(&copy).unwrap(), b"abc");
    // Opened again: the copy is used, nothing is read.
    let again = store_copy(&root.path().join("opened"), &key, "a.mp3", Some(3), || {
        Err("read again".into())
    })
    .unwrap();
    assert_eq!(again, copy);
    // A failed copy leaves nothing behind.
    assert!(
        store_copy(&root.path().join("opened"), "broken", "b.mp3", None, || {
            Err("gone".into())
        })
        .is_err()
    );
    assert!(!root.path().join("opened").join("broken").exists());
    for index in 0..KEPT_COPIES + 3 {
        store_copy(
            &root.path().join("opened"),
            &format!("k{index}"),
            "s.mp3",
            Some(3),
            open,
        )
        .unwrap();
    }
    let kept = std::fs::read_dir(root.path().join("opened"))
        .unwrap()
        .count();
    assert_eq!(kept, KEPT_COPIES);
}

#[test]
fn handed_items_name_themselves() {
    let item = Handed {
        uri: "https://cdn.example/a/Show%20Intro.mp4".into(),
        ..Handed::default()
    };
    assert_eq!(item.shown_name(), "Show Intro.mp4");
    assert_eq!(item.kind(), Kind::Video);
}
