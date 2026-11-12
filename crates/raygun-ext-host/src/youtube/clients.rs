//! The InnerTube clients Raygun speaks as: YouTube Music's web client for search, youtube.com's
//! web client for playlists, and YouTube's app clients whose `player` answers carry plain stream
//! addresses. The app clients, their versions and contexts are those Grayjay's YouTube plugin
//! sends (futo-org/grayjay-plugin-youtube, AGPL-3.0).

use std::time::Duration;

use serde_json::{Value, json};

use super::Request;

pub(crate) const MUSIC: &str = "https://music.youtube.com";
pub(crate) const WEB: &str = "https://www.youtube.com";
/// What the web clients send; YouTube answers unknown agents differently.
pub(crate) const WEB_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";
/// Recent web client versions. YouTube refuses very old ones; a 400 reads the current one from
/// its page.
pub(crate) const MUSIC_VERSION: &str = "1.20261027.10.00";
pub(crate) const WEB_VERSION: &str = "2.20261028.01.00";

/// YouTube Music's Songs and Videos search chips.
pub(crate) const SONGS: &str = "EgWKAQIIAWoKEAkQBRAKEAMQBA==";
pub(crate) const VIDEOS: &str = "EgWKAQIQAWoKEAkQChAFEAMQBA==";
/// youtube.com's search filter for streams on the air now (Features → Live).
pub(crate) const LIVE_FILTER: &str = "EgJAAQ==";
/// YouTube's Live page: the streams it features now, by topic.
pub(crate) const LIVE_PAGE: &str = "UC4R8DWoMoI7CAwX8_LjQHig";

/// How long one InnerTube request may take.
const API_TIMEOUT: Duration = Duration::from_secs(8);
/// How long a `player` request may take: another client is asked after it.
const PLAYER_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the check of a stream's last bytes may take.
const CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// One of YouTube's apps, as its `player` requests name it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PlayerClient {
    pub name: &'static str,
    /// `X-YouTube-Client-Name`.
    pub id: u32,
    pub version: &'static str,
    pub user_agent: &'static str,
}

const ANDROID_VR: PlayerClient = PlayerClient {
    name: "ANDROID_VR",
    id: 28,
    version: "1.65.10",
    user_agent: "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; en_US; Quest 3 Build/SQ3A.220605.009.A1) gzip",
};

const VISIONOS: PlayerClient = PlayerClient {
    name: "VISIONOS",
    id: 101,
    version: "1.04",
    user_agent: "com.google.visionos.youtube/1.04 (RealityDevice17,1; U; CPU visionOS 26_6_0 like Mac OS X; US) gzip",
};

const IOS: PlayerClient = PlayerClient {
    name: "IOS",
    id: 5,
    version: "20.10.4",
    user_agent: "com.google.ios.youtube/20.10.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X;)",
};

/// The clients asked for a stream, in this order (the last one whose stream worked goes first).
pub(crate) const PLAYER_CLIENTS: [PlayerClient; 3] = [ANDROID_VR, VISIONOS, IOS];

impl PlayerClient {
    /// The device the client says it runs on.
    fn device(&self) -> Value {
        match self.name {
            "ANDROID_VR" => json!({
                "deviceMake": "Oculus",
                "deviceModel": "Quest 3",
                "osName": "Android",
                "osVersion": "12L",
                "androidSdkVersion": 32,
                "platform": "MOBILE",
            }),
            "VISIONOS" => json!({
                "deviceMake": "Apple",
                "deviceModel": "RealityDevice17,1",
                "osName": "visionOS",
                "osVersion": "26.6.0",
            }),
            _ => json!({
                "deviceMake": "Apple",
                "deviceModel": "iPhone16,2",
                "osName": "iPhone",
                "osVersion": "18.3.2.22D82",
            }),
        }
    }
}

fn post(
    url: String,
    body: &Value,
    headers: Vec<(&'static str, String)>,
    timeout: Duration,
) -> Request {
    let mut headers = headers;
    headers.push(("Content-Type", "application/json".to_owned()));
    Request {
        method: "POST",
        url,
        headers,
        body: Some(body.to_string()),
        timeout,
    }
}

pub(crate) fn get(
    url: impl Into<String>,
    headers: Vec<(&'static str, String)>,
    timeout: Duration,
) -> Request {
    Request {
        method: "GET",
        url: url.into(),
        headers,
        body: None,
        timeout,
    }
}

/// The language YouTube Music answers a search in: the query's own where names have a spelling
/// in its script (YouTube spells 米津玄師 "Kenshi Yonezu" and 아이유 "IU" in English, and the song
/// would no longer match what was asked), else English.
pub(crate) fn search_language(query: &str) -> &'static str {
    let mut han = false;
    for character in query.chars() {
        match u32::from(character) {
            // Hangul syllables and jamo.
            0xAC00..=0xD7AF | 0x1100..=0x11FF | 0x3130..=0x318F => return "ko",
            // Hiragana and katakana.
            0x3040..=0x30FF | 0x31F0..=0x31FF => return "ja",
            // Han: Japanese spells names the way they're written; Chinese names stay as they are.
            0x4E00..=0x9FFF | 0x3400..=0x4DBF => han = true,
            _ => {}
        }
    }
    if han { "ja" } else { "en" }
}

/// A YouTube Music search with one of its chips (`SONGS`, `VIDEOS`).
pub(crate) fn music_search(version: &str, query: &str, chip: &str) -> Request {
    let body = json!({
        "context": {
            "client": {
                "clientName": "WEB_REMIX",
                "clientVersion": version,
                "hl": search_language(query),
            }
        },
        "query": query,
        "params": chip,
    });
    post(
        format!("{MUSIC}/youtubei/v1/search?prettyPrint=false"),
        &body,
        vec![
            ("User-Agent", WEB_USER_AGENT.to_owned()),
            ("Origin", MUSIC.to_owned()),
            ("Referer", format!("{MUSIC}/")),
            ("X-YouTube-Client-Name", "67".to_owned()),
            ("X-YouTube-Client-Version", version.to_owned()),
        ],
        API_TIMEOUT,
    )
}

/// A youtube.com web request (`browse`, `next`): `ask` holds what it asks for. Visitor data
/// carries the session further pages belong to; without it YouTube answers the first again.
pub(crate) fn web(endpoint: &str, version: &str, visitor: Option<&str>, ask: Value) -> Request {
    let mut client = json!({ "clientName": "WEB", "clientVersion": version, "hl": "en" });
    let mut headers = vec![
        ("User-Agent", WEB_USER_AGENT.to_owned()),
        ("Origin", WEB.to_owned()),
        ("Referer", format!("{WEB}/")),
        ("X-YouTube-Client-Name", "1".to_owned()),
        ("X-YouTube-Client-Version", version.to_owned()),
    ];
    if let Some(visitor) = visitor {
        client["visitorData"] = json!(visitor);
        headers.push(("X-Goog-Visitor-Id", visitor.to_owned()));
    }
    let mut body = json!({ "context": { "client": client } });
    if let (Some(body), Value::Object(ask)) = (body.as_object_mut(), ask) {
        body.extend(ask);
    }
    post(
        format!("{WEB}/youtubei/v1/{endpoint}?prettyPrint=false"),
        &body,
        headers,
        API_TIMEOUT,
    )
}

/// One app client's `player` request for a video.
pub(crate) fn player(client: &PlayerClient, video_id: &str, visitor: Option<&str>) -> Request {
    let mut context = client.device();
    context["clientName"] = json!(client.name);
    context["clientVersion"] = json!(client.version);
    context["hl"] = json!("en");
    context["gl"] = json!("US");
    context["utcOffsetMinutes"] = json!(0);
    let mut headers = vec![
        ("User-Agent", client.user_agent.to_owned()),
        ("X-Goog-Api-Format-Version", "2".to_owned()),
        ("X-YouTube-Client-Name", client.id.to_string()),
        ("X-YouTube-Client-Version", client.version.to_owned()),
    ];
    if let Some(visitor) = visitor {
        context["visitorData"] = json!(visitor);
        headers.push(("X-Goog-Visitor-Id", visitor.to_owned()));
    }
    let body = json!({
        "context": { "client": context, "user": { "lockedSafetyMode": false } },
        "videoId": video_id,
        "contentCheckOk": true,
        "racyCheckOk": true,
    });
    post(
        format!("{WEB}/youtubei/v1/player?prettyPrint=false"),
        &body,
        headers,
        PLAYER_TIMEOUT,
    )
}

/// YouTube's service worker data, which carries a visitor token.
pub(crate) fn sw_data() -> Request {
    get(
        format!("{WEB}/sw.js_data"),
        vec![("User-Agent", WEB_USER_AGENT.to_owned())],
        Duration::from_secs(4),
    )
}

/// A YouTube page, for the client version it states.
pub(crate) fn page(url: &str) -> Request {
    get(
        url,
        vec![("User-Agent", WEB_USER_AGENT.to_owned())],
        API_TIMEOUT,
    )
}

/// Two bytes of a stream at `start`, with the agent of the client it was given to.
pub(crate) fn stream_check(url: &str, start: u64, user_agent: &str) -> Request {
    get(
        url,
        vec![
            ("Range", format!("bytes={start}-{}", start + 1)),
            ("User-Agent", user_agent.to_owned()),
        ],
        CHECK_TIMEOUT,
    )
}
