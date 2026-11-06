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
  var API = "https://api.listenbrainz.org/1/stats/sitewide/recordings";
  var HEADERS = {
    "User-Agent": "Raygun/1.0 ( https://github.com/uvxszi/raygun )",
    Accept: "application/json"
  };
  var MOST = 100;
  var RANGES = [
    { id: "week", name: "Top songs this week", description: "Most listened to on ListenBrainz" },
    { id: "month", name: "Top songs this month", description: "Most listened to on ListenBrainz" },
    { id: "year", name: "Top songs this year", description: "Most listened to on ListenBrainz" },
    {
      id: "all_time",
      name: "Top songs of all time",
      description: "Most listened to on ListenBrainz"
    }
  ];
  var artwork = (recording) => recording.caa_id && recording.caa_release_mbid ? `https://coverartarchive.org/release/${recording.caa_release_mbid}/${recording.caa_id}-250.jpg` : null;
  var toTrack = (recording, index) => ({
    id: recording.recording_mbid ?? `${index}`,
    title: recording.track_name,
    artist: recording.artist_name ?? null,
    album: recording.release_name ?? null,
    artworkUrl: artwork(recording)
  });
  registerProvider(
    defineProvider({
      kind: "music.charts",
      id: "charts",
      name: "ListenBrainz",
      async charts() {
        const charts = RANGES.map((range) => ({ ...range, group: "top" }));
        return { charts };
      },
      async chart({ id, limit }) {
        const range = RANGES.find((known) => known.id === id)?.id ?? "week";
        const count = Math.max(1, Math.min(limit || 50, MOST));
        const answer = await fetchJson(`${API}?range=${range}&count=${count}`, {
          headers: HEADERS
        });
        const tracks = (answer.payload?.recordings ?? []).filter((recording) => recording.track_name).map(toTrack);
        return { tracks };
      }
    })
  );
})();
