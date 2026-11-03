// dev.raygun.deezer 1.4.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var API = "https://api.deezer.com";
  var RETRY_AFTER_MS = 1500;
  var GENRE_CHARTS = {
    Pop: 132,
    "Hip-Hop": 116,
    Rap: 116,
    Reggaeton: 122,
    Rock: 152,
    Dance: 113,
    "R&B": 165,
    Alternative: 85,
    Christian: 186,
    Electronic: 106,
    Folk: 466,
    Reggae: 144,
    Jazz: 129,
    Country: 84,
    Salsa: 67,
    Classical: 98,
    Soundtrack: 173,
    Metal: 464,
    Soul: 169,
    Funk: 169,
    Afrobeats: 2,
    African: 2,
    Blues: 153,
    Brazilian: 75,
    Cumbia: 71,
    Indian: 81,
    Kids: 95,
    Latin: 197
  };
  var RADIOS = {
    Happy: 38235,
    Sad: 39051,
    Chill: 37121,
    Focus: 38225,
    Party: 36871,
    Workout: 36901,
    Romantic: 36861,
    Sleep: 38955,
    Energetic: 648,
    EDM: 648,
    "Bossa Nova": 39101,
    Ska: 31121,
    Indie: 30771,
    Techno: 30851,
    Disco: 30841,
    Gospel: 39071,
    Grime: 40622,
    House: 36891,
    Dubstep: 38215,
    Acoustic: 38435,
    Opera: 37061,
    "'60s": 38285,
    "'70s": 38295,
    "'80s": 38305,
    "'90s": 38315,
    "2000s": 38325
  };
  var DECADES = ["'60s", "'70s", "'80s", "'90s", "2000s"];
  var get = async (path) => {
    for (let attempt = 0; ; attempt += 1) {
      const response = await raygun.http.fetch(`${API}${path}`);
      if (!response.ok) throw new Error(`Deezer answered HTTP ${response.status}`);
      const body = await response.json();
      if (!body.error) return body;
      if (body.error.code === 4 && attempt === 0) {
        await raygun.time.sleep(RETRY_AFTER_MS);
        continue;
      }
      throw new Error(`Deezer: ${body.error.message ?? "error"}`);
    }
  };
  var toTrack = (track) => ({
    id: String(track.id),
    title: track.title,
    artist: track.artist?.name ?? null,
    album: track.album?.title ?? null,
    durationMs: track.duration ? track.duration * 1e3 : null,
    artworkUrl: track.album?.cover_xl ?? track.album?.cover_big ?? null,
    explicit: track.explicit_lyrics ?? null
  });
  registerProvider(
    defineProvider({
      kind: "music.charts",
      id: "charts",
      name: "Deezer",
      async charts() {
        const genres = await get("/genre");
        const charts = [
          {
            id: "0",
            name: "Top songs on Deezer",
            description: "The most played songs today",
            group: "top"
          }
        ];
        for (const genre of genres.data ?? []) {
          if (genre.id === 0) continue;
          charts.push({
            id: String(genre.id),
            name: genre.name,
            description: `Top ${genre.name} songs on Deezer`,
            artworkUrl: genre.picture_xl ?? genre.picture_big ?? null,
            tag: genre.name,
            group: "genre"
          });
        }
        for (const decade of DECADES) {
          charts.push({
            id: `radio:${RADIOS[decade]}`,
            name: `The ${decade}`,
            description: `Deezer's radio of the ${decade}`,
            group: "decade"
          });
        }
        return { charts };
      },
      async chart({ id, limit }) {
        if (id.startsWith("radio:")) return { tracks: await radio(id.slice("radio:".length), limit) };
        if (!/^\d+$/.test(id)) throw new Error(`Unknown chart ${id}`);
        return { tracks: await genreChart(Number(id), limit) };
      },
      async tagSongs({ tag, aliases, limit }) {
        const names = [tag, ...aliases];
        const chart = names.map((name) => GENRE_CHARTS[name]).find((id) => id !== void 0);
        if (chart !== void 0) {
          return { tracks: await genreChart(chart, limit), name: `${tag} on Deezer` };
        }
        const station = names.map((name) => RADIOS[name]).find((id) => id !== void 0);
        if (station !== void 0) {
          return { tracks: await radio(String(station), limit), name: `${tag} radio on Deezer` };
        }
        return playlistFor(tag, limit);
      }
    })
  );
  var genreChart = async (id, limit) => {
    const page = await get(
      `/chart/${id}/tracks?limit=${Math.min(Math.max(limit, 1), 100)}`
    );
    return (page.data ?? []).map(toTrack);
  };
  var radio = async (id, limit) => {
    if (!/^\d+$/.test(id)) throw new Error(`Unknown radio ${id}`);
    const page = await get(
      `/radio/${id}/tracks?limit=${Math.min(Math.max(limit, 1), 100)}`
    );
    return (page.data ?? []).map(toTrack);
  };
  var playlistFor = async (tag, limit) => {
    const found = await get(
      `/search/playlist?q=${encodeURIComponent(tag)}&limit=25`
    );
    const wanted = simple(tag);
    const named = (found.data ?? []).filter(
      (playlist) => simple(playlist.title).includes(wanted) && (playlist.nb_tracks ?? 0) >= 15
    );
    const editorial = named.find((playlist) => /deezer/i.test(playlist.user?.name ?? ""));
    const chosen = editorial ?? [...named].sort((a, b) => (b.nb_tracks ?? 0) - (a.nb_tracks ?? 0))[0] ?? null;
    if (!chosen) return { tracks: [] };
    const page = await get(
      `/playlist/${chosen.id}/tracks?limit=${Math.min(Math.max(limit, 1), 100)}`
    );
    return { tracks: (page.data ?? []).map(toTrack), name: `${chosen.title} (Deezer)` };
  };
  var simple = (text) => text.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/\(.*?\)|\[.*?\]/g, " ").replace(/[^a-z0-9]+/g, " ").trim();
  var sameSong = (track, query) => {
    const title = simple(query.title);
    const found = simple(track.title_short ?? track.title);
    if (!title || !(found === title || found.startsWith(`${title} `))) return false;
    if (!query.artist) return true;
    const wanted = simple(query.artist);
    const artist = simple(track.artist?.name ?? "");
    return Boolean(artist) && (wanted.includes(artist) || artist.includes(wanted));
  };
  registerProvider(
    defineProvider({
      kind: "music.tags",
      id: "genres",
      name: "Deezer",
      async tags(query) {
        const terms = [query.artist, query.title].filter(Boolean).join(" ");
        const found = await get(
          `/search?q=${encodeURIComponent(terms)}&limit=10`
        );
        const match = (found.data ?? []).find((track) => sameSong(track, query));
        if (!match?.album) return { tags: [] };
        const album = await get(
          `/album/${match.album.id}`
        );
        const tags = (album.genres?.data ?? []).map((genre, index) => ({
          name: genre.name,
          weight: index === 0 ? 1 : 0.7
        }));
        return { tags };
      }
    })
  );
  var toAlbum = (album) => ({
    id: String(album.id),
    title: album.title,
    artist: album.artist?.name ?? null,
    year: album.release_date ? Number(album.release_date.slice(0, 4)) || null : null,
    artworkUrl: album.cover_xl ?? album.cover_big ?? null,
    trackCount: album.nb_tracks ?? null,
    kind: kindOf(album.record_type)
  });
  var toPlaylist = (playlist) => {
    const author = playlist.user?.name ?? playlist.creator?.name ?? null;
    return {
      id: String(playlist.id),
      title: playlist.title,
      author,
      artworkUrl: playlist.picture_xl ?? playlist.picture_big ?? null,
      trackCount: playlist.nb_tracks ?? null,
      // Deezer's editors sign as "Rudy - Deezer Moods Editor".
      featured: /\bdeezer\b/i.test(author ?? "")
    };
  };
  var kindOf = (type) => {
    switch (type) {
      case "album":
      case "single":
      case "ep":
        return type;
      case "compile":
        return "compilation";
      default:
        return null;
    }
  };
  var limitOf = (limit) => Math.min(Math.max(limit, 1), 50);
  registerProvider(
    defineProvider({
      kind: "music.metadata",
      id: "catalogue",
      name: "Deezer",
      async search({ query, limit }) {
        const found = await get(
          `/search?q=${encodeURIComponent(query)}&limit=${limitOf(limit)}`
        );
        return { tracks: (found.data ?? []).map(toTrack) };
      },
      async searchArtists({ query, limit }) {
        const found = await get(
          `/search/artist?q=${encodeURIComponent(query)}&limit=${limitOf(limit)}`
        );
        const artists2 = (found.data ?? []).map((artist) => ({
          id: String(artist.id),
          name: artist.name,
          imageUrl: artist.picture_xl ?? artist.picture_big ?? null
        }));
        return { artists: artists2 };
      },
      async searchAlbums({ query, limit }) {
        const found = await get(
          `/search/album?q=${encodeURIComponent(query)}&limit=${limitOf(limit)}`
        );
        return { albums: (found.data ?? []).map(toAlbum) };
      },
      async album({ id }) {
        if (!/^\d+$/.test(id)) throw new Error(`Unknown album ${id}`);
        const album = await get(`/album/${id}`);
        const tracks = (album.tracks?.data ?? []).map(
          (track) => toTrack({
            ...track,
            album: track.album ?? {
              id: album.id,
              title: album.title,
              cover_xl: album.cover_xl,
              cover_big: album.cover_big
            }
          })
        );
        return { album: toAlbum(album), tracks };
      },
      async searchPlaylists({ query, limit, featured }) {
        const found = await get(
          `/search/playlist?q=${encodeURIComponent(query)}&limit=${featured ? 50 : limitOf(limit)}`
        );
        const playlists = (found.data ?? []).map(toPlaylist).filter((playlist) => !featured || playlist.featured).slice(0, limitOf(limit));
        return { playlists };
      },
      async playlist({ id }) {
        if (!/^\d+$/.test(id)) throw new Error(`Unknown playlist ${id}`);
        const playlist = await get(
          `/playlist/${id}`
        );
        return { playlist: toPlaylist(playlist), tracks: (playlist.tracks?.data ?? []).map(toTrack) };
      },
      async artistReleases({ query, limit }) {
        const artist = await artistNamed(query);
        if (!artist) return { releases: [] };
        const found = await get(
          `/artist/${artist.id}/albums?limit=${Math.min(Math.max(limit, 1), 100)}`
        );
        const releases = (found.data ?? []).map(
          (album) => toAlbum({ ...album, artist: album.artist ?? { name: artist.name } })
        );
        return { releases };
      }
    })
  );
  var artists = /* @__PURE__ */ new Map();
  var artistNamed = async (name) => {
    const wanted = simple(name);
    const known = artists.get(wanted);
    if (known) return known;
    const found = await get(
      `/search/artist?q=${encodeURIComponent(name)}&limit=10`
    );
    const list = found.data ?? [];
    const named = list.filter((candidate) => simple(candidate.name) === wanted).sort((a, b) => (b.nb_fan ?? 0) - (a.nb_fan ?? 0));
    const artist = named[0] ?? list[0] ?? null;
    if (artist) {
      if (artists.size > 500) artists.clear();
      artists.set(wanted, artist);
    }
    return artist;
  };
  var DAY_MS = 24 * 60 * 60 * 1e3;
  registerProvider(
    defineProvider({
      kind: "music.recommend",
      id: "recommend",
      name: "Deezer",
      async similarArtists({ artist, limit }) {
        const found = await artistNamed(artist);
        if (!found) return { artists: [] };
        const related = await get(
          `/artist/${found.id}/related?limit=${limitOf(limit)}`
        );
        const artists2 = (related.data ?? []).map((other) => ({
          id: String(other.id),
          name: other.name,
          imageUrl: other.picture_xl ?? other.picture_big ?? null
        }));
        return { artists: artists2 };
      },
      async topTracks({ artist, limit }) {
        const found = await artistNamed(artist);
        if (!found) return { tracks: [] };
        const top = await get(
          `/artist/${found.id}/top?limit=${limitOf(limit)}`
        );
        return { tracks: (top.data ?? []).map(toTrack) };
      },
      async newReleases({ artist, days, limit }) {
        const found = await artistNamed(artist);
        if (!found) return { tracks: [] };
        const albums = await get(`/artist/${found.id}/albums?limit=25`);
        const since = Date.now() - Math.max(1, days) * DAY_MS;
        const recent = (albums.data ?? []).filter((album) => album.release_date && Date.parse(album.release_date) >= since).sort((a, b) => Date.parse(b.release_date ?? "") - Date.parse(a.release_date ?? "")).slice(0, 3);
        const tracks = [];
        for (const album of recent) {
          const page = await get(`/album/${album.id}/tracks?limit=50`);
          for (const track of page.data ?? []) {
            tracks.push(
              toTrack({
                ...track,
                artist: track.artist ?? { name: found.name },
                album: {
                  id: album.id,
                  title: album.title,
                  cover_xl: album.cover_xl,
                  cover_big: album.cover_big
                }
              })
            );
          }
        }
        return { tracks: tracks.slice(0, limitOf(limit)) };
      },
      async radio({ artist, limit }) {
        const found = await artistNamed(artist);
        if (!found) return { tracks: [] };
        const radio2 = await get(
          `/artist/${found.id}/radio?limit=${Math.min(Math.max(limit, 1), 100)}`
        );
        return { tracks: (radio2.data ?? []).map(toTrack) };
      }
    })
  );
})();
