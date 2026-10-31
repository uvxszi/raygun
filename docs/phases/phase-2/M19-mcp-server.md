# M19 — MCP server

**Relevant docs:** ARCHITECTURE §9 (MCP server and remote API), §10 (what never syncs), M18 (collections), M14 (resolver).

## Goal
An AI agent can control Raygun over the Model Context Protocol: see what's playing, search, play songs, manage the queue, favorites and playlists, and read lyrics. Access needs a token, and the server only listens on this device.

## Design

### Transport (`crates/raygun-mcp`)
- MCP **streamable HTTP** at `http://127.0.0.1:<port>/mcp`, on the first free port in 8810–8819.
  - `POST` takes one JSON-RPC 2.0 message and answers with `application/json`. The transport allows plain JSON instead of an SSE stream, so there's no streaming.
  - `GET` answers 405, since the server offers no server-initiated stream. Notifications get 202 and no body.
- **Hand-rolled on axum** (already a dependency) instead of rmcp: the server needs only `initialize`, `ping`, `tools/list`, `tools/call`, `resources/list` and `resources/read`. That fits in a small, fully testable module, without tracking a fast-moving SDK. Protocol version `2025-06-18`. `Mcp-Session-Id` is issued on `initialize` and checked after that.
- **The crate is pure plumbing:** a `Backend` trait (tool definitions, `call_tool`, resources) that `raygun-app` implements. Tests run the router against a fake backend.

### Security
- Every request needs `Authorization: Bearer <token>`. The token is 32 random bytes in hex, generated on first use.
- **`Host` must be a loopback name and `Origin`, when sent, must be loopback**: protection against DNS rebinding.
- **Off by default.** Settings → MCP server turns it on, shows the URL and token, copies them, and makes a new token (which disconnects existing clients).
- The token and the on/off switch are device-local (`local_meta`), never synced settings: the token is a secret and the server belongs to one device (ARCHITECTURE §10).

### Tools (direct, for ergonomics)
| Tool | Does |
|---|---|
| `now_playing` | Status, position, current track and where its stream came from |
| `search_music {query}` | Library matches, top results and each provider's first tracks |
| `play_track {title, artist?, album?}` | Plays a song now: local file if the library has it, else the resolver finds a stream |
| `queue_add {title, artist?, album?}` | Same matching, appended to the queue |
| `queue_get` | The queue in play order, with the current item marked |
| `playback_control {action, positionMs?}` | play, pause, toggle, next, previous, seek |
| `favorites_list`, `favorite_set {title, artist?, favorite}` | Favorites |
| `playlists_list`, `playlist_play {name}`, `playlist_add {name, title, artist?}` | Playlists, matched by name (case-insensitive); `playlist_add` creates the playlist when needed |
| `history_recent {limit?}` | Recently played |
| `lyrics_current` | Lyrics for the current track |

Results are short JSON text, so agents spend few tokens. Errors come back as `isError` tool results with a message, not protocol errors.

### Resources
- `raygun://now-playing` and `raygun://queue`, the same JSON as `now_playing` and `queue_get`.

## Out of scope (later)
- Nuclear-style meta-tools (`list_domains`, `call`) generated from a service catalog: they pay off once manga and novels add more domains.
- The `raygun-mcp` stdio shim (desktop; Android has no stdio clients).
- LAN access, REST/SSE and the remote web UI (M20).

## Done-check
- **Rust:**
  - transport: auth (missing or wrong token, bad `Host` or `Origin`); initialize gives a session; tools and resources through a fake backend; notifications; unknown methods; malformed JSON;
  - backend helpers: argument parsing and result shaping;
  - token and switch storage.
- **Vitest:** the Settings card (enable, show the token, copy, new token).
- 📱 On the emulator:
  - enable the server in Settings;
  - `adb forward` the port, then from the PC run `initialize`, `tools/list`, `now_playing`, `play_track` (the app plays it), `playlist_add` and `resources/read`;
  - a request without the token is refused.

## Verification (2026-10-31, Pixel_4 emulator)
- **Rust:**
  - `raygun-mcp` (8 tests): missing or wrong token refused, foreign `Host` or `Origin` refused, loopback origins allowed; `initialize` issues a session, unknown and ended sessions get 404; tools and resources through a fake backend, with tool failures as `isError` results; notifications get 202; parse errors, batches and unknown methods; a new token applies at once; binds the next free loopback port.
  - `raygun-app`: argument parsing and the tool schemas.
  - `raygun-db`: device-local values stay out of synced settings.
- **Vitest (app 92):** the Settings card turns the server on and shows the address, hides and shows the token, copies it, makes a new token, and shows a start error.
- 📱 **On the emulator,** after turning the switch on (the log shows "MCP server listening address=127.0.0.1:8810") and `adb forward tcp:8810 tcp:8810`, a PC client:
  - was refused (401) without a token or with a wrong one;
  - got a session from `initialize` and the tool list from `tools/list`; `now_playing` reported idle;
  - asked `play_track` for "Blue in Green" by Miles Davis: the app resolved it through Internet Archive and played it. `resources/read raygun://now-playing` then showed it playing at 11 s;
  - had `playlist_add` create "From MCP" with "So What"; `playlists_list` and `history_recent` answered; a `play_track` with no title came back as a tool error.
  - After "New token" the old token got 401, and after switching off the connection was refused.

## Decisions and notes
- **Transport hand-rolled on axum,** not rmcp: the needed subset is small, it's fully testable without a network, and there's no fast-moving SDK API to track. Answers are plain JSON, which the streamable-HTTP transport allows; switching to rmcp later changes nothing for clients.
- **Off by default, with the token device-local:** the server can control the whole app. On Android only apps on the phone (or `adb forward`) can reach it.
- Songs played through MCP carry only title and artist, so the player shows no artwork until the resolver's pick has some. Filling artwork from search would be a follow-up.
