# Jellyfin and Emby

Raygun connects to a Jellyfin or Emby server as a source of films, series and music.

## Adding a server

1. Browse → Sources → Servers → Add server.
2. Choose Jellyfin (or Emby; Raygun tells them apart by what the server says it is).
3. Type the server's address, as the web interface has it: `media.example.org`,
   `192.168.1.20:8096`, or `https://example.org/jellyfin`.
4. Sign in with a user name and password, or turn on **Use an API key** and paste a key from the
   server's dashboard (Administration → API Keys).

Raygun keeps the access token the server gives at sign-in, not the password. Adding the same
server for the same user again signs in anew.

## What it brings

| Raygun | From the server |
|---|---|
| Browse → Sources → Videos | Films and series, best rated first; Latest is by date added. Filters: library, films or series, sort, genre, not watched, favorites. |
| Title pages | Overview, genres, year, cast, directors, series status. A film is its one episode; a series lists every season's episodes. |
| Playing a video | Two choices per version of the file: **Original** (the file as it is, with its own subtitle tracks and the external ones) and **Converted** (H.264 and AAC over HLS, for players that can't decode the original, with the text subtitles sent apart). Chapters mark the timeline; intro, credits and recap segments (Jellyfin 10.10 and newer) can be skipped. |
| Search → Music | Songs, artists, albums and the user's playlists. |
| Playing a song | The server's universal stream: the file when its format and bitrate fit the Audio quality setting, else converted to MP3. |
| Lyrics | The server's own (Jellyfin 10.9 and newer), timed when the file has timing. |

## Limits

- Raygun doesn't report playback to the server: watched marks and positions stay in Raygun.
- Live TV, photos, books and Jellyfin's audiobook libraries aren't listed.
- Emby is served by the same client (Jellyfin's API forked from Emby's); it hasn't been checked
  against an Emby server.

## Checking

`jellyfin_live` signs in to Jellyfin's public demo (`demo.jellyfin.org/stable`, user `demo`),
opens a title's servers, and resolves a song's stream:

```sh
cargo test -p raygun-ext-host -- --ignored jellyfin_live --nocapture
```
