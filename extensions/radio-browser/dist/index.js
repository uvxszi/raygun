// dev.raygun.radio-browser 1.0.1. Built by raygun-ext from src/index.ts; do not edit.
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
  var DIRECTORY = "https://all.api.radio-browser.info";
  var FALLBACK_SERVERS = ["de1.api.radio-browser.info", "de2.api.radio-browser.info"];
  var SERVERS_KEY = "servers";
  var SERVERS_MAX_AGE_MS = 24 * 60 * 60 * 1e3;
  var HEADERS = { "User-Agent": "Raygun/1.0 (+https://github.com/uvxszi/raygun)" };
  var ORDERS = {
    popularity: "clickcount",
    votes: "votes",
    name: "name"
  };
  var servers = async () => {
    const cached = raygun.storage.get(SERVERS_KEY);
    if (cached && Date.now() - cached.fetchedAt < SERVERS_MAX_AGE_MS && cached.names.length > 0) {
      return cached.names;
    }
    try {
      const list = await fetchJson(`${DIRECTORY}/json/servers`, {
        headers: HEADERS
      });
      const names = [...new Set(list.map((server) => server.name))].filter(
        (name) => name.endsWith(".api.radio-browser.info")
      );
      if (names.length > 0) {
        raygun.storage.set(SERVERS_KEY, { fetchedAt: Date.now(), names });
        return names;
      }
    } catch (error) {
      raygun.log.warn("server list unavailable, using fallbacks:", String(error));
    }
    return FALLBACK_SERVERS;
  };
  var query = async (path) => {
    let lastError;
    for (const server of await servers()) {
      try {
        return await fetchJson(`https://${server}${path}`, { headers: HEADERS });
      } catch (error) {
        lastError = error;
        raygun.log.warn(`${server} failed:`, String(error));
      }
    }
    throw lastError instanceof Error ? lastError : new Error("no radio-browser server answered");
  };
  var describe = (station) => {
    const tags = station.tags.split(",").map((tag) => tag.trim()).filter(Boolean).slice(0, 3);
    return tags.length > 0 ? tags.join(", ") : null;
  };
  var toTrack = (station) => ({
    id: station.stationuuid,
    title: station.name.trim(),
    artist: describe(station),
    album: [station.country, station.bitrate ? `${station.bitrate} kbps` : null].filter(Boolean).join(" · "),
    durationMs: null,
    // The webview only loads https images in release builds.
    artworkUrl: station.favicon.startsWith("https://") ? station.favicon : null,
    source: { kind: "url", url: station.url_resolved, headers: {} }
  });
  var search = defineProvider({
    kind: "music.metadata",
    id: "stations",
    name: "Radio Browser",
    async search({ query: text, limit }) {
      const name = text.trim();
      if (name === "") return { tracks: [] };
      const order = ORDERS[raygun.settings.get("order") ?? "popularity"] ?? "clickcount";
      const country = (raygun.settings.get("country") ?? "").trim().toUpperCase();
      const params = [
        `name=${encodeURIComponent(name)}`,
        `limit=${Math.min(limit * 2, 100)}`,
        "hidebroken=true",
        `order=${order}`,
        `reverse=${order === "name" ? "false" : "true"}`,
        country ? `countrycode=${encodeURIComponent(country)}` : null
      ].filter(Boolean);
      const stations = await query(`/json/stations/search?${params.join("&")}`);
      const playable = stations.filter(
        // The native player has no HLS module yet; skip HLS stations.
        (station) => station.hls !== 1 && /^https?:\/\//.test(station.url_resolved) && station.name.trim()
      );
      const seen = /* @__PURE__ */ new Set();
      const unique = playable.filter((station) => {
        const key = station.url_resolved.replace(/^https?:\/\//, "").replace(/\/+$/, "");
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      });
      return { tracks: unique.slice(0, limit).map(toTrack) };
    }
  });
  registerProvider(search);
})();
