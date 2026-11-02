// dev.raygun.deezer 1.1.0. Built by raygun-ext from src/index.ts; do not edit.
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
    artworkUrl: track.album?.cover_xl ?? track.album?.cover_big ?? null
  });
  registerProvider(
    defineProvider({
      kind: "music.charts",
      id: "charts",
      name: "Deezer",
      async charts() {
        const genres = await get("/genre");
        const charts = [
          { id: "0", name: "Top songs on Deezer", description: "The most played songs today" }
        ];
        for (const genre of genres.data ?? []) {
          if (genre.id === 0) continue;
          charts.push({
            id: String(genre.id),
            name: genre.name,
            description: `Top ${genre.name} songs on Deezer`,
            artworkUrl: genre.picture_xl ?? genre.picture_big ?? null,
            tag: genre.name
          });
        }
        return { charts };
      },
      async chart({ id, limit }) {
        if (!/^\d+$/.test(id)) throw new Error(`Unknown chart ${id}`);
        const page = await get(
          `/chart/${id}/tracks?limit=${Math.min(Math.max(limit, 1), 100)}`
        );
        return { tracks: (page.data ?? []).map(toTrack) };
      }
    })
  );
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
    trackCount: album.nb_tracks ?? null
  });
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
        const artists = (found.data ?? []).map((artist) => ({
          id: String(artist.id),
          name: artist.name,
          imageUrl: artist.picture_xl ?? artist.picture_big ?? null
        }));
        return { artists };
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
      }
    })
  );
})();
