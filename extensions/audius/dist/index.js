// dev.raygun.audius 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var API = "https://api.audius.co/v1";
  var APP = "raygun";
  var MAX = 100;
  var GENRES = {
    Electronic: "Electronic",
    "Hip-Hop": "Hip-Hop/Rap",
    "Lo-fi": "Lo-Fi",
    Rock: "Rock",
    Metal: "Metal",
    Jazz: "Jazz",
    Classical: "Classical",
    Ambient: "Ambient",
    Hyperpop: "Hyperpop",
    "Drum & Bass": "Drum & Bass",
    Dubstep: "Dubstep",
    House: "House",
    Techno: "Techno",
    Trance: "Trance",
    Pop: "Pop",
    "R&B": "R&B/Soul",
    Soul: "R&B/Soul",
    Country: "Country",
    Folk: "Folk",
    Latin: "Latin",
    Reggae: "Reggae",
    Punk: "Punk",
    Soundtrack: "Soundtrack",
    Blues: "Blues",
    Alternative: "Alternative",
    "Jersey Club": "Jersey Club",
    Phonk: "Phonk",
    Trap: "Trap",
    Experimental: "Experimental",
    Funk: "Funk",
    Disco: "Disco",
    Acoustic: "Acoustic",
    Hardstyle: "Hardstyle",
    Downtempo: "Downtempo",
    Kids: "Kids",
    World: "World"
  };
  var CHART_GENRES = [
    "Electronic",
    "Hip-Hop",
    "Lo-fi",
    "Pop",
    "R&B",
    "Rock",
    "Alternative",
    "Drum & Bass",
    "House",
    "Phonk",
    "Hyperpop",
    "Ambient"
  ];
  var get = async (path) => {
    const join = path.includes("?") ? "&" : "?";
    const response = await raygun.http.fetch(`${API}${path}${join}app_name=${APP}`);
    if (!response.ok) throw new Error(`Audius answered HTTP ${response.status}`);
    const body = await response.json();
    if (body.data === void 0) throw new Error("Audius sent nothing back");
    return body.data;
  };
  var playable = (track) => track.is_streamable !== false && !track.is_delete && !track.stream_conditions;
  var stream = (id) => ({
    kind: "url",
    url: `${API}/tracks/${encodeURIComponent(id)}/stream?app_name=${APP}`,
    headers: {}
  });
  var toTrack = (track) => ({
    id: track.id,
    title: track.title,
    artist: track.user?.name ?? track.user?.handle ?? null,
    album: null,
    durationMs: track.duration ? track.duration * 1e3 : null,
    artworkUrl: track.artwork?.["1000x1000"] ?? track.artwork?.["480x480"] ?? null,
    source: stream(track.id)
  });
  var tracks = (found, limit) => found.filter(playable).slice(0, limit).map(toTrack);
  var TIMES = { week: "This week", month: "This month", allTime: "All time" };
  var trending = async (time, genre, limit) => {
    const params = [`time=${time}`, `limit=${Math.min(MAX, limit * 2)}`];
    if (genre) params.push(`genre=${encodeURIComponent(genre)}`);
    return tracks(await get(`/tracks/trending?${params.join("&")}`), limit);
  };
  registerProvider(
    defineProvider({
      kind: "music.charts",
      id: "charts",
      name: "Audius",
      async charts() {
        const charts = [
          {
            id: "trending:week",
            name: "Trending on Audius",
            description: "What artists on Audius are being played most this week",
            group: "trending"
          },
          {
            id: "underground",
            name: "Underground trending",
            description: "Rising artists on Audius with smaller followings",
            group: "underground"
          },
          {
            id: "trending:allTime",
            name: "Audius all-time",
            description: "The most played songs on Audius",
            group: "top"
          }
        ];
        for (const name of CHART_GENRES) {
          charts.push({
            id: `trending:week:${GENRES[name]}`,
            name: `${name} on Audius`,
            description: `Trending ${name} songs on Audius this week`,
            tag: name,
            group: "genre"
          });
        }
        return { charts };
      },
      async chart({ id, limit }) {
        if (id === "underground") {
          return {
            tracks: tracks(
              await get(
                `/tracks/trending/underground?limit=${Math.min(MAX, limit * 2)}`
              ),
              limit
            )
          };
        }
        const [kind, time, ...rest] = id.split(":");
        if (kind !== "trending" || !time || !(time in TIMES)) throw new Error(`Unknown chart ${id}`);
        return { tracks: await trending(time, rest.join(":") || null, limit) };
      },
      async tagSongs({ tag, limit }) {
        const genre = GENRES[tag];
        if (!genre) return { tracks: [] };
        return { tracks: await trending("month", genre, limit), name: `${tag} on Audius` };
      }
    })
  );
  registerProvider(
    defineProvider({
      kind: "music.metadata",
      id: "search",
      name: "Audius",
      async search({ query, limit }) {
        if (!query.trim()) return { tracks: await trending("week", null, limit) };
        const found = await get(
          `/tracks/search?query=${encodeURIComponent(query)}&limit=${Math.min(MAX, limit * 2)}`
        );
        return { tracks: tracks(found, limit) };
      }
    })
  );
  registerProvider(
    defineProvider({
      kind: "music.stream",
      id: "stream",
      name: "Audius",
      async candidates({ title, artist, limit }) {
        const query = [artist, title].filter(Boolean).join(" ").trim();
        if (!query) return { candidates: [] };
        const found = await get(
          `/tracks/search?query=${encodeURIComponent(query)}&limit=${Math.min(20, limit * 2)}`
        );
        return {
          candidates: found.filter(playable).slice(0, limit).map((track) => ({
            id: track.id,
            title: track.title,
            artist: track.user?.name ?? null,
            durationMs: track.duration ? track.duration * 1e3 : null,
            // Audius streams MP3 at 320 kbps.
            quality: "320 kbps MP3",
            bitrateKbps: 320,
            source: stream(track.id)
          }))
        };
      }
    })
  );
})();
