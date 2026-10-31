// dev.raygun.internet-archive 1.1.0. Built by raygun-ext from src/index.ts; do not edit.
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
  var searchDocs = async (q, rows) => {
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
  var searchItems = (query, rows) => searchDocs(`(title:(${query}) OR creator:(${query})) AND mediatype:audio`, rows);
  var bareTitle = (title) => title.replace(/\([^)]*\)|\[[^\]]*\]/g, " ").trim() || title;
  var mainArtist = (artist) => artist.split(/\s*(?:&|,|;|\/|\bfeat\.?|\bft\.?|\band\b|\bwith\b|\bx\b)\s*/i)[0]?.trim() || artist;
  var words = (text) => escapeQuery(text).toLowerCase().split(/\s+/).filter(Boolean);
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
  var streams = defineProvider({
    kind: "music.stream",
    id: "stream",
    name: "Internet Archive",
    async candidates({ title, artist, limit }) {
      const wantedTitle = escapeQuery(bareTitle(title));
      if (wantedTitle === "") return { candidates: [] };
      const wantedArtist = artist ? escapeQuery(mainArtist(artist)) : "";
      const queries = wantedArtist ? [
        `title:(${wantedTitle}) AND creator:(${wantedArtist})`,
        `creator:(${wantedArtist}) AND (${wantedTitle})`,
        `title:(${wantedTitle})`
      ] : [`title:(${wantedTitle})`];
      const lists = await Promise.allSettled(
        queries.map((query) => searchDocs(`${query} AND mediatype:audio`, 4))
      );
      const failures = lists.flatMap((list) => list.status === "rejected" ? [list.reason] : []);
      if (failures.length === lists.length) throw failures[0];
      for (const failure of failures) raygun.log.warn("stream search failed:", String(failure));
      const docs = [
        ...new Map(
          lists.flatMap((list) => list.status === "fulfilled" ? list.value : []).map((doc) => [doc.identifier, doc])
        ).values()
      ].slice(0, 6);
      const wanted = words(bareTitle(title));
      const settled = await Promise.allSettled(docs.map((doc) => itemTracks(doc, 500)));
      for (const result of settled) {
        if (result.status === "rejected") raygun.log.warn("item skipped:", String(result.reason));
      }
      const matching = settled.flatMap((result) => result.status === "fulfilled" ? result.value : []).filter((track) => {
        const offered = words(track.title);
        return wanted.every((word) => offered.includes(word));
      });
      raygun.log.info(
        `stream candidates for "${title}": ${docs.length} items, ${matching.length} matching files`
      );
      return {
        candidates: matching.slice(0, limit).map((track) => ({
          id: track.id ?? track.title,
          title: track.title,
          artist: track.artist ?? null,
          durationMs: track.durationMs ?? null,
          quality: "MP3",
          source: track.source ?? null
        }))
      };
    }
  });
  registerProvider(search);
  registerProvider(streams);
})();
