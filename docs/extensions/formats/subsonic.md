# Subsonic, OpenSubsonic and Navidrome

Raygun connects to servers that speak the Subsonic API (version 1.16.1, with OpenSubsonic's
additions): Navidrome, Gonic, Airsonic-Advanced, Ampache, LMS and others. A server whose ping says
it's Navidrome (`"type": "navidrome"`) is connected as a Navidrome server; everything below works
with the others too, as far as they implement the API.

## Adding a server

1. Browse → Sources → Add, and type the server's address. Raygun asks the server's ping
   (`/rest/ping.view`), which answers without a sign-in, and says what it found.
2. The address can be typed as it is anywhere: `music.example.org`, `192.168.1.20:4533`, one with
   a path (`https://example.org/navidrome`), the API's own (`…/rest/ping.view`), or a page of
   Navidrome's web app or a share link (`https://example.org/app/#/album/…`,
   `https://example.org/share/…`). The server is at the address before `/app`, `/share` or
   `/rest`.
3. Sign in with a user name and password, or turn on **Use an API key** for servers with
   OpenSubsonic's API keys (`apiKeyAuthentication`).

Requests sign in with the salted token of the password (`t` and `s`). A server that can't check
tokens for a user (LDAP accounts) gets the password itself, hex-encoded, as the API allows. The
password stays in Raygun's encrypted secrets store.

### Coming from Feishin or Navic

Use the same server address and user as in Feishin's or Navic's server settings: the address
Feishin shows under its server's URL, or the one typed into Navic's login. Raygun doesn't read
their settings files; it signs in on its own. With **Sync with Raygun** on (below), stars,
playlists, plays and the queue are the same in Raygun as in those apps, since all of them keep
them on the server.

## What it brings

| Raygun | From the server |
|---|---|
| Search → Music | Songs, artists, albums (`search3`), and the user's playlists by name. |
| Album and artist pages | `getAlbum`; an artist's releases newest first (`getArtist`). Artists as OpenSubsonic's `displayArtist` (every credited artist). |
| Browse → Music (`music.charts`) | The library to browse: Recently added, Recently played, Most played, Starred, Highest rated (`getAlbumList2`), Random songs (`getRandomSongs`), each genre (`getGenres`, `getSongsByGenre`), each decade the library has music from, and Internet radio when the server lists stations (`getInternetRadioStations`). Genre and mood pages ask `tagSongs` (the genre whose name or other spelling matches). |
| Playing a song | `stream`: the file as it is (`format=raw`) when the Audio quality setting is the best; else with `maxBitRate`, which the server converts down to when the file is above it (Navidrome's default format for that, usually Opus); MP3 when Raygun decodes the stream itself (downloads, captions). |
| Lyrics | OpenSubsonic's `getLyricsBySongId` (`songLyrics`; timed when the server has timing, such as Navidrome's `.lrc` files beside songs; translations and pronunciations are left out), else `getLyrics`. |

Songs found on the server are played through its stream provider like any other song, so a saved
song keeps playing when the server's address changes, and plays from another source when the
server is gone.

## Sync with Raygun

Sources → Servers → the server's ⋮ → **Sync with Raygun** (Subsonic and Navidrome servers). Each
switch is per server and kept on this device.

| Switch | What it does |
|---|---|
| Favorites and stars (off) | Songs starred on the server are in Raygun's Favorites, and Raygun's Favorites are starred on the server when it has the song (`getStarred2`, `star`, `unstar`). A song taken out on one side is taken out on the other. |
| Playlists (off) | The user's playlists on the server and in Raygun are the same on both: made, renamed, changed and deleted on either side (`getPlaylists`, `getPlaylist`, `createPlaylist`, `updatePlaylist`, `deletePlaylist`). Playlists another user shares and smart playlists can't be changed here: Raygun's copy follows them. |
| Count plays (on) | A song played long enough to count in Raygun's history is counted on the server (`scrobble` with `submission=true`), which keeps its play counts and passes plays on to Last.fm or ListenBrainz when it's set up to. Nothing is counted while incognito. |
| Keep the play queue (off) | Raygun's queue, from the song playing on, and the place in it are kept on the server a few seconds after they change (`savePlayQueue`), for Feishin, Navic, the web app or Raygun on another device to go on from. |

**Play the server's queue** (in the same sheet) takes the queue the server keeps (`getPlayQueue`)
in place of Raygun's, paused at the song and place it was saved at.

Songs are matched by their names, as Raygun's saved songs are, so songs saved from any source
are starred and added to playlists when the server has them; the sheet says how many it doesn't.
Each sync compares both sides with what both held after the last one, so the first sync joins
both lists and later ones carry over additions and removals. Lists sync when a switch turns on,
every 15 minutes, a little after Raygun's Favorites or playlists change, and with **Sync now**.
Long lists of songs go in one form (OpenSubsonic's `formPost`) where the server takes forms.

## Limits

- Podcasts, bookmarks, shares and ratings aren't used.
- ReplayGain values the server sends aren't applied; Raygun's loudness normalization measures
  songs itself.
- Playlists can't be searched in the Subsonic API: Raygun filters the user's own by name.

## Checking

`navidrome_live` signs in to Navidrome's public demo (`demo.navidrome.org`, `demo`/`demo`):

```sh
cargo test -p raygun-ext-host -- --ignored navidrome_live --nocapture
```

`own_navidrome_live` checks browsing, lyrics, a stream converted down and the calls sync uses
against a Navidrome of one's own (`NAVIDROME_URL`, `NAVIDROME_USER`, `NAVIDROME_PASSWORD`;
default `http://127.0.0.1:4599`, `admin`/`raygun-test`) whose library holds generated test tones:

```sh
# Three tagged sine tones and a FLAC, with an .lrc beside the first (ffmpeg):
mkdir -p "music/Raygun Test Tones/Sine Album" "music/Second Artist/Square Things"
for i in 1 2 3; do f=$((220*i)); ffmpeg -f lavfi -i "sine=frequency=$f:duration=20" \
  -metadata title="Tone $f Hz" -metadata artist="Raygun Test Tones" -metadata album="Sine Album" \
  -metadata track=$i -metadata genre=Electronic -metadata date=2024 -b:a 192k \
  "music/Raygun Test Tones/Sine Album/0$i - Tone $f Hz.mp3"; done
printf '[00:00.00]First line of the tone\n[00:05.50]Second line\n[00:12.00]Third line\n' \
  > "music/Raygun Test Tones/Sine Album/01 - Tone 220 Hz.lrc"
ffmpeg -f lavfi -i "sine=frequency=330:duration=15" -metadata title="Quiet Square" \
  -metadata artist="Second Artist" -metadata album="Square Things" -metadata genre=Ambient \
  -metadata date=1999 -c:a flac "music/Second Artist/Square Things/01 - Quiet Square.flac"
# Navidrome's release binary, its admin made at start:
ND_MUSICFOLDER=music ND_DATAFOLDER=data ND_ADDRESS=127.0.0.1 ND_PORT=4599 \
  ND_DEVAUTOCREATEADMINPASSWORD=raygun-test ./navidrome
cargo test -p raygun-ext-host -- --ignored own_navidrome_live --nocapture
```
