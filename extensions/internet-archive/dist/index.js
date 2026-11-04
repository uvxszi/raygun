// dev.raygun.internet-archive 1.5.0. Built by raygun-ext from src/index.ts; do not edit.
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

  // src/films.ts
  var API = "https://archive.org";
  var ROWS = 30;
  var COLLECTIONS = [
    { value: "feature_films", label: "Feature films" },
    { value: "silent_films", label: "Silent films" },
    { value: "Film_Noir", label: "Film noir" },
    { value: "SciFi_Horror", label: "Sci-fi and horror" },
    { value: "Comedy_Films", label: "Comedy films" },
    { value: "classic_cartoons", label: "Classic cartoons" },
    { value: "animationandcartoons", label: "Animation" },
    { value: "prelinger", label: "Prelinger Archives" },
    { value: "classic_tv", label: "Classic TV" }
  ];
  var PUBLIC_DOMAIN_BEFORE = (/* @__PURE__ */ new Date()).getUTCFullYear() - 95;
  var MARKS_BELIEVED_UNTIL = 1977;
  var MARK = /publicdomain\/mark|licenses\/publicdomain/i;
  var PUBLIC_DOMAIN_MARK = "(licenseurl:*publicdomain\\/mark* OR licenseurl:*licenses\\/publicdomain*)";
  var PUBLIC_DOMAIN = `(year:[1800 TO ${PUBLIC_DOMAIN_BEFORE - 1}] OR (${PUBLIC_DOMAIN_MARK} AND year:[1800 TO ${MARKS_BELIEVED_UNTIL}]))`;
  var PRELINGER = "licenseurl:*publicdomain*";
  var SORTS = [
    { value: "downloads desc", label: "Most viewed" },
    { value: "addeddate desc", label: "Recently added" },
    { value: "titleSorter asc", label: "Title" }
  ];
  var escapeQuery = (query) => query.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, " ").trim();
  var firstString = (value) => (Array.isArray(value) ? value[0] : value) ?? null;
  var encodePath = (path) => path.split("/").map(encodeURIComponent).join("/");
  var cover = (identifier) => `${API}/services/img/${encodeURIComponent(identifier)}`;
  var plainText = (html) => html.replace(/<br\s*\/?>/gi, "\n").replace(/<\/p>/gi, "\n\n").replace(/<[^>]*>/g, "").replace(/&nbsp;/g, " ").replace(/&quot;/g, '"').replace(/&#39;|&apos;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&").replace(/\n{3,}/g, "\n\n").trim();
  var pick = (filters2, key, options) => {
    const value = filters2?.[key];
    return typeof value === "string" && options.some((option) => option.value === value) ? value : options[0].value;
  };
  var searchPage = async (q, sort, page) => {
    const params = [
      `q=${encodeURIComponent(q)}`,
      "fl[]=identifier",
      "fl[]=title",
      `rows=${ROWS}`,
      `page=${Math.max(1, page)}`,
      `sort[]=${encodeURIComponent(sort)}`,
      "output=json"
    ];
    const result = await fetchJson(`${API}/advancedsearch.php?${params.join("&")}`);
    const docs = result.response?.docs ?? [];
    return {
      items: docs.map((doc) => ({
        id: doc.identifier,
        title: firstString(doc.title) ?? doc.identifier,
        coverUrl: cover(doc.identifier)
      })),
      hasNextPage: page * ROWS < (result.response?.numFound ?? 0)
    };
  };
  var inCollection = (collection) => `collection:(${collection}) AND mediatype:(movies) AND ${collection === "prelinger" ? PRELINGER : PUBLIC_DOMAIN}`;
  var items = /* @__PURE__ */ new Map();
  var fetchItem = (identifier) => {
    let item = items.get(identifier);
    if (!item) {
      item = fetchJson(`${API}/metadata/${encodeURIComponent(identifier)}`);
      item.catch(() => items.delete(identifier));
      items.set(identifier, item);
      if (items.size > 20) items.delete(items.keys().next().value);
    }
    return item;
  };
  var isOpen = (item) => {
    const licence = item.metadata?.licenseurl ?? "";
    const collections = [item.metadata?.collection ?? []].flat();
    if (collections.includes("prelinger") && /publicdomain/i.test(licence)) return true;
    const year = Number.parseInt((item.metadata?.year ?? item.metadata?.date ?? "").slice(0, 4), 10);
    if (!Number.isFinite(year)) return false;
    return year < PUBLIC_DOMAIN_BEFORE || MARK.test(licence) && year <= MARKS_BELIEVED_UNTIL;
  };
  var metadata = async (identifier) => {
    const item = await fetchItem(identifier);
    if (!item.metadata) throw new Error("The Internet Archive has no such item");
    if (!isOpen(item)) {
      throw new Error(
        "The Internet Archive doesn't list this item as public domain or openly licensed, so Raygun doesn't play it"
      );
    }
    return item;
  };
  var isPlayable = (file) => /\.(mp4|m4v)$/i.test(file.name) && /h\.264|mpeg4|mpeg-4/i.test(file.format ?? "");
  var episodeFiles = (item) => {
    const groups = /* @__PURE__ */ new Map();
    for (const file of item.files ?? []) {
      if (!isPlayable(file)) continue;
      const key = file.source === "derivative" && file.original ? file.original : file.name;
      groups.set(key, [...groups.get(key) ?? [], file]);
    }
    return new Map([...groups].sort(([a], [b]) => a.localeCompare(b, "en", { numeric: true })));
  };
  var baseName = (name) => name.replace(/\.[^./]+$/, "");
  var subtitlesFor = (identifier, item, original) => {
    const base = baseName(original);
    return (item.files ?? []).filter((file) => /\.(srt|vtt)$/i.test(file.name) && file.name.startsWith(base)).map((file) => ({
      url: `${API}/download/${encodeURIComponent(identifier)}/${encodePath(file.name)}`,
      // `.asr.srt` files are the archive's automatic transcripts.
      label: /\.asr\.(srt|vtt)$/i.test(file.name) ? "Automatic (English)" : "Subtitles",
      lang: "en"
    }));
  };
  var splitEpisode = (episodeId) => {
    const slash = episodeId.indexOf("/");
    return slash < 0 ? { identifier: episodeId, original: null } : { identifier: episodeId.slice(0, slash), original: episodeId.slice(slash + 1) };
  };
  var resolveServer = async (episodeId) => {
    const { identifier, original } = splitEpisode(episodeId);
    const item = await metadata(identifier);
    const groups = episodeFiles(item);
    const key = original ?? [...groups.keys()][0] ?? "";
    const subtitles = subtitlesFor(identifier, item, key);
    const videos = (groups.get(key) ?? []).map((file) => {
      const height = Number.parseInt(file.height ?? "", 10);
      const known = Number.isFinite(height) && height > 0;
      return {
        url: `${API}/download/${encodeURIComponent(identifier)}/${encodePath(file.name)}`,
        label: known ? `${height}p · ${file.format}` : file.format,
        quality: known ? height : void 0,
        subtitles
      };
    }).sort((a, b) => (b.quality ?? 0) - (a.quality ?? 0));
    return { id: episodeId, name: "Internet Archive", videos };
  };
  var filters = [
    { kind: "select", key: "collection", label: "Collection", options: COLLECTIONS },
    { kind: "select", key: "sort", label: "Sort", options: SORTS }
  ];
  var films = defineProvider({
    kind: "video.source",
    id: "films",
    name: "Internet Archive Films",
    popular: ({ page }) => searchPage(inCollection("feature_films"), "downloads desc", page),
    latest: ({ page }) => searchPage(inCollection("feature_films"), "addeddate desc", page),
    async search({ query, page, filters: values }) {
      const collection = pick(values, "collection", COLLECTIONS);
      const sort = pick(values, "sort", SORTS);
      const cleaned = escapeQuery(query);
      const q = cleaned ? `(title:(${cleaned}) OR subject:(${cleaned})) AND ${inCollection(collection)}` : inCollection(collection);
      return searchPage(q, sort, page);
    },
    filters: async () => filters,
    async details({ id }) {
      const item = await metadata(id);
      const meta = item.metadata ?? {};
      const description = firstString(meta.description);
      const year = Number.parseInt((meta.year ?? meta.date ?? "").slice(0, 4), 10);
      const subjects = (Array.isArray(meta.subject) ? meta.subject : [meta.subject ?? ""]).flatMap((subject) => subject.split(/\s*;\s*/)).map((subject) => subject.trim()).filter(Boolean);
      const creators = Array.isArray(meta.creator) ? meta.creator : meta.creator ? [meta.creator] : [];
      return {
        id,
        title: firstString(meta.title) ?? id,
        coverUrl: cover(id),
        description: description ? plainText(description) : null,
        genres: [...new Set(subjects)].slice(0, 8),
        directors: creators,
        type: "movie",
        year: Number.isFinite(year) ? year : null,
        status: "completed",
        url: `${API}/details/${encodeURIComponent(id)}`
      };
    },
    async episodes({ id }) {
      const groups = [...episodeFiles(await metadata(id))];
      if (groups.length === 0) throw new Error("This item has no video Raygun can play");
      if (groups.length === 1) return { episodes: [{ id: `${id}/${groups[0][0]}`, name: "Film" }] };
      return {
        episodes: groups.map(([original, files], index) => ({
          id: `${id}/${original}`,
          name: files.find((file) => file.title)?.title ?? baseName(original).replace(/_/g, " "),
          number: index + 1
        }))
      };
    },
    // One server, resolved at once: the archive serves every file itself.
    servers: async ({ episodeId }) => ({ servers: [await resolveServer(episodeId)] }),
    // Never needed (`servers` comes resolved), but part of the contract.
    videos: async ({ serverId }) => ({ videos: (await resolveServer(serverId)).videos ?? [] })
  });

  // src/index.ts
  var API2 = "https://archive.org";
  var OPEN_LICENCE = /creativecommons\.org|publicdomain/i;
  var LEFT_OUT = ["78rpm", "georgeblood"];
  var OPEN_AUDIO = [
    "mediatype:audio",
    "collection:(netlabels OR musopen)",
    "(licenseurl:*creativecommons.org* OR licenseurl:*publicdomain*)",
    `-collection:(${LEFT_OUT.join(" OR ")})`
  ].join(" AND ");
  var escapeQuery2 = (query) => query.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, " ").trim();
  var firstString2 = (value) => (Array.isArray(value) ? value[0] : value) ?? null;
  var parseLength = (length) => {
    if (!length) return null;
    const seconds = length.split(":").reduce((total, part) => total * 60 + Number.parseFloat(part), 0);
    return Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds * 1e3) : null;
  };
  var trackNumber = (file) => Number.parseInt(String(file.track ?? "").split("/")[0] ?? "", 10);
  var formatKbps = (format) => {
    if (!format) return null;
    if (/flac|lossless/i.test(format)) return 1411;
    const stated = /(\d+)\s*kbps/i.exec(format);
    if (stated) return Number(stated[1]);
    return format === "VBR MP3" ? 200 : null;
  };
  var isPlayable2 = (file) => (file.format?.includes("MP3") ?? false) || /flac/i.test(file.format ?? "");
  var pickFormat = (files, targetKbps) => {
    const formats = [...new Set(files.map((file) => file.format ?? ""))].filter(
      (format) => formatKbps(format) !== null
    );
    if (formats.length === 0) return null;
    if (targetKbps === void 0) {
      return formats.includes("VBR MP3") ? "VBR MP3" : formats.find((format) => format.includes("MP3")) ?? formats[0] ?? null;
    }
    const distance = (format) => {
      const kbps = formatKbps(format) ?? 160;
      if (targetKbps === null) return -kbps;
      return Math.max(kbps, targetKbps) / Math.max(1, Math.min(kbps, targetKbps));
    };
    return [...formats].sort((a, b) => distance(a) - distance(b))[0] ?? null;
  };
  var encodePath2 = (path) => path.split("/").map(encodeURIComponent).join("/");
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
    const result = await fetchJson(`${API2}/advancedsearch.php?${params.join("&")}`);
    return result.response?.docs ?? [];
  };
  var searchItems = (query, rows) => searchDocs(`(title:(${query}) OR creator:(${query})) AND ${OPEN_AUDIO}`, rows);
  var bareTitle = (title) => title.replace(/\([^)]*\)|\[[^\]]*\]/g, " ").trim() || title;
  var mainArtist = (artist) => artist.split(/\s*(?:&|,|;|\/|\bfeat\.?|\bft\.?|\band\b|\bwith\b|\bx\b)\s*/i)[0]?.trim() || artist;
  var words = (text) => escapeQuery2(text).toLowerCase().split(/\s+/).filter(Boolean);
  var itemTracks = async (doc, max, targetKbps) => {
    const id = doc.identifier;
    const item = await fetchJson(`${API2}/metadata/${encodeURIComponent(id)}`);
    const collections = [item.metadata?.collection ?? []].flat();
    if (!OPEN_LICENCE.test(item.metadata?.licenseurl ?? "")) return [];
    if (collections.some((name) => LEFT_OUT.includes(name))) return [];
    const files = (item.files ?? []).filter(isPlayable2);
    const format = pickFormat(files, targetKbps);
    const preferred = format === null ? files : files.filter((file) => file.format === format);
    preferred.sort((a, b) => (trackNumber(a) || 1e9) - (trackNumber(b) || 1e9));
    const album = firstString2(item.metadata?.title) ?? firstString2(doc.title);
    const creator = firstString2(item.metadata?.creator) ?? firstString2(doc.creator);
    return preferred.slice(0, max).map((file) => ({
      format: file.format ?? null,
      track: {
        id: `${id}/${file.name}`,
        title: file.title || file.name.replace(/\.(mp3|flac)$/i, "").replace(/_/g, " "),
        artist: firstString2(file.creator) ?? firstString2(file.artist) ?? creator,
        album: file.album || album,
        durationMs: parseLength(file.length),
        artworkUrl: `${API2}/services/img/${encodeURIComponent(id)}`,
        source: {
          kind: "url",
          url: `${API2}/download/${encodeURIComponent(id)}/${encodePath2(file.name)}`,
          headers: {}
        }
      }
    }));
  };
  var search = defineProvider({
    kind: "music.metadata",
    id: "search",
    name: "Internet Archive",
    async search({ query, limit }) {
      const cleaned = escapeQuery2(query);
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
      const tracks = settled.flatMap((result) => result.status === "fulfilled" ? result.value : []).map((item) => item.track);
      return { tracks: tracks.slice(0, limit) };
    }
  });
  var streams = defineProvider({
    kind: "music.stream",
    id: "stream",
    name: "Internet Archive",
    async candidates({ title, artist, limit, targetKbps }) {
      const wantedTitle = escapeQuery2(bareTitle(title));
      if (wantedTitle === "") return { candidates: [] };
      const wantedArtist = artist ? escapeQuery2(mainArtist(artist)) : "";
      const queries = wantedArtist ? [
        `title:(${wantedTitle}) AND creator:(${wantedArtist})`,
        `creator:(${wantedArtist}) AND (${wantedTitle})`,
        `title:(${wantedTitle})`
      ] : [`title:(${wantedTitle})`];
      const lists = await Promise.allSettled(
        queries.map((query) => searchDocs(`${query} AND ${OPEN_AUDIO}`, 4))
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
      const settled = await Promise.allSettled(docs.map((doc) => itemTracks(doc, 500, targetKbps)));
      for (const result of settled) {
        if (result.status === "rejected") raygun.log.warn("item skipped:", String(result.reason));
      }
      const matching = settled.flatMap((result) => result.status === "fulfilled" ? result.value : []).filter(({ track }) => {
        const offered = words(track.title);
        return wanted.every((word) => offered.includes(word));
      });
      raygun.log.info(
        `stream candidates for "${title}": ${docs.length} items, ${matching.length} matching files`
      );
      return {
        candidates: matching.slice(0, limit).map(({ track, format }) => ({
          id: track.id ?? track.title,
          title: track.title,
          artist: track.artist ?? null,
          durationMs: track.durationMs ?? null,
          quality: format ?? "MP3",
          bitrateKbps: formatKbps(format ?? void 0),
          source: track.source ?? null
        }))
      };
    }
  });
  registerProvider(search);
  registerProvider(streams);
  registerProvider(films);
})();
