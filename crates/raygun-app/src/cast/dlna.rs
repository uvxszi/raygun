//! DLNA (UPnP AV) media renderers, which most smart TVs are: SOAP calls to the TV's AVTransport
//! service (`SetAVTransportURI`, `Play`, `Pause`, `Seek`, `Stop`, `GetPositionInfo`,
//! `GetTransportInfo`) and RenderingControl (volume). A renderer reports nothing by itself here,
//! so it's asked where it is every second.

use std::time::Duration;

use raygun_model::{CastControl, CastState};
use tokio::sync::mpsc;

use super::{CastMedia, Command, Report, Reporter};

const AV_TRANSPORT: &str = "urn:schemas-upnp-org:service:AVTransport:1";
const RENDERING_CONTROL: &str = "urn:schemas-upnp-org:service:RenderingControl:1";
/// What a TV is told it may do with a stream: seek by byte range, stream it.
const DLNA_FEATURES: &str =
    "DLNA.ORG_OP=01;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000";
const CALL_TIMEOUT: Duration = Duration::from_secs(6);
const POLL_EVERY: Duration = Duration::from_secs(1);
/// A TV that hasn't started the video by then can't play it.
const START_LIMIT: Duration = Duration::from_secs(30);
/// Calls failing in a row before the TV counts as gone.
const FAILURES_LIMIT: u32 = 8;
/// Stopped this close to the end means played to the end.
const END_SLACK_MS: f64 = 15_000.0;

/// Where a renderer's services are controlled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renderer {
    pub av_transport: String,
    pub rendering_control: Option<String>,
}

fn escape(text: &str) -> String {
    quick_xml::escape::escape(text).into_owned()
}

/// A SOAP request for `action` of `service`, its arguments in order.
pub fn envelope(service: &str, action: &str, arguments: &[(&str, String)]) -> String {
    let mut body = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action} xmlns:u=\"{service}\">"
    );
    for (name, value) in arguments {
        body.push_str(&format!("<{name}>{}</{name}>", escape(value)));
    }
    body.push_str(&format!("</u:{action}></s:Body></s:Envelope>"));
    body
}

/// The DIDL-Lite description sent with a video: its title and poster, the stream with its type,
/// and the subtitle file the way Samsung and LG TVs look for it.
pub fn didl(media: &CastMedia) -> String {
    let mut item = format!(
        "<DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\" xmlns:sec=\"http://www.sec.co.kr/\" xmlns:pv=\"http://www.pv.com/pvns/\"><item id=\"raygun-0\" parentID=\"-1\" restricted=\"1\"><dc:title>{}</dc:title><upnp:class>object.item.videoItem</upnp:class>",
        escape(&media.title)
    );
    if let Some(poster) = &media.poster {
        item.push_str(&format!(
            "<upnp:albumArtURI>{}</upnp:albumArtURI>",
            escape(poster)
        ));
    }
    let subtitle = media.active_track.and_then(|index| media.tracks.get(index));
    let subtitle_attribute = subtitle
        .map(|track| {
            format!(
                " pv:subtitleFileUri=\"{}\" pv:subtitleFileType=\"srt\"",
                escape(&track.url)
            )
        })
        .unwrap_or_default();
    item.push_str(&format!(
        "<res protocolInfo=\"http-get:*:{}:{DLNA_FEATURES}\"{subtitle_attribute}>{}</res>",
        escape(&media.content_type),
        escape(&media.url)
    ));
    if let Some(track) = subtitle {
        let url = escape(&track.url);
        item.push_str(&format!(
            "<res protocolInfo=\"http-get:*:text/srt:*\">{url}</res><sec:CaptionInfoEx sec:type=\"srt\">{url}</sec:CaptionInfoEx><sec:CaptionInfo sec:type=\"srt\">{url}</sec:CaptionInfo>"
        ));
    }
    item.push_str("</item></DIDL-Lite>");
    item
}

/// The text of the first `<name>` element in a SOAP answer, whatever its prefix.
pub fn field(answer: &str, name: &str) -> Option<String> {
    let mut from = 0;
    while let Some(found) = answer[from..].find('<').map(|index| from + index) {
        let rest = &answer[found + 1..];
        let end = rest.find('>')?;
        let tag = &rest[..end];
        from = found + 1;
        if tag.starts_with('/') || tag.ends_with('/') {
            continue;
        }
        let tag_name = tag.split_whitespace().next().unwrap_or_default();
        let local = tag_name.rsplit(':').next().unwrap_or_default();
        if local != name {
            continue;
        }
        let content = &rest[end + 1..];
        let close = content.find(&format!("</{tag_name}>"))?;
        let text = content[..close].trim();
        return Some(
            quick_xml::escape::unescape(text)
                .map(|text| text.into_owned())
                .unwrap_or_else(|_| text.to_owned()),
        );
    }
    None
}

/// What a SOAP fault means for the viewer.
pub fn fault(answer: &str) -> Option<String> {
    let code = field(answer, "errorCode")?;
    let description = field(answer, "errorDescription").unwrap_or_default();
    tracing::warn!(%code, %description, "the TV refused a request");
    Some(
        match code.trim() {
            "701" => "The TV can't do that right now",
            "710" | "711" => "The TV can't seek in this video",
            "714" | "715" | "716" => "The TV can't play this kind of video",
            "402" | "600" => "The TV didn't understand the request",
            _ => super::TV_FAILED,
        }
        .to_owned(),
    )
}

/// `H:MM:SS` for a seek; hours have two digits, which every renderer reads.
pub fn clock(ms: f64) -> String {
    let seconds = (ms.max(0.0) / 1000.0).round() as u64;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

/// `0:01:23`, `01:23:45.500` or `NOT_IMPLEMENTED` as milliseconds.
pub fn parse_clock(text: &str) -> Option<f64> {
    let text = text.trim();
    let (clock, fraction) = text.split_once('.').unwrap_or((text, ""));
    let parts: Vec<&str> = clock.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let mut seconds = 0.0;
    for part in parts {
        seconds = seconds * 60.0 + part.trim().parse::<f64>().ok()?;
    }
    let fraction = format!("0.{fraction}").parse::<f64>().unwrap_or(0.0);
    Some((seconds + fraction) * 1000.0)
}

/// A renderer's transport state as Raygun shows it; `played` says whether it has played yet.
pub fn transport_state(value: &str, played: bool) -> CastState {
    match value {
        "PLAYING" => CastState::Playing,
        "PAUSED_PLAYBACK" | "PAUSED_RECORDING" => CastState::Paused,
        "TRANSITIONING" if played => CastState::Buffering,
        "TRANSITIONING" => CastState::Loading,
        "STOPPED" | "NO_MEDIA_PRESENT" if played => CastState::Stopped,
        _ => CastState::Loading,
    }
}

pub struct Client {
    http: reqwest::Client,
    renderer: Renderer,
}

impl Client {
    pub fn new(http: reqwest::Client, renderer: Renderer) -> Self {
        Self { http, renderer }
    }

    async fn call(
        &self,
        url: &str,
        service: &str,
        action: &str,
        arguments: &[(&str, String)],
    ) -> Result<String, String> {
        let response = self
            .http
            .post(url)
            .header("Content-Type", "text/xml; charset=\"utf-8\"")
            .header("SOAPAction", format!("\"{service}#{action}\""))
            .body(envelope(service, action, arguments))
            .timeout(CALL_TIMEOUT)
            .send()
            .await
            .map_err(|error| {
                tracing::debug!(%error, %action, "the TV didn't answer");
                "The TV didn't answer".to_owned()
            })?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if status.is_success() {
            Ok(text)
        } else {
            Err(fault(&text).unwrap_or_else(|| super::TV_FAILED.to_owned()))
        }
    }

    async fn transport(
        &self,
        action: &str,
        arguments: &[(&str, String)],
    ) -> Result<String, String> {
        let mut all = vec![("InstanceID", "0".to_owned())];
        all.extend(arguments.iter().cloned());
        self.call(&self.renderer.av_transport, AV_TRANSPORT, action, &all)
            .await
    }

    async fn rendering(
        &self,
        action: &str,
        arguments: &[(&str, String)],
    ) -> Result<String, String> {
        let url = self
            .renderer
            .rendering_control
            .as_deref()
            .ok_or("This TV's volume can't be changed from here")?;
        let mut all = vec![
            ("InstanceID", "0".to_owned()),
            ("Channel", "Master".to_owned()),
        ];
        all.extend(arguments.iter().cloned());
        self.call(url, RENDERING_CONTROL, action, &all).await
    }

    pub async fn load(&self, media: &CastMedia) -> Result<(), String> {
        // A TV still playing something may refuse a new address until stopped.
        let _ = self.transport("Stop", &[]).await;
        self.transport(
            "SetAVTransportURI",
            &[
                ("CurrentURI", media.url.clone()),
                ("CurrentURIMetaData", didl(media)),
            ],
        )
        .await?;
        self.play().await
    }

    pub async fn play(&self) -> Result<(), String> {
        self.transport("Play", &[("Speed", "1".to_owned())])
            .await
            .map(drop)
    }

    pub async fn pause(&self) -> Result<(), String> {
        self.transport("Pause", &[]).await.map(drop)
    }

    pub async fn stop(&self) -> Result<(), String> {
        self.transport("Stop", &[]).await.map(drop)
    }

    pub async fn seek(&self, ms: f64) -> Result<(), String> {
        self.transport(
            "Seek",
            &[("Unit", "REL_TIME".to_owned()), ("Target", clock(ms))],
        )
        .await
        .map(drop)
    }

    /// The state (`PLAYING`, `STOPPED`…).
    pub async fn state(&self) -> Result<String, String> {
        let answer = self.transport("GetTransportInfo", &[]).await?;
        Ok(field(&answer, "CurrentTransportState").unwrap_or_default())
    }

    /// Where it is and how long the video is.
    pub async fn position(&self) -> Result<(Option<f64>, Option<f64>), String> {
        let answer = self.transport("GetPositionInfo", &[]).await?;
        Ok((
            field(&answer, "RelTime").and_then(|time| parse_clock(&time)),
            field(&answer, "TrackDuration")
                .and_then(|time| parse_clock(&time))
                .filter(|ms| *ms > 0.0),
        ))
    }

    /// 0–1, and muted.
    pub async fn volume(&self) -> Option<(f64, bool)> {
        let answer = self.rendering("GetVolume", &[]).await.ok()?;
        let level = field(&answer, "CurrentVolume")?.parse::<f64>().ok()?;
        let muted = match self.rendering("GetMute", &[]).await {
            Ok(answer) => {
                field(&answer, "CurrentMute").is_some_and(|mute| mute == "1" || mute == "true")
            }
            Err(_) => false,
        };
        Some(((level / 100.0).clamp(0.0, 1.0), muted))
    }

    pub async fn set_volume(&self, level: f64) -> Result<(), String> {
        let level = (level.clamp(0.0, 1.0) * 100.0).round() as u32;
        self.rendering("SetVolume", &[("DesiredVolume", level.to_string())])
            .await
            .map(drop)
    }

    pub async fn set_mute(&self, muted: bool) -> Result<(), String> {
        self.rendering(
            "SetMute",
            &[("DesiredMute", if muted { "1" } else { "0" }.to_owned())],
        )
        .await
        .map(drop)
    }
}

/// Plays `media` on the renderer and follows it until disconnected or the TV stops answering.
pub async fn run(
    client: Client,
    media: CastMedia,
    mut commands: mpsc::UnboundedReceiver<Command>,
    report: Reporter,
) -> Result<(), String> {
    let mut current = media;
    client.load(&current).await?;
    report(Report {
        state: Some(CastState::Loading),
        key: Some(current.key.clone()),
        ..Report::default()
    });
    // A renderer starts at the beginning: it's moved once it plays.
    let mut pending_seek = (current.start_ms > 1_000.0).then_some(current.start_ms);
    let mut loaded_at = tokio::time::Instant::now();
    let mut played = false;
    let mut finished = false;
    let mut last_position = current.start_ms;
    let mut duration: Option<f64> = None;
    let mut failures = 0u32;
    let mut ticks = 0u64;
    let mut poll = tokio::time::interval(POLL_EVERY);
    loop {
        tokio::select! {
            command = commands.recv() => {
                let result = match command {
                    None | Some(Command::Disconnect) => {
                        let _ = client.stop().await;
                        return Ok(());
                    }
                    Some(Command::Load(media)) => {
                        current = *media;
                        played = false;
                        finished = false;
                        duration = None;
                        last_position = current.start_ms;
                        pending_seek = (current.start_ms > 1_000.0).then_some(current.start_ms);
                        loaded_at = tokio::time::Instant::now();
                        report(Report { state: Some(CastState::Loading), key: Some(current.key.clone()), position_ms: Some(last_position), ..Report::default() });
                        client.load(&current).await
                    }
                    Some(Command::Control(control)) => match control {
                        CastControl::Play => {
                            let stopped = finished
                                || (played
                                    && matches!(
                                        client.state().await.as_deref(),
                                        Ok("STOPPED" | "NO_MEDIA_PRESENT")
                                    ));
                            if stopped {
                                // Stopped or finished: loaded again, from where it was.
                                let from = if finished { 0.0 } else { last_position };
                                played = false;
                                finished = false;
                                pending_seek = (from > 1_000.0).then_some(from);
                                loaded_at = tokio::time::Instant::now();
                                client.load(&current).await
                            } else {
                                client.play().await
                            }
                        }
                        CastControl::Pause => client.pause().await,
                        CastControl::Stop => client.stop().await,
                        CastControl::Seek { position_ms } => {
                            last_position = position_ms.max(0.0);
                            report(Report { position_ms: Some(last_position), ..Report::default() });
                            client.seek(last_position).await
                        }
                        CastControl::Volume { level } => {
                            let done = client.set_volume(level).await;
                            if done.is_ok() {
                                report(Report { volume: Some(level.clamp(0.0, 1.0)), ..Report::default() });
                            }
                            done
                        }
                        CastControl::Mute { muted } => {
                            let done = client.set_mute(muted).await;
                            if done.is_ok() {
                                report(Report { muted: Some(muted), ..Report::default() });
                            }
                            done
                        }
                    },
                };
                if let Err(error) = result {
                    report(Report { error: Some(error), ..Report::default() });
                }
            }
            _ = poll.tick() => {
                ticks += 1;
                let state = match client.state().await {
                    Ok(state) => {
                        failures = 0;
                        state
                    }
                    Err(_) => {
                        failures += 1;
                        if failures >= FAILURES_LIMIT {
                            return Err(super::LOST.to_owned());
                        }
                        continue;
                    }
                };
                let (position, length) = client.position().await.unwrap_or((None, None));
                if length.is_some() {
                    duration = length;
                }
                let playing = state == "PLAYING";
                if playing {
                    played = true;
                    if let Some(target) = pending_seek.take() {
                        let _ = client.seek(target).await;
                        last_position = target;
                    } else if let Some(position) = position {
                        last_position = position;
                    }
                } else if state == "PAUSED_PLAYBACK" && let Some(position) = position {
                    last_position = position;
                }
                let mut shown = transport_state(&state, played);
                let stopped = matches!(state.as_str(), "STOPPED" | "NO_MEDIA_PRESENT");
                if stopped && played && duration.is_some_and(|length| last_position >= length - END_SLACK_MS) {
                    shown = CastState::Finished;
                    finished = true;
                }
                let error = (!played && loaded_at.elapsed() > START_LIMIT).then(|| super::TV_FAILED.to_owned());
                if error.is_some() {
                    shown = CastState::Failed;
                }
                let volume = if ticks % 5 == 1 { client.volume().await } else { None };
                report(Report {
                    state: Some(shown),
                    position_ms: Some(last_position),
                    duration_ms: duration,
                    volume: volume.map(|(level, _)| level),
                    muted: volume.map(|(_, muted)| muted),
                    error,
                    key: Some(current.key.clone()),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media() -> CastMedia {
        CastMedia {
            key: "url:https://cdn.example/a&b.mp4".into(),
            url: "http://192.168.1.5:50000/cast/t/media/m/https/cdn.example/a.mp4?x=1&y=2".into(),
            content_type: "video/mp4".into(),
            title: "Tom & Jerry <1>".into(),
            subtitle: None,
            poster: Some("https://images.example/p.jpg".into()),
            tracks: vec![super::super::Track {
                url: "http://192.168.1.5:50000/cast/t/subtitle/0.srt".into(),
                language: Some("en".into()),
                label: "English".into(),
            }],
            active_track: Some(0),
            start_ms: 0.0,
        }
    }

    #[test]
    fn soap_bodies_carry_escaped_arguments() {
        let body = envelope(
            AV_TRANSPORT,
            "Seek",
            &[
                ("InstanceID", "0".into()),
                ("Unit", "REL_TIME".into()),
                ("Target", clock(3_723_400.0)),
            ],
        );
        assert!(body.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>"));
        assert!(body.contains(
            "<s:Body><u:Seek xmlns:u=\"urn:schemas-upnp-org:service:AVTransport:1\"><InstanceID>0</InstanceID><Unit>REL_TIME</Unit><Target>01:02:03</Target></u:Seek></s:Body>"
        ));

        let didl = didl(&media());
        assert!(didl.contains("<dc:title>Tom &amp; Jerry &lt;1&gt;</dc:title>"));
        assert!(didl.contains("<upnp:class>object.item.videoItem</upnp:class>"));
        assert!(didl.contains(
            "protocolInfo=\"http-get:*:video/mp4:DLNA.ORG_OP=01;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000\""
        ));
        assert!(didl.contains(
            ">http://192.168.1.5:50000/cast/t/media/m/https/cdn.example/a.mp4?x=1&amp;y=2</res>"
        ));
        assert!(didl.contains("<sec:CaptionInfoEx sec:type=\"srt\">http://192.168.1.5:50000/cast/t/subtitle/0.srt</sec:CaptionInfoEx>"));
        assert!(
            didl.contains("pv:subtitleFileUri=\"http://192.168.1.5:50000/cast/t/subtitle/0.srt\"")
        );

        // The description goes inside the request as text, escaped once more.
        let set = envelope(
            AV_TRANSPORT,
            "SetAVTransportURI",
            &[
                ("InstanceID", "0".into()),
                ("CurrentURI", media().url),
                ("CurrentURIMetaData", didl.clone()),
            ],
        );
        assert!(set.contains("<CurrentURIMetaData>&lt;DIDL-Lite xmlns=&quot;urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/&quot;"));
        assert!(set.contains("Tom &amp;amp; Jerry"));
        assert!(!media().tracks.is_empty());
    }

    #[test]
    fn answers_and_faults_are_read() {
        let position = r#"<?xml version="1.0"?><s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><u:GetPositionInfoResponse xmlns:u="urn:schemas-upnp-org:service:AVTransport:1"><Track>1</Track><TrackDuration>0:42:13.000</TrackDuration><TrackMetaData>&lt;DIDL-Lite/&gt;</TrackMetaData><RelTime>00:01:02</RelTime><AbsTime>NOT_IMPLEMENTED</AbsTime></u:GetPositionInfoResponse></s:Body></s:Envelope>"#;
        assert_eq!(field(position, "RelTime").as_deref(), Some("00:01:02"));
        assert_eq!(
            field(position, "TrackMetaData").as_deref(),
            Some("<DIDL-Lite/>")
        );
        assert_eq!(
            field(position, "TrackDuration").and_then(|time| parse_clock(&time)),
            Some(2_533_000.0)
        );
        assert_eq!(
            field(position, "AbsTime").and_then(|time| parse_clock(&time)),
            None
        );
        assert_eq!(field(position, "Missing"), None);
        assert_eq!(parse_clock("1:02:03.5"), Some(3_723_500.0));

        let fault = r#"<s:Envelope><s:Body><s:Fault><faultcode>s:Client</faultcode><faultstring>UPnPError</faultstring><detail><UPnPError xmlns="urn:schemas-upnp-org:control-1-0"><errorCode>714</errorCode><errorDescription>Illegal MIME-type</errorDescription></UPnPError></detail></s:Fault></s:Body></s:Envelope>"#;
        assert_eq!(
            super::fault(fault).as_deref(),
            Some("The TV can't play this kind of video")
        );
        assert_eq!(super::fault("<html>500</html>"), None);
    }

    /// A renderer that plays once asked to, and answers where it is.
    async fn fake_renderer(actions: std::sync::Arc<std::sync::Mutex<Vec<String>>>) -> String {
        use axum::extract::State;
        use axum::http::HeaderMap;
        async fn control(
            State(actions): State<std::sync::Arc<std::sync::Mutex<Vec<String>>>>,
            headers: HeaderMap,
            body: String,
        ) -> String {
            let action = headers["SOAPAction"]
                .to_str()
                .unwrap()
                .trim_matches('"')
                .rsplit('#')
                .next()
                .unwrap()
                .to_owned();
            let mut actions = actions.lock().unwrap();
            let played = actions.iter().any(|done| done == "Play");
            match action.as_str() {
                "GetTransportInfo" => {
                    let state = if played { "PLAYING" } else { "STOPPED" };
                    format!("<CurrentTransportState>{state}</CurrentTransportState>")
                }
                "GetPositionInfo" => {
                    "<RelTime>00:01:05</RelTime><TrackDuration>00:10:00</TrackDuration>".into()
                }
                "GetVolume" => "<CurrentVolume>30</CurrentVolume>".into(),
                "GetMute" => "<CurrentMute>0</CurrentMute>".into(),
                _ => {
                    let target = field(&body, "Target").map(|target| format!(" {target}"));
                    actions.push(format!("{action}{}", target.unwrap_or_default()));
                    String::new()
                }
            }
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let router = axum::Router::new()
                .route("/control", axum::routing::post(control))
                .with_state(actions);
            axum::serve(listener, router).await.unwrap();
        });
        format!("http://{address}/control")
    }

    #[tokio::test]
    async fn a_renderer_is_loaded_moved_to_the_start_and_followed() {
        let actions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let url = fake_renderer(actions.clone()).await;
        let client = Client::new(
            raygun_net::http_client().unwrap(),
            Renderer {
                av_transport: url.clone(),
                rendering_control: Some(url),
            },
        );
        let reports = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let heard = reports.clone();
        let reporter: Reporter =
            std::sync::Arc::new(move |report| heard.lock().unwrap().push(report));
        let (commands, receiver) = mpsc::unbounded_channel();
        let running = tokio::spawn(run(
            client,
            CastMedia {
                start_ms: 61_000.0,
                ..media()
            },
            receiver,
            reporter,
        ));
        tokio::time::sleep(Duration::from_millis(2_300)).await;
        commands
            .send(Command::Control(CastControl::Volume { level: 0.5 }))
            .unwrap();
        commands.send(Command::Control(CastControl::Pause)).unwrap();
        commands.send(Command::Disconnect).unwrap();
        running.await.unwrap().unwrap();

        let actions = actions.lock().unwrap().clone();
        assert_eq!(
            actions,
            [
                "Stop",
                "SetAVTransportURI",
                "Play",
                "Seek 00:01:01",
                "SetVolume",
                "Pause",
                "Stop"
            ]
        );
        let reports = reports.lock().unwrap();
        let playing = reports
            .iter()
            .find(|report| report.state == Some(CastState::Playing))
            .unwrap();
        assert_eq!(playing.duration_ms, Some(600_000.0));
        assert_eq!(playing.key.as_deref(), Some(media().key.as_str()));
        assert!(reports.iter().any(|report| report.volume == Some(0.3)));
        assert!(reports.iter().any(|report| report.volume == Some(0.5)));
    }

    #[test]
    fn transport_states_as_shown() {
        assert_eq!(transport_state("PLAYING", false), CastState::Playing);
        assert_eq!(transport_state("PAUSED_PLAYBACK", true), CastState::Paused);
        assert_eq!(transport_state("TRANSITIONING", false), CastState::Loading);
        assert_eq!(transport_state("TRANSITIONING", true), CastState::Buffering);
        assert_eq!(transport_state("STOPPED", false), CastState::Loading);
        assert_eq!(transport_state("STOPPED", true), CastState::Stopped);
        assert_eq!(
            transport_state("NO_MEDIA_PRESENT", true),
            CastState::Stopped
        );
    }
}
