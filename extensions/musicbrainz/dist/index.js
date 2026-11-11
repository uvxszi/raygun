// dev.raygun.musicbrainz 1.7.0. Built by raygun-ext from src/index.ts; do not edit.
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
    artworkUrl: `https://coverartarchive.org/release-group/${group.id}/front-250`,
    kind: kindOf(group)
  });
  var kindOf = (group) => {
    const secondary = group["secondary-types"] ?? [];
    if (secondary.includes("Compilation")) return "compilation";
    if (secondary.includes("Live")) return "live";
    switch (group["primary-type"]) {
      case "Album":
        return "album";
      case "Single":
        return "single";
      case "EP":
        return "ep";
      case void 0:
        return null;
      default:
        return "other";
    }
  };
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
    async artistReleases({ query, limit }) {
      const cleaned = escapeQuery(query);
      if (cleaned === "") return { releases: [] };
      const found = await getJson(
        `${API}/artist?query=${encodeURIComponent(`artist:"${cleaned}"`)}&limit=5&fmt=json`
      );
      const wanted = simple(query);
      const artist = (found.artists ?? []).find((candidate) => simple(candidate.name) === wanted) ?? (found.artists ?? [])[0];
      if (!artist) return { releases: [] };
      const groups = await getJson(
        `${API}/release-group?artist=${artist.id}&inc=artist-credits&limit=${Math.min(Math.max(limit, 1), 100)}&fmt=json`
      );
      const releases = (groups["release-groups"] ?? []).map(toAlbum).sort((a, b) => (b.year ?? 0) - (a.year ?? 0));
      return { releases };
    },
    async artistEvents({ query, limit }) {
      const cleaned = escapeQuery(query);
      if (cleaned === "") return { events: [] };
      const found = await getJson(
        `${API}/artist?query=${encodeURIComponent(`artist:"${cleaned}"`)}&limit=5&fmt=json`
      );
      const wanted = simple(query);
      const artist = (found.artists ?? []).find((candidate) => simple(candidate.name) === wanted) ?? (found.artists ?? [])[0];
      if (!artist) return { events: [] };
      const today = (/* @__PURE__ */ new Date()).toISOString().slice(0, 10);
      const search2 = `arid:${artist.id} AND begin:[${today} TO 2100-12-31]`;
      const listed = await getJson(
        `${API}/event?query=${encodeURIComponent(search2)}&limit=50&fmt=json`
      );
      const events = (listed.events ?? []).filter((event) => !event.cancelled && (event["life-span"]?.begin ?? "") >= today).sort((a, b) => (a["life-span"]?.begin ?? "").localeCompare(b["life-span"]?.begin ?? "")).slice(0, Math.max(limit, 1)).map((event) => {
        const place = event.relations?.find((relation) => relation.place)?.place;
        return {
          name: event.name,
          date: event["life-span"]?.begin ?? "",
          place: [place?.name, place?.area?.name].filter(Boolean).join(", ") || null,
          url: `https://musicbrainz.org/event/${event.id}`
        };
      });
      return { events };
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
  var WIKI_HEADERS = HEADERS;
  var unique = (names) => [...new Set(names.filter((name) => name.trim() !== ""))];
  var year = (date) => date?.slice(0, 4) ?? "";
  var wikipediaSummary = async (wikidataUrl) => {
    const id = wikidataUrl.split("/").pop();
    if (!id || !/^Q\d+$/.test(id)) return null;
    const entity = await raygun.http.fetch(
      `https://www.wikidata.org/wiki/Special:EntityData/${id}.json`,
      { headers: WIKI_HEADERS }
    );
    if (!entity.ok) return null;
    const data = await entity.json();
    const title = data.entities?.[id]?.sitelinks?.enwiki?.title;
    if (!title) return null;
    const summary = await raygun.http.fetch(
      `https://en.wikipedia.org/api/rest_v1/page/summary/${encodeURIComponent(title.replace(/ /g, "_"))}`,
      { headers: WIKI_HEADERS }
    );
    if (!summary.ok) return null;
    const page = await summary.json();
    return {
      bio: page.extract ?? null,
      imageUrl: page.thumbnail?.source ?? page.originalimage?.source ?? null,
      url: page.content_urls?.desktop?.page ?? null
    };
  };
  var capital = (text) => text.charAt(0).toUpperCase() + text.slice(1);
  var roleOf = (relation) => {
    const attribute = relation.attributes?.[0];
    switch (relation.type) {
      case "producer":
        return "Produced by";
      case "mix":
        return "Mixed by";
      case "mastering":
        return "Mastered by";
      case "engineer":
      case "audio":
      case "recording":
      case "sound":
        return "Engineered by";
      case "remixer":
        return "Remixed by";
      case "arranger":
      case "instrument arranger":
      case "vocal arranger":
        return "Arranged by";
      case "composer":
        return "Composed by";
      case "lyricist":
        return "Lyrics by";
      case "writer":
        return "Written by";
      case "vocal":
        return attribute ? capital(attribute) : "Vocals";
      case "instrument":
        return attribute ? capital(attribute) : "Instruments";
      default:
        return capital(relation.type);
    }
  };
  var FIRST_ROLES = ["Performed by", "Written by", "Composed by", "Lyrics by", "Produced by"];
  registerProvider(
    defineProvider({
      kind: "music.info",
      id: "info",
      name: "MusicBrainz",
      async artist({ name }) {
        const found = await getJson(
          `${API}/artist?query=${encodeURIComponent(`artist:"${escapeQuery(name)}"`)}&limit=5&fmt=json`
        );
        const wanted = simple(name);
        const candidates = found.artists ?? [];
        const match = candidates.find((candidate) => simple(candidate.name) === wanted) ?? candidates.find((candidate) => (candidate.score ?? 0) >= 95);
        if (!match) return { artist: null };
        const details = await getJson(
          `${API}/artist/${match.id}?inc=url-rels+artist-rels&fmt=json`
        );
        const relations = details.relations ?? [];
        const members = unique(
          relations.filter(
            (relation) => relation.type === "member of band" && relation.direction === "backward"
          ).map((relation) => relation.artist?.name ?? "")
        );
        const wikidata = relations.find((relation) => relation.type === "wikidata")?.url?.resource;
        let summary = null;
        if (wikidata) {
          try {
            summary = await wikipediaSummary(wikidata);
          } catch (error) {
            raygun.log.info(`Wikipedia had no summary: ${String(error)}`);
          }
        }
        const span = details["life-span"];
        const begin = year(span?.begin);
        const end = span?.ended ? year(span.end) : null;
        const active = begin ? end ? `${begin}–${end}` : begin : null;
        return {
          artist: {
            name: details.name,
            kind: details.type ?? null,
            bio: summary?.bio ?? null,
            imageUrl: summary?.imageUrl ?? null,
            url: summary?.url ?? `https://musicbrainz.org/artist/${details.id}`,
            origin: details["begin-area"]?.name ?? details.area?.name ?? details.country ?? null,
            active,
            members
          }
        };
      },
      async credits({ title, artist }) {
        const parts = [`recording:"${escapeQuery(title)}"`];
        if (artist) parts.push(`artist:"${escapeQuery(artist)}"`);
        const found = await getJson(
          `${API}/recording?query=${encodeURIComponent(parts.join(" AND "))}&limit=10&fmt=json`
        );
        const wanted = simple(title);
        const match = (found.recordings ?? []).find(
          (recording) => simple(recording.title) === wanted && (!artist || words(credit(recording)).some((word) => words(artist).includes(word)))
        );
        if (!match) return { credits: [] };
        const details = await getJson(
          `${API}/recording/${match.id}?inc=artist-credits+artist-rels+work-rels+work-level-rels&fmt=json`
        );
        const groups = /* @__PURE__ */ new Map();
        const add = (role, names) => groups.set(role, unique([...groups.get(role) ?? [], ...names]));
        add(
          "Performed by",
          (details["artist-credit"] ?? []).map((part) => part.name)
        );
        for (const relation of details.relations ?? []) {
          if (relation.artist) add(roleOf(relation), [relation.artist.name]);
          for (const workRelation of relation.work?.relations ?? []) {
            if (workRelation.artist) add(roleOf(workRelation), [workRelation.artist.name]);
          }
        }
        const ordered = [
          ...FIRST_ROLES.filter((role) => groups.has(role)),
          ...[...groups.keys()].filter((role) => !FIRST_ROLES.includes(role))
        ];
        return {
          credits: ordered.map((role) => ({ role, names: groups.get(role) ?? [] })),
          url: `https://musicbrainz.org/recording/${details.id}`
        };
      }
    })
  );
})();
