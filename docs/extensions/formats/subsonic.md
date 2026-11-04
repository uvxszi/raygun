# Subsonic and OpenSubsonic

Raygun connects to servers that speak the Subsonic API (version 1.16.1, with OpenSubsonic's
additions): Navidrome, Gonic, Airsonic-Advanced, Ampache, LMS and others.

## Adding a server

1. Browse → Sources → Servers → Add server, and choose Subsonic.
2. Type the server's address: `music.example.org`, `192.168.1.20:4533`, or one with a path
   (`https://example.org/navidrome`).
3. Sign in with a user name and password, or turn on **Use an API key** for servers with
   OpenSubsonic's API keys.

Requests sign in with the salted token of the password (`t` and `s`). A server that can't check
tokens for a user (LDAP accounts) gets the password itself, hex-encoded, as the API allows. The
password stays in Raygun's encrypted secrets store.

## What it brings

| Raygun | From the server |
|---|---|
| Search → Music | Songs, artists, albums (`search3`), and the user's playlists by name. |
| Album and artist pages | `getAlbum`; an artist's releases newest first (`getArtist`). |
| Playing a song | `stream`: the file as it is, or converted down to the Audio quality setting's bitrate (`maxBitRate`); MP3 when Raygun must decode it itself (downloads, captions) and the file's format is one it can't. |
| Lyrics | OpenSubsonic's `getLyricsBySongId` (timed when the server has timing), else `getLyrics`. |

Songs found on the server are played through its stream provider like any other song, so a
saved song keeps playing when the server's address changes, and plays from another source when
the server is gone.

## Limits

- Podcasts, internet radio, bookmarks and play queues kept by the server aren't used.
- Stars and ratings aren't sent to the server.
- Playlists can't be searched in the Subsonic API: Raygun filters the user's own by name.

## Checking

`navidrome_live` signs in to Navidrome's public demo (`demo.navidrome.org`, `demo`/`demo`):

```sh
cargo test -p raygun-ext-host -- --ignored navidrome_live --nocapture
```
