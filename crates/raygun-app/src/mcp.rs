//! The MCP server (M19): its on/off switch and token, and the tools that map onto Raygun's
//! services. Transport and auth live in `raygun-mcp`.

use std::sync::{Arc, Mutex};

use raygun_mcp::{Backend, ResourceDef, Running, Token, ToolDef};
use raygun_model::{
    Collection, CollectionPlayRequest, McpStatus, OnlineTrack, PlaybackControl, QueueItem,
    SavedTrack, TrackInput, TrackQuery,
};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use crate::collections::CollectionsService;
use crate::extensions::ExtensionService;
use crate::library::LibraryService;
use crate::playback::PlaybackService;
use crate::startup::AppDb;

const ENABLED_KEY: &str = "mcp_enabled";
const TOKEN_KEY: &str = "mcp_token";
/// Tracks listed per provider in `search_music`; agents rarely need more.
const SEARCH_TRACKS_PER_PROVIDER: usize = 5;
const SEARCH_LIBRARY_LIMIT: u32 = 8;
const HISTORY_DEFAULT: u32 = 10;

pub struct McpService {
    app: AppHandle,
    token: Token,
    state: Mutex<ServerState>,
}

#[derive(Default)]
struct ServerState {
    running: Option<Running>,
    error: Option<String>,
}

impl McpService {
    /// Loads (or creates) the token and starts the server when it was left on.
    pub async fn start(app: AppHandle) -> Result<Self, String> {
        let db = &app.state::<AppDb>().inner().0;
        let token = match raygun_db::local_value(db, TOKEN_KEY)
            .await
            .map_err(to_message)?
        {
            Some(token) => token,
            None => {
                let token = raygun_mcp::generate_token();
                raygun_db::set_local_value(db, TOKEN_KEY, &token)
                    .await
                    .map_err(to_message)?;
                token
            }
        };
        let enabled = raygun_db::local_value(db, ENABLED_KEY)
            .await
            .map_err(to_message)?
            .as_deref()
            == Some("true");
        let service = Self {
            app: app.clone(),
            token: Token::new(token),
            state: Mutex::new(ServerState::default()),
        };
        if enabled {
            service.run().await;
        }
        Ok(service)
    }

    pub async fn status(&self) -> Result<McpStatus, String> {
        let db = self.db();
        let enabled = raygun_db::local_value(db, ENABLED_KEY)
            .await
            .map_err(to_message)?
            .as_deref()
            == Some("true");
        let token = raygun_db::local_value(db, TOKEN_KEY)
            .await
            .map_err(to_message)?
            .unwrap_or_default();
        let state = self.lock();
        Ok(McpStatus {
            enabled,
            url: state
                .running
                .as_ref()
                .map(|running| format!("http://{}/mcp", running.address)),
            token,
            error: state.error.clone(),
        })
    }

    pub async fn set_enabled(&self, enabled: bool) -> Result<McpStatus, String> {
        raygun_db::set_local_value(
            self.db(),
            ENABLED_KEY,
            if enabled { "true" } else { "false" },
        )
        .await
        .map_err(to_message)?;
        if enabled {
            if self.lock().running.is_none() {
                self.run().await;
            }
        } else {
            let mut state = self.lock();
            if let Some(running) = state.running.take() {
                running.stop();
                tracing::info!("MCP server stopped");
            }
            state.error = None;
        }
        self.status().await
    }

    /// Replaces the token; clients holding the old one are refused from now on.
    pub async fn new_token(&self) -> Result<McpStatus, String> {
        let token = raygun_mcp::generate_token();
        raygun_db::set_local_value(self.db(), TOKEN_KEY, &token)
            .await
            .map_err(to_message)?;
        self.token.set(token);
        self.status().await
    }

    async fn run(&self) {
        let router = raygun_mcp::router(
            AppBackend {
                app: self.app.clone(),
            },
            self.token.clone(),
            env!("CARGO_PKG_VERSION"),
        );
        let result = raygun_mcp::serve(router).await;
        let mut state = self.lock();
        match result {
            Ok(running) => {
                tracing::info!(address = %running.address, "MCP server listening");
                state.running = Some(running);
                state.error = None;
            }
            Err(error) => {
                tracing::warn!(%error, "MCP server could not start");
                state.error = Some(format!("Couldn't start: {error}"));
            }
        }
    }

    fn db(&self) -> &raygun_db::Db {
        &self.app.state::<AppDb>().inner().0
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ServerState> {
        self.state.lock().expect("mcp state poisoned")
    }
}

/// The tools, backed by the app's services.
struct AppBackend {
    app: AppHandle,
}

impl Backend for AppBackend {
    fn tools(&self) -> Vec<ToolDef> {
        tool_defs()
    }

    async fn call_tool(&self, name: &str, args: Value) -> Result<String, String> {
        let result = match name {
            "now_playing" => Ok(self.now_playing()),
            "search_music" => self.search(&text_arg(&args, "query")?).await,
            "play_track" => self.add(track_arg(&args)?, true).await,
            "queue_add" => self.add(track_arg(&args)?, false).await,
            "queue_get" => Ok(self.queue()),
            "playback_control" => self.control(&args),
            "favorites_list" => self.favorites().await,
            "favorite_set" => self.set_favorite(&args).await,
            "playlists_list" => self.playlists().await,
            "playlist_play" => self.play_playlist(&text_arg(&args, "name")?).await,
            "playlist_add" => self.add_to_playlist(&args).await,
            "history_recent" => self.history(&args).await,
            "lyrics_current" => self.lyrics().await,
            _ => Err(format!("Unknown tool: {name}")),
        };
        result.map(|value| value.to_string())
    }

    fn resources(&self) -> Vec<ResourceDef> {
        vec![
            ResourceDef {
                uri: "raygun://now-playing",
                name: "Now playing",
                description: "Playback status and the current track",
            },
            ResourceDef {
                uri: "raygun://queue",
                name: "Queue",
                description: "The play queue in order, with the current item marked",
            },
        ]
    }

    async fn read_resource(&self, uri: &str) -> Option<Result<String, String>> {
        match uri {
            "raygun://now-playing" => Some(Ok(self.now_playing().to_string())),
            "raygun://queue" => Some(Ok(self.queue().to_string())),
            _ => None,
        }
    }
}

impl AppBackend {
    fn playback(&self) -> tauri::State<'_, PlaybackService> {
        self.app.state::<PlaybackService>()
    }

    fn collections(&self) -> tauri::State<'_, CollectionsService> {
        self.app.state::<CollectionsService>()
    }

    fn now_playing(&self) -> Value {
        let playback = self.playback();
        let state = playback.state();
        let queue = playback.queue();
        let current = state
            .current_item_id
            .as_ref()
            .and_then(|id| queue.items.iter().find(|item| &item.id == id));
        json!({
            "status": state.status,
            "positionMs": state.position_ms.round(),
            "durationMs": state.duration_ms.map(f64::round),
            "track": current.map(item_json),
            "source": state.source_label,
            "error": state.error,
        })
    }

    fn queue(&self) -> Value {
        let playback = self.playback();
        let state = playback.state();
        let queue = playback.queue();
        let items: Vec<Value> = queue
            .order
            .iter()
            .filter_map(|index| queue.items.get(*index as usize))
            .map(|item| {
                let mut value = item_json(item);
                value["current"] = json!(state.current_item_id.as_ref() == Some(&item.id));
                value
            })
            .collect();
        json!({ "items": items, "shuffle": state.shuffle, "repeat": state.repeat })
    }

    async fn search(&self, query: &str) -> Result<Value, String> {
        let library = self.app.state::<LibraryService>();
        let extensions = self.app.state::<Arc<ExtensionService>>();
        let (local, providers) = futures_util::future::join(
            library.search(query, SEARCH_LIBRARY_LIMIT),
            extensions.search(query),
        )
        .await;
        let top = crate::search::merge_top(&providers);
        Ok(json!({
            "library": local.unwrap_or_default().iter().map(|track| json!({
                "title": track.title, "artist": track.artist, "album": track.album,
                "durationMs": track.duration_ms,
            })).collect::<Vec<_>>(),
            "top": top.iter().map(online_json).collect::<Vec<_>>(),
            "providers": providers.iter().map(|result| json!({
                "name": result.provider.name,
                "error": result.error,
                "tracks": result.tracks.iter().take(SEARCH_TRACKS_PER_PROVIDER)
                    .map(online_json).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "hint": "Play a song with play_track using its title and artist.",
        }))
    }

    async fn add(&self, track: TrackInput, play_now: bool) -> Result<Value, String> {
        let item = self.collections().queue_item_for(track).await?;
        let summary = json!({
            "title": item.title,
            "artist": item.artist,
            "from": match &item.source {
                raygun_model::MediaSource::LocalFile { .. } => "library",
                raygun_model::MediaSource::Lookup { .. } => "online (finding a stream)",
                _ => "stream",
            },
        });
        self.playback().enqueue(vec![item], play_now);
        Ok(json!({ "queued": summary, "playing": play_now }))
    }

    fn control(&self, args: &Value) -> Result<Value, String> {
        let control = parse_control(args)?;
        self.playback().control(control);
        Ok(json!({ "done": text_arg(args, "action")? }))
    }

    async fn favorites(&self) -> Result<Value, String> {
        let favorites = self.collections().favorites().await?;
        Ok(json!({ "favorites": favorites.iter().map(saved_json).collect::<Vec<_>>() }))
    }

    async fn set_favorite(&self, args: &Value) -> Result<Value, String> {
        let favorite = args
            .get("favorite")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let track = track_arg(args)?;
        let title = track.title.clone();
        self.collections().set_favorite(track, favorite).await?;
        Ok(json!({ "title": title, "favorite": favorite }))
    }

    async fn playlists(&self) -> Result<Value, String> {
        let playlists = self.collections().playlists().await?;
        Ok(json!({
            "playlists": playlists.iter().map(|playlist| json!({
                "name": playlist.name, "tracks": playlist.item_count,
            })).collect::<Vec<_>>(),
        }))
    }

    /// The playlist whose name matches, ignoring case.
    async fn find_playlist(&self, name: &str) -> Result<Option<String>, String> {
        let wanted = name.trim().to_lowercase();
        Ok(self
            .collections()
            .playlists()
            .await?
            .into_iter()
            .find(|playlist| playlist.name.to_lowercase() == wanted)
            .map(|playlist| playlist.id))
    }

    async fn play_playlist(&self, name: &str) -> Result<Value, String> {
        let id = self
            .find_playlist(name)
            .await?
            .ok_or_else(|| format!("No playlist named “{name}”. See playlists_list."))?;
        self.collections()
            .play(CollectionPlayRequest {
                order: None,
                collection: Collection::Playlist { id },
                start_index: None,
                shuffle: false,
            })
            .await?;
        Ok(json!({ "playing": name }))
    }

    async fn add_to_playlist(&self, args: &Value) -> Result<Value, String> {
        let name = text_arg(args, "name")?;
        let track = track_arg(args)?;
        let title = track.title.clone();
        let collections = self.collections();
        let (id, created) = match self.find_playlist(&name).await? {
            Some(id) => (id, false),
            None => (collections.create_playlist(&name).await?.id, true),
        };
        // A song the playlist holds already isn't added twice (M46.48).
        let added = collections.add_to_playlist(&id, vec![track], false).await?;
        if added.added == 0 {
            return Ok(json!({ "playlist": name, "alreadyIn": title, "created": created }));
        }
        Ok(json!({ "playlist": name, "added": title, "created": created }))
    }

    async fn history(&self, args: &Value) -> Result<Value, String> {
        let limit = args
            .get("limit")
            .and_then(Value::as_u64)
            .map_or(HISTORY_DEFAULT, |limit| limit.clamp(1, 100) as u32);
        let history = self.collections().history(limit).await?;
        Ok(json!({
            "recent": history.iter().map(|entry| {
                let mut value = saved_json(&entry.track);
                value["playedAt"] = json!(entry.played_at);
                value
            }).collect::<Vec<_>>(),
        }))
    }

    async fn lyrics(&self) -> Result<Value, String> {
        let playback = self.playback();
        let state = playback.state();
        let queue = playback.queue();
        let Some(item) = state
            .current_item_id
            .as_ref()
            .and_then(|id| queue.items.iter().find(|item| &item.id == id))
        else {
            return Err("Nothing is playing".into());
        };
        let query = TrackQuery::for_item(item);
        let lyrics = self
            .app
            .state::<Arc<ExtensionService>>()
            .lyrics(&query)
            .await?;
        Ok(match lyrics {
            None => json!({ "title": item.title, "lyrics": null }),
            Some(lyrics) => {
                let text = if lyrics.synced.is_empty() {
                    lyrics.plain.unwrap_or_default()
                } else {
                    lyrics
                        .synced
                        .iter()
                        .map(|line| line.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                json!({ "title": item.title, "from": lyrics.provider_name, "lyrics": text })
            }
        })
    }
}

fn tool_defs() -> Vec<ToolDef> {
    let none = json!({ "type": "object", "properties": {} });
    let track = |extra: Value| {
        let mut properties = json!({
            "title": { "type": "string", "description": "Song title" },
            "artist": { "type": "string", "description": "Artist, which makes the match much better" },
            "album": { "type": "string" },
        });
        if let (Some(target), Some(more)) = (properties.as_object_mut(), extra.as_object()) {
            target.extend(more.clone());
        }
        json!({ "type": "object", "properties": properties, "required": ["title"] })
    };
    vec![
        ToolDef {
            name: "now_playing",
            description: "What Raygun is playing: status, position, track and stream source.",
            input_schema: none.clone(),
        },
        ToolDef {
            name: "search_music",
            description: "Search the local library and every installed music source.",
            input_schema: json!({
                "type": "object",
                "properties": { "query": { "type": "string", "description": "Song, artist or both" } },
                "required": ["query"],
            }),
        },
        ToolDef {
            name: "play_track",
            description: "Play a song now. Uses the local file when the library has it, \
                otherwise finds a stream online.",
            input_schema: track(json!({})),
        },
        ToolDef {
            name: "queue_add",
            description: "Add a song to the end of the play queue.",
            input_schema: track(json!({})),
        },
        ToolDef {
            name: "queue_get",
            description: "The play queue in order, with the current item marked.",
            input_schema: none.clone(),
        },
        ToolDef {
            name: "playback_control",
            description: "Control playback: play, pause, toggle, next, previous or seek.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string",
                                "enum": ["play", "pause", "toggle", "next", "previous", "seek"] },
                    "positionMs": { "type": "number", "description": "For seek, in milliseconds" },
                },
                "required": ["action"],
            }),
        },
        ToolDef {
            name: "favorites_list",
            description: "The user's favorite songs, newest first.",
            input_schema: none.clone(),
        },
        ToolDef {
            name: "favorite_set",
            description: "Add a song to favorites, or remove it with favorite=false.",
            input_schema: track(json!({ "favorite": { "type": "boolean", "default": true } })),
        },
        ToolDef {
            name: "playlists_list",
            description: "The user's playlists with their sizes.",
            input_schema: none.clone(),
        },
        ToolDef {
            name: "playlist_play",
            description: "Play a playlist by name.",
            input_schema: json!({
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"],
            }),
        },
        ToolDef {
            name: "playlist_add",
            description: "Add a song to a playlist by name, creating the playlist if needed. A \
                          song the playlist already holds isn't added again (alreadyIn).",
            input_schema: {
                let mut schema =
                    track(json!({ "name": { "type": "string", "description": "Playlist name" } }));
                schema["required"] = json!(["name", "title"]);
                schema
            },
        },
        ToolDef {
            name: "history_recent",
            description: "Recently played songs, newest first.",
            input_schema: json!({
                "type": "object",
                "properties": { "limit": { "type": "integer", "minimum": 1, "maximum": 100 } },
            }),
        },
        ToolDef {
            name: "lyrics_current",
            description: "Lyrics of the song playing now.",
            input_schema: none,
        },
    ]
}

/// A playback command from `{ action, positionMs?, volume?, itemId? }` (MCP and the LAN remote).
pub(crate) fn parse_control(args: &Value) -> Result<PlaybackControl, String> {
    let action = text_arg(args, "action")?;
    Ok(match action.as_str() {
        "play" => PlaybackControl::Play,
        "pause" => PlaybackControl::Pause,
        "toggle" => PlaybackControl::Toggle,
        "next" => PlaybackControl::Next,
        "previous" => PlaybackControl::Previous,
        "seek" => PlaybackControl::Seek {
            position_ms: args
                .get("positionMs")
                .and_then(Value::as_f64)
                .filter(|position| *position >= 0.0)
                .ok_or("seek needs positionMs (milliseconds, 0 or more)")?,
        },
        "volume" => PlaybackControl::SetVolume {
            volume: args
                .get("volume")
                .and_then(Value::as_f64)
                .filter(|volume| (0.0..=1.0).contains(volume))
                .ok_or("volume needs a volume from 0 to 1")?,
        },
        "playItem" => PlaybackControl::PlayItem {
            item_id: text_arg(args, "itemId")?,
        },
        other => return Err(format!("Unknown action: {other}")),
    })
}

/// A required, non-empty string argument.
pub(crate) fn text_arg(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("{key} is required"))
}

/// A song from `title`, `artist` and `album` arguments.
pub(crate) fn track_arg(args: &Value) -> Result<TrackInput, String> {
    let optional = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    Ok(TrackInput {
        title: text_arg(args, "title")?,
        artist: optional("artist"),
        album: optional("album"),
        duration_ms: None,
        artwork_url: None,
        source: None,
    })
}

fn item_json(item: &QueueItem) -> Value {
    json!({ "title": item.title, "artist": item.artist, "album": item.album,
            "durationMs": item.duration_ms })
}

fn online_json(track: &OnlineTrack) -> Value {
    json!({ "title": track.title, "artist": track.artist, "album": track.album,
            "durationMs": track.duration_ms })
}

fn saved_json(track: &SavedTrack) -> Value {
    json!({ "title": track.title, "artist": track.artist, "album": track.album,
            "onDevice": track.local })
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_trimmed_and_required() {
        let args = json!({ "title": "  Feeling Good ", "artist": "", "album": "Pastel Blues" });
        let track = track_arg(&args).unwrap();
        assert_eq!(track.title, "Feeling Good");
        assert_eq!(track.artist, None, "blank means absent");
        assert_eq!(track.album.as_deref(), Some("Pastel Blues"));
        assert_eq!(track.source, None);

        assert_eq!(
            text_arg(&json!({ "query": "  " }), "query").unwrap_err(),
            "query is required"
        );
        assert!(track_arg(&json!({ "artist": "Nina Simone" })).is_err());
        assert!(text_arg(&json!({ "query": 5 }), "query").is_err());
    }

    #[test]
    fn controls_parse_with_their_arguments() {
        assert_eq!(
            parse_control(&json!({ "action": "toggle" })),
            Ok(PlaybackControl::Toggle)
        );
        assert_eq!(
            parse_control(&json!({ "action": "seek", "positionMs": 5000 })),
            Ok(PlaybackControl::Seek {
                position_ms: 5000.0
            })
        );
        assert_eq!(
            parse_control(&json!({ "action": "volume", "volume": 0.5 })),
            Ok(PlaybackControl::SetVolume { volume: 0.5 })
        );
        assert_eq!(
            parse_control(&json!({ "action": "playItem", "itemId": "q3" })),
            Ok(PlaybackControl::PlayItem {
                item_id: "q3".into()
            })
        );
        for bad in [
            json!({ "action": "seek" }),
            json!({ "action": "seek", "positionMs": -1 }),
            json!({ "action": "volume", "volume": 2 }),
            json!({ "action": "playItem" }),
            json!({ "action": "dance" }),
            json!({}),
        ] {
            assert!(parse_control(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn every_tool_has_an_object_schema_and_a_unique_name() {
        let tools = tool_defs();
        let mut names: Vec<_> = tools.iter().map(|tool| tool.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), tools.len());
        for tool in &tools {
            assert_eq!(tool.input_schema["type"], "object", "{}", tool.name);
            assert!(!tool.description.is_empty());
        }
        let playlist_add = tools
            .iter()
            .find(|tool| tool.name == "playlist_add")
            .unwrap();
        assert_eq!(
            playlist_add.input_schema["required"],
            json!(["name", "title"])
        );
        assert!(playlist_add.input_schema["properties"]["artist"].is_object());
    }
}
