// dev.raygun.live-music-archive 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
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
  var query = (params) => Object.entries(params).flatMap(([name, value]) => {
    if (value === void 0 || value === null) return [];
    const values = Array.isArray(value) ? value : [value];
    return values.map((item) => `${encodeURIComponent(name)}=${encodeURIComponent(item)}`);
  }).join("&");

  // src/index.ts
  var API = "https://archive.org";
  var SHOWS = "collection:etree AND mediatype:etree AND -collection:stream_only";
  var CONCERTS_PER_SEARCH = 6;
  var escape = (text) => text.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, " ").trim();
  var first = (value) => (Array.isArray(value) ? value[0] : value)?.trim() || null;
  var simple = (text) => (text ?? "").toLowerCase().replace(/['’]/g, "").replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  var wordsOf = (text) => simple(text).split(" ").filter(Boolean);
  var parseLength = (length) => {
    if (!length) return null;
    const seconds = length.split(":").reduce((total, part) => total * 60 + Number.parseFloat(part), 0);
    return Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds * 1e3) : null;
  };
  var yearOf = (doc) => {
    const year = Number.parseInt(String(doc.year ?? doc.date ?? "").slice(0, 4), 10);
    return Number.isFinite(year) && year > 1800 ? year : null;
  };
  var artworkOf = (identifier) => `${API}/services/img/${encodeURIComponent(identifier)}`;
  var encodePath = (path) => path.split("/").map(encodeURIComponent).join("/");
  var search = async (q, rows, sort = "downloads desc") => {
    const params = query({
      q: `${SHOWS} AND ${q}`,
      "fl[]": ["identifier", "title", "creator", "date", "year", "collection", "downloads"],
      "sort[]": sort,
      rows,
      output: "json"
    });
    const answer = await fetchJson(`${API}/advancedsearch.php?${params}`);
    return answer.response?.docs ?? [];
  };
  var MP3_FORMATS = ["VBR MP3", "64Kbps MP3", "MP3", "128Kbps MP3"];
  var trackNumber = (file) => Number.parseInt(String(file.track ?? "").split("/")[0] ?? "", 10);
  var songTitle = (file) => {
    const named = file.title?.trim() || file.name.replace(/\.mp3$/i, "").replace(/_/g, " ");
    return named.replace(/(?:\s*(?:-+>|=+>|>+|\*+|#+))+\s*$/, "").trim() || named;
  };
  var show = async (identifier) => {
    const item = await fetchJson(`${API}/metadata/${encodeURIComponent(identifier)}`);
    const collections = [item.metadata?.collection ?? []].flat();
    if (collections.includes("stream_only") || !collections.includes("etree")) return null;
    const files = item.files ?? [];
    const format = MP3_FORMATS.find((wanted) => files.some((file) => file.format === wanted));
    if (!format) return null;
    const title = first(item.metadata?.title) ?? identifier;
    const artist = first(item.metadata?.creator);
    const tracks = files.filter((file) => file.format === format).sort(
      (a, b) => (trackNumber(a) || 1e9) - (trackNumber(b) || 1e9) || a.name.localeCompare(b.name)
    ).map((file) => ({
      id: `${identifier}/${file.name}`,
      title: songTitle(file),
      artist,
      album: title,
      durationMs: parseLength(file.length),
      artworkUrl: artworkOf(identifier),
      source: {
        kind: "url",
        url: `${API}/download/${encodeURIComponent(identifier)}/${encodePath(file.name)}`,
        headers: {}
      }
    }));
    return {
      identifier,
      title,
      artist,
      year: yearOf(item.metadata ?? {}),
      tracks
    };
  };
  var toAlbum = (doc) => ({
    id: doc.identifier,
    title: first(doc.title) ?? doc.identifier,
    artist: first(doc.creator),
    year: yearOf(doc),
    artworkUrl: artworkOf(doc.identifier),
    kind: "live"
  });
  var everyWord = (text, fields) => wordsOf(text).slice(0, 8).map((word) => `(${fields.map((field) => `${field}:(${word})`).join(" OR ")})`).join(" AND ");
  var SHOW_FIELDS = ["title", "creator", "venue", "coverage"];
  var SONG_FIELDS = ["creator", "title", "description"];
  registerProvider(
    defineProvider({
      kind: "music.metadata",
      id: "search",
      name: "Live Music Archive",
      // Songs: concerts by the artist or whose set lists name the song, then the matching songs
      // in each (a few of each concert when only the artist was asked for).
      async search({ query: text, limit }) {
        const wanted = wordsOf(text);
        if (wanted.length === 0) return { tracks: [] };
        const docs = await search(everyWord(text, SONG_FIELDS), CONCERTS_PER_SEARCH);
        const settled = await Promise.allSettled(docs.map((doc) => show(doc.identifier)));
        const perShow = Math.max(2, Math.ceil(limit / Math.max(1, docs.length)));
        const tracks = [];
        for (const result of settled) {
          if (result.status === "rejected") {
            raygun.log.warn("show skipped:", String(result.reason));
            continue;
          }
          const found = result.value;
          if (!found) continue;
          const artistWords = new Set(wordsOf(found.artist));
          const rest = wanted.filter((word) => !artistWords.has(word));
          const matching = rest.length === 0 ? found.tracks.slice(0, perShow) : found.tracks.filter((track) => {
            const title = wordsOf(track.title);
            return rest.every((word) => title.includes(word));
          });
          tracks.push(...matching);
        }
        return { tracks: tracks.slice(0, Math.max(1, limit)) };
      },
      async searchAlbums({ query: text, limit }) {
        if (wordsOf(text).length === 0) return { albums: [] };
        const docs = await search(
          everyWord(text, SHOW_FIELDS),
          Math.max(1, Math.min(limit || 20, 50))
        );
        return { albums: docs.map(toAlbum) };
      },
      async album({ id }) {
        const found = await show(id);
        if (!found) throw new Error("This concert is no longer in the Live Music Archive");
        return {
          album: {
            id: found.identifier,
            title: found.title,
            artist: found.artist,
            year: found.year,
            artworkUrl: artworkOf(found.identifier),
            trackCount: found.tracks.length,
            kind: "live"
          },
          tracks: found.tracks
        };
      },
      // An artist's concerts, newest first, when the artist shares them here. Each artist has a
      // collection of their own; its shows are found through the artist's most played one, since
      // the name alone also finds tribute bands ("… Grateful Dead Revue").
      async artistReleases({ query: name, limit }) {
        const words = escape(name);
        const wanted = simple(name);
        if (!words || !wanted) return { releases: [] };
        const known = await search(`creator:("${words}")`, 10);
        const own = known.find((doc) => simple(first(doc.creator)) === wanted);
        const collection = [own?.collection ?? []].flat().find((id) => id !== "etree" && id !== "stream_only" && !id.startsWith("fav-"));
        if (!own || !collection) return { releases: [] };
        const count = Math.max(1, Math.min(limit || 30, 50));
        const docs = await search(
          `collection:"${collection.replace(/["\\]/g, "")}"`,
          Math.min(count * 3, 150),
          "date desc"
        );
        const nights = /* @__PURE__ */ new Map();
        for (const doc of docs) {
          if (simple(first(doc.creator)) !== wanted) continue;
          const night = (doc.date ?? doc.identifier).slice(0, 10);
          const kept = nights.get(night);
          if (!kept || (doc.downloads ?? 0) > (kept.downloads ?? 0)) nights.set(night, doc);
        }
        return { releases: [...nights.values()].slice(0, count).map(toAlbum) };
      }
    })
  );
})();
