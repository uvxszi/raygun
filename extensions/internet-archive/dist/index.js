// dev.raygun.internet-archive 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
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
  var API = "https://archive.org";
  var escapeQuery = (query) => query.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, " ").trim();
  var firstString = (value) => (Array.isArray(value) ? value[0] : value) ?? null;
  var parseLength = (length) => {
    if (!length) return null;
    const seconds = length.split(":").reduce((total, part) => total * 60 + Number.parseFloat(part), 0);
    return Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds * 1e3) : null;
  };
  var trackNumber = (file) => Number.parseInt(String(file.track ?? "").split("/")[0] ?? "", 10);
  var isMp3 = (file) => file.format?.includes("MP3") ?? false;
  var encodePath = (path) => path.split("/").map(encodeURIComponent).join("/");
  var searchItems = async (query, rows) => {
    const q = `(title:(${query}) OR creator:(${query})) AND mediatype:audio`;
    const params = [
      `q=${encodeURIComponent(q)}`,
      "fl[]=identifier",
      "fl[]=title",
      "fl[]=creator",
      `rows=${rows}`,
      "sort[]=downloads+desc",
      "output=json"
    ];
    const result = await fetchJson(`${API}/advancedsearch.php?${params.join("&")}`);
    return result.response?.docs ?? [];
  };
  var itemTracks = async (doc, max) => {
    const id = doc.identifier;
    const item = await fetchJson(`${API}/metadata/${encodeURIComponent(id)}`);
    const files = (item.files ?? []).filter(isMp3);
    const preferred = files.some((file) => file.format === "VBR MP3") ? files.filter((file) => file.format === "VBR MP3") : files;
    preferred.sort((a, b) => (trackNumber(a) || 1e9) - (trackNumber(b) || 1e9));
    const album = firstString(item.metadata?.title) ?? firstString(doc.title);
    const creator = firstString(item.metadata?.creator) ?? firstString(doc.creator);
    return preferred.slice(0, max).map((file) => ({
      id: `${id}/${file.name}`,
      title: file.title || file.name.replace(/\.mp3$/i, "").replace(/_/g, " "),
      artist: firstString(file.creator) ?? firstString(file.artist) ?? creator,
      album: file.album || album,
      durationMs: parseLength(file.length),
      artworkUrl: `${API}/services/img/${encodeURIComponent(id)}`,
      source: {
        kind: "url",
        url: `${API}/download/${encodeURIComponent(id)}/${encodePath(file.name)}`,
        headers: {}
      }
    }));
  };
  var search = defineProvider({
    kind: "music.metadata",
    id: "search",
    name: "Internet Archive",
    async search({ query, limit }) {
      const cleaned = escapeQuery(query);
      if (cleaned === "") return { tracks: [] };
      const rows = Math.max(1, Math.min(raygun.settings.get("itemsPerSearch") ?? 6, 12));
      const docs = await searchItems(cleaned, rows);
      const perItem = Math.max(1, Math.ceil(limit / Math.max(docs.length, 1)));
      const settled = await Promise.allSettled(docs.map((doc) => itemTracks(doc, perItem)));
      for (const result of settled) {
        if (result.status === "rejected") {
          raygun.log.warn("item skipped:", String(result.reason));
        }
      }
      const tracks = settled.flatMap((result) => result.status === "fulfilled" ? result.value : []);
      return { tracks: tracks.slice(0, limit) };
    }
  });
  registerProvider(search);
})();
