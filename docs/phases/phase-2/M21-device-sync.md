# M21 — Device sync v1

**Relevant docs:** ARCHITECTURE §5 (sync-ready tables), §10 (device sync), M18 (synced collections), M19/M20 (device-local secrets, pairing codes).

## Goal
Two of the user's devices on the same network keep settings, favorites, playlists and history in step, with no server or account. They pair once with a code. After that they recognise each other by key, the channel is encrypted, and conflicting edits resolve the same way on both sides. Settings → Devices shows paired devices, what syncs and a log, and can undo the last sync.

## Design

### Identity and channel (`crates/raygun-sync`)
- Each device has a long-term **X25519 static key** (device-local, `local_meta`). Its fingerprint is the first 8 bytes of the public key in hex, shown in groups.
- **Channel:** **Noise `XX`** (`Noise_XX_25519_ChaChaPoly_BLAKE2s`, crate `snow`) over TCP, with length-prefixed frames. After the handshake, each side knows the other's static key.
  - *Decision:* the Noise static key *is* the device identity, instead of separate Ed25519 keys (ARCHITECTURE §10). Noise already proves possession of the static key, and a second key pair would add nothing for v1.
- **Server:** `0.0.0.0`, first free port in 8830–8839, only while Device sync is on (off by default).
  - After the handshake it accepts only pairing (while a code is open) or a sync from a trusted key that matches the device id it claims.

### Pairing
- The host opens a code: 8 characters, single use, 10 minutes, cancelled after 5 wrong tries (as in M20). It shows a **pairing text**: `raygun-pair:<host>:<port>:<code>:<host device id>:<host public key hex>`, as copyable text and a QR code.
- The joiner pastes the text. It connects and **checks that the host's static key matches the key in the text before sending the code**, so a man in the middle never learns it. Then it sends `{pair, code, deviceId, name}`.
- The host checks the code and stores the joiner's key. Both store each other: id, name, key, last known address.
- Removing a device on one side stops syncing with it from that side; its key is no longer trusted.

### Change log and merge (`raygun-db::sync`, migration `0006_sync.sql`)
- **Synced tables:** `setting` (key), `music_favorite`, `music_playlist`, `music_playlist_item`, `music_play_history` (id).
- **`local_sync_log`:** triggers append `(seq, table, key)` on every insert and update of those tables, whether local or merged. A peer's watermark is "the last log `seq` it acknowledged", so rows that arrived from a third device are still forwarded.
- **Rows travel as JSON objects** (`json_object` / `json_extract`), with a table registry giving each table's key and columns.
- **Merge:** last writer wins per row, by `(updated_at_hlc, origin_device)`. Every incoming HLC is observed, so later local writes order after it. Tombstones are ordinary updates. History is append-only in practice (rows are never edited, only tombstoned).
  - *Decision:* per-row rather than per-field for v1. These rows are small and their fields change together; per-field merge can come with the episodic library.
- **Favorites conflict:** two devices favoriting the same song make two live rows with the same `track_key`. On merge, the smaller id stays live and the other is tombstoned with a fresh local stamp, so both devices settle on the same row.
- **Undo:** before a sync changes a row, its previous JSON (or "absent") is saved under the session in `local_sync_undo`. "Undo last sync" restores those rows as new local writes, which propagate on the next sync.

### Protocol (one session, half-duplex)
1. Initiator → `{hello, deviceId, name, kinds}`. The responder checks the key is trusted for that id → `{hello, deviceId, name, kinds}`.
2. Initiator → `{rows: [...]}` batches (at most ~48 KB each), then `{done, upto}`. The responder applies them and answers `{ack, upto}`.
3. Then the other direction, the same way. Each side moves its watermark for the peer on `ack`.
- Only kinds both sides have on are sent. Turning a kind on resets every peer's watermark, so its rows are sent again (merging is idempotent).

### What syncs (toggles, device-local)
- Settings; Favorites and playlists; History. All on by default once Device sync is on.

### Discovery
- **mDNS (`mdns-sd`):** advertise `_raygun-sync._tcp.local.` with TXT `id=<device id>`, and browse for paired ids to keep their addresses current.
- **Android:** a `WifiManager.MulticastLock` is held while the app is in the foreground (`MainActivity`), with the `CHANGE_WIFI_MULTICAST_STATE` permission.
- **Fallback:** the last known address (from pairing or the last sync). Emulators can't see each other's multicast, so device checks use it.

### When
- On app start (when on), right after pairing, and with **Sync now**. Automatic triggers are M22.

### UI: Settings → Devices (`/settings/devices`, linked from a Settings card)
- Device sync switch; this device's name and fingerprint.
- **Pair a device:** shows the pairing text and QR code.
- **Join a device:** a paste field.
- **Paired devices:** name, fingerprint, last sync, Sync now, Remove.
- What-syncs toggles.
- **Log:** the last sessions with rows sent and received, or the error, and **Undo last sync**.

### Test peer
`cargo run -p raygun-sync --example peer` runs the same engine on the PC against a database in a folder you name:
- `serve`;
- `join <text>`;
- `sync <host:port>`;
- `favorite <title> <artist>`;
- `list`.

It's a dev tool, not shipped.

## Out of scope
- Background sync, automatic triggers, file transfer (M22).
- Per-key settings opt-out, extension list sync, scanning QR codes with the camera.

## Done-check
- **Rust:**
  - the change log triggers;
  - row export and apply with LWW, including ties and tombstones;
  - the favorites conflict;
  - undo;
  - kind filtering and watermark reset;
  - pairing (a wrong key in the text is refused before the code is sent; wrong code; lockout);
  - an **end-to-end sync of two databases over loopback**: both directions, a conflict, a third-device forward, an untrusted key refused.
- **Vitest:** the Devices page (switch, pairing text, join, paired list with Sync now and Remove, toggles, log and undo).
- 📱 **Emulator ↔ PC test peer:**
  - pair both ways;
  - a favorite and a playlist made on the phone appear on the PC, and the reverse;
  - a conflicting edit resolves the same on both;
  - undo restores the phone's rows;
  - removing the peer stops syncing.

## Verification (2026-10-31)
- **Rust:**
  - `raygun-db` sync (7 tests): the change log keeps one entry per row and skips disabled kinds while moving the seq on; LWW merge with ties broken by device id; tombstones; the favorites conflict settles on one row; undo restores rows and the undo propagates; kind filtering; peers and sessions.
  - `raygun-sync` (10 tests): pairing-text round trip, including IPv6 and a capitalized prefix; message tags.
  - **End to end over loopback:** pair and sync both ways; a conflict resolving the same on both devices; a third device getting changes through a second; a forged key in the pairing text refused before the code is sent; wrong codes, lockout and expiry; an unpaired stranger and a removed device refused; kinds turned on later sent in full; a 1,500-row sync split into batches.
- **Vitest (app 101):** the Devices page (switch, pairing code and QR, copy, join, peers with Sync now and Remove, what-syncs toggles, log and undo) and the refresh after a sync event.
- 📱 **On the emulator: partly.** Device sync turned on and listened on 8830, and the Devices page rendered. Two PC peers paired and synced with each other over loopback using the test peer.
  - Pairing the emulator with the PC peer was stopped partway, when sync was put on hold. The attempts failed for two reasons: the keyboard capitalized the typed pairing text (fixed: the prefix is read case-insensitively and the field no longer auto-capitalizes), and my own adb input mistakes.
  - Phone ↔ PC sync is still to be checked when sync work resumes (M22).

## Decisions and notes
- **Echoes:** a merged row remembers the peer it came from (`from_peer` in the change log) and isn't sent back there; third devices still get it.
- **Kinds per peer:** each side records the kinds it last sent to a peer, and resends everything when that set grows. Turning a kind on can then reach the device that holds the rows.
- **The test peer listens on loopback by default.** An emulator reaches it as 10.0.2.2, and a loopback listener raises no firewall prompt.
- **Status:** sync is on hold by the user's call. The feature ships off by default.
