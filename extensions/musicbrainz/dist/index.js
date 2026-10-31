// dev.raygun.musicbrainz 1.0.1. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var API = "https://musicbrainz.org/ws/2";
  var HEADERS = {
    "User-Agent": "Raygun/1.0 ( https://github.com/uvxszi/raygun )",
    Accept: "application/json"
  };
  var MIN_INTERVAL_MS = 1100;
  var RETRIES = 2;
  var FETCHED = 50;
  var escapeQuery = (query) => query.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, " ").trim();
  var words = (text) => text.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").split(/[^a-z0-9]+/).filter((word) => word.length > 2);
  var credit = (recording) => (recording["artist-credit"] ?? []).map((part) => part.name + (part.joinphrase ?? "")).join("").trim();
  var bestRelease = (releases = []) => releases.find(
    (release) => release.status === "Official" && release["release-group"]?.["primary-type"] === "Album"
  ) ?? releases.find((release) => release.status === "Official") ?? releases[0];
  var rank = (recordings, query) => {
    const asked = new Set(words(query));
    const scored = recordings.map((recording) => {
      const byNamedArtist = words(credit(recording)).some((word) => asked.has(word));
      const score = (recording.score ?? 0) + (byNamedArtist ? 60 : 0) + (recording.length ? 10 : 0);
      return { recording, score };
    });
    scored.sort((a, b) => b.score - a.score);
    const seen = /* @__PURE__ */ new Set();
    return scored.map(({ recording }) => recording).filter((recording) => {
      const key = `${words(recording.title).join(" ")}|${words(credit(recording)).join(" ")}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  };
  var toTrack = (recording) => {
    const release = bestRelease(recording.releases);
    return {
      id: recording.id,
      title: recording.title,
      artist: credit(recording) || null,
      album: release?.title ?? null,
      durationMs: recording.length ?? null,
      artworkUrl: release ? `https://coverartarchive.org/release/${release.id}/front-250` : null
    };
  };
  var lastRequestAt = 0;
  var fetchPaced = async (url) => {
    for (let attempt = 0; ; attempt += 1) {
      const wait = lastRequestAt + MIN_INTERVAL_MS - Date.now();
      if (wait > 0) await raygun.time.sleep(wait);
      lastRequestAt = Date.now();
      const response = await raygun.http.fetch(url, { headers: HEADERS });
      if (response.status !== 503 || attempt >= RETRIES) return response;
      raygun.log.info("MusicBrainz is busy, retrying");
      await raygun.time.sleep(MIN_INTERVAL_MS * (attempt + 1));
    }
  };
  var search = defineProvider({
    kind: "music.metadata",
    id: "recordings",
    name: "MusicBrainz",
    async search({ query, limit }) {
      const cleaned = escapeQuery(query);
      if (cleaned === "") return { tracks: [] };
      const url = `${API}/recording?query=${encodeURIComponent(cleaned)}&limit=${FETCHED}&fmt=json`;
      const response = await fetchPaced(url);
      if (response.status === 503) {
        throw new Error("MusicBrainz is busy; try again in a moment");
      }
      if (!response.ok) {
        throw new Error(`MusicBrainz answered HTTP ${response.status}`);
      }
      const body = await response.json();
      return {
        tracks: rank(body.recordings ?? [], cleaned).slice(0, limit).map(toTrack)
      };
    }
  });
  registerProvider(search);
})();
