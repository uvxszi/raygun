// dev.raygun.lrclib 1.0.1. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var API = "https://lrclib.net/api";
  var HEADERS = { "User-Agent": "Raygun/1.0 (https://github.com/uvxszi/raygun)" };
  var MAX_LENGTH_GAP_S = 10;
  var encode = (params) => Object.entries(params).filter(([, value]) => value !== null && value !== void 0 && value !== "").map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`).join("&");
  var TIMESTAMP = /\[(\d+):(\d{1,2})(?:[.:](\d{1,3}))?\]/g;
  var parseLrc = (lrc) => {
    const lines = [];
    for (const row of lrc.split(/\r?\n/)) {
      const stamps = [...row.matchAll(TIMESTAMP)];
      if (stamps.length === 0) continue;
      const text = row.replace(TIMESTAMP, "").trim();
      for (const [, minutes, seconds, fraction = "0"] of stamps) {
        const fractionMs = Number(fraction.padEnd(3, "0").slice(0, 3));
        lines.push({ timeMs: (Number(minutes) * 60 + Number(seconds)) * 1e3 + fractionMs, text });
      }
    }
    return lines.sort((a, b) => a.timeMs - b.timeMs);
  };
  var toResult = (record) => {
    if (record.instrumental) return null;
    const synced = record.syncedLyrics ? parseLrc(record.syncedLyrics) : [];
    const plain = record.plainLyrics?.trim() || null;
    return synced.length > 0 || plain ? { synced, plain } : null;
  };
  var getExact = async (query) => {
    if (!query.artist?.trim()) return null;
    const params = encode({
      track_name: query.title,
      artist_name: query.artist,
      album_name: query.album,
      duration: query.durationMs ? Math.round(query.durationMs / 1e3) : null
    });
    const response = await raygun.http.fetch(`${API}/get?${params}`, { headers: HEADERS });
    if (response.status === 404 || response.status === 400) return null;
    if (!response.ok) throw new Error(`LRCLIB answered HTTP ${response.status}`);
    return response.json();
  };
  var searchClosest = async (query) => {
    const params = query.artist?.trim() ? encode({ track_name: query.title, artist_name: query.artist }) : encode({ q: query.title });
    const response = await raygun.http.fetch(`${API}/search?${params}`, { headers: HEADERS });
    if (response.status === 400 || response.status === 404) return null;
    if (!response.ok) throw new Error(`LRCLIB answered HTTP ${response.status}`);
    const records = await response.json();
    const wanted = query.durationMs ? query.durationMs / 1e3 : null;
    const gap = (record) => wanted && record.duration ? Math.abs(record.duration - wanted) : MAX_LENGTH_GAP_S;
    const usable = records.filter((record) => gap(record) <= MAX_LENGTH_GAP_S).sort((a, b) => Number(!a.syncedLyrics) - Number(!b.syncedLyrics) || gap(a) - gap(b));
    return usable[0] ?? null;
  };
  var lyrics = defineProvider({
    kind: "lyrics",
    id: "lyrics",
    name: "LRCLIB",
    async lyrics(query) {
      if (!query.title.trim()) return null;
      const exact = await getExact(query);
      if (exact) {
        const result = toResult(exact);
        if (result?.synced?.length || exact.instrumental) return result;
      }
      const found = await searchClosest(query);
      return (found && toResult(found)) ?? (exact && toResult(exact));
    }
  });
  registerProvider(lyrics);
})();
