// dev.raygun.musicbrainz 1.2.0. Built by raygun-ext from src/index.ts; do not edit.
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
  var creditOf = (credits = []) => credits.map((part) => part.name + (part.joinphrase ?? "")).join("").trim();
  var toAlbum = (group) => ({
    id: group.id,
    title: group.title,
    artist: creditOf(group["artist-credit"]) || null,
    year: Number(group["first-release-date"]?.slice(0, 4)) || null,
    artworkUrl: `https://coverartarchive.org/release-group/${group.id}/front-250`
  });
  var search = defineProvider({
    kind: "music.metadata",
    id: "recordings",
    name: "MusicBrainz",
    async searchArtists({ query, limit }) {
      const cleaned = escapeQuery(query);
      if (cleaned === "") return { artists: [] };
      const found = await getJson(
        `${API}/artist?query=${encodeURIComponent(cleaned)}&limit=${Math.min(limit, 25)}&fmt=json`
      );
      return {
        artists: (found.artists ?? []).map((artist) => ({ id: artist.id, name: artist.name }))
      };
    },
    async searchAlbums({ query, limit }) {
      const cleaned = escapeQuery(query);
      if (cleaned === "") return { albums: [] };
      const found = await getJson(
        `${API}/release-group?query=${encodeURIComponent(cleaned)}&limit=${Math.min(limit, 25)}&fmt=json`
      );
      return { albums: (found["release-groups"] ?? []).map(toAlbum) };
    },
    async album({ id }) {
      if (!/^[0-9a-f-]{36}$/.test(id)) throw new Error(`Unknown album ${id}`);
      const group = await getJson(
        `${API}/release-group/${id}?inc=artist-credits&fmt=json`
      );
      const releases = await getJson(
        `${API}/release?release-group=${id}&inc=recordings+artist-credits&status=official&limit=1&fmt=json`
      );
      const release = releases.releases?.[0];
      const artist = creditOf(release?.["artist-credit"] ?? group["artist-credit"]) || null;
      const tracks = (release?.media ?? []).flatMap(
        (medium) => (medium.tracks ?? []).map((track) => ({
          id: track.recording?.id,
          title: track.title,
          artist,
          album: group.title,
          durationMs: track.length ?? null,
          artworkUrl: `https://coverartarchive.org/release-group/${id}/front-250`
        }))
      );
      return { album: toAlbum(group), tracks };
    },
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
  var simple = (text) => words(text).join(" ");
  var weighted = (tags = []) => {
    const most = Math.max(1, ...tags.map((tag) => tag.count ?? 1));
    return tags.filter((tag) => (tag.count ?? 1) > 0).map((tag) => ({ name: tag.name, weight: (tag.count ?? 1) / most }));
  };
  var getJson = async (url) => {
    const response = await fetchPaced(url);
    if (!response.ok) throw new Error(`MusicBrainz answered HTTP ${response.status}`);
    return response.json();
  };
  registerProvider(
    defineProvider({
      kind: "music.tags",
      id: "genres",
      name: "MusicBrainz",
      async tags({ title, artist }) {
        const parts = [`recording:"${escapeQuery(title)}"`];
        if (artist) parts.push(`artist:"${escapeQuery(artist)}"`);
        const found = await getJson(
          `${API}/recording?query=${encodeURIComponent(parts.join(" AND "))}&limit=10&fmt=json`
        );
        const wanted = simple(title);
        const matches = (found.recordings ?? []).filter(
          (recording) => simple(recording.title) === wanted && (!artist || words(credit(recording)).some((word) => words(artist).includes(word)))
        );
        const match = matches.find((recording) => recording.tags?.length) ?? matches[0];
        if (!match) return { tags: [] };
        if (match.tags?.length) return { tags: weighted(match.tags) };
        const artistId = match["artist-credit"]?.[0]?.artist?.id;
        if (!artistId) return { tags: [] };
        const byArtist = await getJson(
          `${API}/artist/${artistId}?inc=genres+tags&fmt=json`
        );
        return { tags: weighted(byArtist.genres?.length ? byArtist.genres : byArtist.tags) };
      }
    })
  );
})();
