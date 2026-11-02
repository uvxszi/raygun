// dev.raygun.listenbrainz 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };
  var fetchJson = async (url, init) => {
    const response = await raygun.http.fetch(url, init);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status} from ${url}`);
    }
    return response.json();
  };

  // src/index.ts
  var API = "https://api.listenbrainz.org/1";
  var HEADERS = { "User-Agent": "Raygun/1.0 ( https://github.com/uvxszi/raygun )" };
  var RANGES = {
    week: { name: "Most listened this week", description: "Everyone on ListenBrainz, last week" },
    month: { name: "Most listened this month", description: "Everyone on ListenBrainz, last month" },
    year: { name: "Most listened this year", description: "Everyone on ListenBrainz, this year" }
  };
  var artwork = (recording) => recording.caa_id && recording.caa_release_mbid ? `https://coverartarchive.org/release/${recording.caa_release_mbid}/${recording.caa_id}-500.jpg` : null;
  registerProvider(
    defineProvider({
      kind: "music.charts",
      id: "charts",
      name: "ListenBrainz",
      async charts() {
        return {
          charts: Object.keys(RANGES).map((range) => ({ id: range, ...RANGES[range] }))
        };
      },
      async chart({ id, limit }) {
        if (!(id in RANGES)) throw new Error(`Unknown chart ${id}`);
        const count = Math.min(Math.max(limit, 1), 100);
        const stats = await fetchJson(
          `${API}/stats/sitewide/recordings?count=${count}&range=${id}`,
          { headers: HEADERS }
        );
        const tracks = (stats.payload?.recordings ?? []).map((recording) => ({
          id: recording.recording_mbid ?? void 0,
          title: recording.track_name,
          artist: recording.artist_name ?? null,
          album: recording.release_name ?? null,
          artworkUrl: artwork(recording)
        }));
        return { tracks };
      }
    })
  );
})();
