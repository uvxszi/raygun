# M20 — LAN remote

**Relevant docs:** ARCHITECTURE §9 (LAN remote: REST + SSE, remote web UI, QR pairing with a scoped token, off by default), M19 (MCP server, tool logic to share).

## Goal
Any phone or computer on the same Wi-Fi can control Raygun from a browser: see what's playing, play, pause, skip, seek, change volume, pick from the queue, and search and play songs. A device gets access by scanning a QR code shown in Raygun's Settings; no app install or account is needed.

## Design

### Server (`crates/raygun-remote`)
- Axum on `0.0.0.0`, on the first free port in 8820–8829, **only while the remote is switched on** (off by default).
- `GET /` serves the remote page: one self-contained HTML file with inline CSS and JS, embedded in the binary.
- **API** (JSON; every `/api` route except pairing needs `Authorization: Bearer <device token>`):
  - `POST /api/pair {code, name}` → `{token, deviceId}`;
  - `GET /api/state` → `{playback, queue}`;
  - `GET /api/events?token=…` → Server-Sent Events. The token is in the query because `EventSource` can't send headers. The server checks the snapshot every second and sends it when it changed, plus a keep-alive comment every 15 s;
  - `POST /api/control {action, positionMs?, volume?, itemId?}`;
  - `GET /api/search?q=` → tracks;
  - `POST /api/play {title, artist?, album?}`.
- A `Backend` trait (snapshot, control, search, play) is implemented by the app; tests use a fake.

### Pairing
- Settings shows a QR code for `http://<LAN IP>:<port>/#pair=<code>`, plus the address as text.
  - The code is 10 random characters, single-use, and expires after 10 minutes.
  - After 5 wrong codes, the current code is cancelled, so it can't be brute-forced.
- The page reads the code from the URL fragment (fragments never reach server logs or proxies), asks for a device name, and pairs. It keeps the token in `localStorage` and removes the fragment.
- **Paired devices** are listed in Settings with the date paired, and each can be removed, which cuts it off at once. Devices and tokens are device-local (`local_meta`) and never synced.
- **LAN IP:** the address of the interface the default route uses, found with a connected UDP socket (no packet is sent).

### What the page shows
- **Now playing:** artwork (http(s) artwork only; the local artwork server listens on loopback), title, artist, status, a progress bar you can tap to seek, previous / play-pause / next, and volume.
- **Queue** in play order: tap to jump to an item.
- **Search:** results from the library and providers; tap to play.
- **Errors and reconnection:** shows when the connection drops and reconnects with backoff. When the device was removed (401), it asks to pair again.

## Out of scope
- HTTPS on the LAN: no certificates for local addresses. The token keeps others out; traffic on the Wi-Fi isn't encrypted (as with most LAN remotes). The Settings card says so.
- mDNS discovery (M21 brings mDNS for sync).
- Library browsing, playlists and lyrics in the remote page (MCP covers agents; the page is a remote control).

## Done-check
- **Rust:**
  - pairing: one-time codes, expiry, lockout after 5 failures, a removed device is refused;
  - API auth, including the SSE query token;
  - control and search through a fake backend;
  - an SSE event arrives when the snapshot changes;
  - LAN IP parsing.
- **Vitest:** the Settings card (enable, QR and address shown, new code, remove a device).
- 📱 On the emulator:
  - turn the remote on;
  - `adb forward` the port, then open the pairing URL in the desktop browser pane;
  - pair, see now playing, play/pause and skip from the page and see the app follow, search and play a song;
  - remove the device in Settings, and the page asks to pair again.

## Verification (2026-10-31, Pixel_4 emulator)
- **Rust:**
  - `raygun-remote` (10 tests): one-time codes, expiry, lockout after 5 wrong codes, removed devices refused; the page is served without pairing; the API, including the SSE query token, needs a paired token; control, search and play reach the backend; the event stream sends the snapshot and then each change; ports and LAN-address filtering.
  - `raygun-app`: control parsing (seek, volume, playItem, bad input), the QR code as SVG, and artwork reachable from the network.
- **Vitest (app 96):** the Settings card turns the remote on with the QR code and address, makes a new code once one was used, picks up a device that paired elsewhere, and lists and removes devices.
- 📱 **On the emulator:**
  - Turning the remote on showed the QR code and `http://10.0.2.16:8820/#pair=…` (the emulator's network address; a phone shows its Wi-Fi address). The log shows "remote listening on the local network port=8820".
  - With `adb forward tcp:8820 tcp:8820`, the pairing URL opened in the desktop browser pane. It offered "Windows browser" as the name and paired; the log shows "remote device paired". The page then showed "Connected" and the empty queue.
  - Searching "blue in green davis" on the page listed Top results (only the song both MusicBrainz sources agreed on) and each source. Tapping "Blue in Green" by Miles Davis played it on the phone, and the page followed live: "via Internet Archive", progress at 0:09, the queue.
  - Pause on the page paused the phone (media session `PAUSED`).
  - Removing the device in Settings made the page drop to "This device isn't paired…" with the pairing form. Turning the remote off closed the port.

## Decisions and notes
- **Settings card refresh:** devices pair from another device, so the card refreshes every 3 s while the remote is on. Before this, it kept showing the used code until you left the page; the device run above used a build without the refresh.
- **Fragment for the code:** `#pair=` never reaches the server's request line, so the code doesn't land in logs or proxies.
- **Plain HTTP on the LAN:** there are no certificates for local addresses. Tokens keep others out, and the card says the Wi-Fi traffic isn't encrypted.
- **Live updates** come from checking the snapshot once a second, not from playback events. It's simple, and a remote doesn't need finer timing. The page moves the progress bar between updates.
