// dev.raygun.apple-charts 1.1.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var FEED = "https://rss.marketingtools.apple.com/api/v2";
  var ITUNES = "https://itunes.apple.com";
  var DEFAULT_REGION = "us";
  var MAX_SONGS = 100;
  var HEADERS = { "User-Agent": "Raygun/1.0 (https://github.com/uvxszi/raygun)" };
  var GENRES = {
    Pop: 14,
    Rock: 21,
    "Hip-Hop": 18,
    "R&B": 15,
    Electronic: 7,
    Dance: 17,
    House: 1048,
    Jazz: 11,
    Classical: 5,
    Country: 6,
    Metal: 1153,
    Indie: 1004,
    Folk: 1289,
    Latin: 12,
    Brazilian: 1122,
    "K-Pop": 51,
    Reggae: 24,
    Blues: 2,
    Soul: 1143,
    Punk: 1006,
    Ambient: 1056,
    Soundtrack: 16,
    Anime: 29,
    Afrobeats: 100051,
    Gospel: 1101,
    Christian: 22,
    Alternative: 20
  };
  var CHART_GENRES = ["Pop", "Hip-Hop", "Rock", "Alternative", "R&B", "Country", "Dance", "Latin"];
  var fetchJson = async (url) => {
    let response = await raygun.http.fetch(url, { headers: HEADERS });
    if (response.status >= 500) {
      await raygun.time.sleep(1e3);
      response = await raygun.http.fetch(url, { headers: HEADERS });
    }
    if (!response.ok) throw new Error(`Apple's charts answered HTTP ${response.status}`);
    return response.json();
  };
  var region = (asked) => asked && /^[a-z]{2}$/i.test(asked) ? asked.toLowerCase() : DEFAULT_REGION;
  var bigArtwork = (url) => url ? url.replace(/\/\d+x\d+bb\./, "/600x600bb.") : null;
  var countryName = (code) => {
    try {
      return new Intl.DisplayNames(["en"], { type: "region" }).of(code.toUpperCase()) ?? code;
    } catch {
      return code.toUpperCase();
    }
  };
  var genreSongs = async (genre, country, limit) => {
    const count = Math.min(Math.max(limit, 1), MAX_SONGS);
    const page = await fetchJson(
      `${ITUNES}/${country}/rss/topsongs/limit=${count}/genre=${genre}/explicit=true/json`
    );
    const entries = page.feed?.entry;
    const list = Array.isArray(entries) ? entries : entries ? [entries] : [];
    return list.flatMap((entry) => {
      const title = entry["im:name"]?.label;
      if (!title) return [];
      const images = entry["im:image"] ?? [];
      return [
        {
          id: entry.id?.attributes?.["im:id"],
          title,
          artist: entry["im:artist"]?.label ?? null,
          album: entry["im:collection"]?.["im:name"]?.label?.replace(/ - Single$/, "") ?? null,
          artworkUrl: bigArtwork(images[images.length - 1]?.label)
        }
      ];
    });
  };
  registerProvider(
    defineProvider({
      kind: "music.charts",
      id: "charts",
      name: "Apple Music",
      async charts(args) {
        const country = region(args.region);
        const charts = [
          {
            id: "most-played",
            name: `Top songs: ${countryName(country)}`,
            description: "The most played songs on Apple Music",
            group: "top"
          }
        ];
        for (const name of CHART_GENRES) {
          charts.push({
            id: `genre:${GENRES[name]}`,
            name: `${name}: ${countryName(country)}`,
            description: `Top ${name} songs on the iTunes Store`,
            tag: name,
            group: "genre"
          });
        }
        return { charts };
      },
      async chart({ id, region: asked, limit }) {
        const country = region(asked);
        if (id.startsWith("genre:")) {
          const genre = Number(id.slice("genre:".length));
          if (!Number.isInteger(genre)) throw new Error(`Unknown chart ${id}`);
          return { tracks: await genreSongs(genre, country, limit) };
        }
        if (id !== "most-played") throw new Error(`Unknown chart ${id}`);
        const count = Math.min(Math.max(limit, 1), MAX_SONGS);
        const page = await fetchJson(
          `${FEED}/${country}/music/most-played/${count}/songs.json`
        );
        const tracks = (page.feed.results ?? []).map((song) => ({
          id: song.id,
          title: song.name,
          artist: song.artistName ?? null,
          artworkUrl: bigArtwork(song.artworkUrl100)
        }));
        return { tracks };
      },
      async tagSongs({ tag, region: asked, limit }) {
        const genre = GENRES[tag];
        if (genre === void 0) return { tracks: [] };
        return {
          tracks: await genreSongs(genre, region(asked), limit),
          name: `${tag} on Apple Music`
        };
      }
    })
  );
})();
