// dev.raygun.great-78 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
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
  var COLLECTION = "georgeblood";
  var LIST_MOST = 30;
  var LIST_TTL_MS = 30 * 6e4;
  var AT_ONCE = 10;
  var cutoff = () => (/* @__PURE__ */ new Date()).getUTCFullYear() - 102;
  var allowed = () => `collection:${COLLECTION} AND mediatype:audio AND year:[1880 TO ${cutoff()}]`;
  var UNISSUED = /\b(?:unissued|unreleased|unpublished|test pressing|home recording|field recording|private recording)\b/i;
  var SLUR = /\b(?:nigg\w*|coons?|dark(?:e?y|ies)|pickaninn\w*|chinks?|japs?|kikes?|wops?)\b/i;
  var escapeQuery = (text) => text.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, " ").trim();
  var first = (value) => ((Array.isArray(value) ? value[0] : value) ?? "").trim();
  var decode = (text) => text.replace(/<br\s*\/?>|<\/?p>/gi, "\n").replace(/<[^>]*>/g, "").replace(/&#(\d+);/g, (_match, code) => String.fromCodePoint(Number(code))).replace(/&nbsp;/g, " ").replace(/&quot;/g, '"').replace(/&#39;|&apos;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&").trim();
  var unshouted = (text) => /\p{Ll}/u.test(text) ? text : text.toLowerCase().replace(
    /(^|[\s(["/-])(\p{L})/gu,
    (_match, before, letter) => `${before}${letter.toUpperCase()}`
  );
  var peopleOf = (names) => names.split(/\s*;\s*/).filter(Boolean).slice(0, 3).map(unshouted).join("; ");
  var performerOf = (description) => {
    const line = /Performers?:\s*([^\n]+)/i.exec(decode(description))?.[1]?.trim();
    return line ? line.replace(/[.;,]\s*$/, "") : null;
  };
  var latestYear = (...texts) => {
    const years = texts.flatMap(
      (text) => [...text.matchAll(/\b(1[89]\d\d|20\d\d)\b/g)].map((match) => Number(match[1]))
    );
    return years.length > 0 ? Math.max(...years) : null;
  };
  var lengthMs = (length) => {
    if (!length) return null;
    const seconds = length.split(":").reduce((total, part) => total * 60 + Number(part), 0);
    return Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds * 1e3) : null;
  };
  var encodePath = (path) => path.split("/").map(encodeURIComponent).join("/");
  var unrestored = (name) => name.replace(/[-_ ]*\(?restored\)?(?=\.mp3$)/i, "");
  var mp3sOf = (files) => {
    const byRecording = /* @__PURE__ */ new Map();
    for (const file of files) {
      if (!/mp3/i.test(file.format ?? "") || !/\.mp3$/i.test(file.name)) continue;
      const key = unrestored(file.name);
      const kept = byRecording.get(key);
      if (!kept || /restored/i.test(file.name) && !/restored/i.test(kept.name)) {
        byRecording.set(key, file);
      }
    }
    return [...byRecording.entries()].sort(([one], [other]) => one.localeCompare(other, "en", { numeric: true })).map(([, file]) => file);
  };
  var records = /* @__PURE__ */ new Map();
  var recordOf = (identifier) => {
    let record = records.get(identifier);
    if (!record) {
      record = fetchJson(`${API}/metadata/${encodeURIComponent(identifier)}`);
      record.catch(() => records.delete(identifier));
      records.set(identifier, record);
      if (records.size > 300) records.delete(records.keys().next().value);
    }
    return record;
  };
  var fileTitle = (file) => decode(file.title ?? "").replace(/\s*\(restored\)\s*$/i, "").trim();
  var recordingsOf = async (identifier) => {
    const record = await recordOf(identifier);
    const metadata = record.metadata ?? {};
    const collections = [metadata.collection ?? []].flat();
    const year = latestYear(
      [metadata.year ?? []].flat().join(" "),
      [metadata.date ?? []].flat().join(" "),
      // Matrix and catalogue numbers aren't years.
      decode([metadata.description ?? []].flat().join("\n")).replace(
        /\b(?:matrix|catalog(?:ue)?|take)\s+numbers?:[^\n]*/gi,
        ""
      )
    );
    if (!collections.includes(COLLECTION) || year === null || year > cutoff()) return [];
    const notes = decode([metadata.title ?? [], metadata.description ?? []].flat().join("\n"));
    if (UNISSUED.test(notes)) return [];
    const title = unshouted(decode(first(metadata.title)));
    const files = mp3sOf(record.files ?? []).slice(0, 12);
    if (files.length === 0 || !title) return [];
    const label = decode(first(metadata.publisher));
    const number = first(metadata["publisher-catalog-number"]);
    const album = label ? `${unshouted(label)}${number ? ` ${number}` : ""}` : null;
    const performer = performerOf(first(metadata.description)) ?? decode(first(metadata.creator));
    return files.map((file, index) => ({
      id: files.length === 1 ? identifier : `${identifier}/${file.name}`,
      title: files.length === 1 ? title : unshouted(fileTitle(file)) || `${title} (${index + 1})`,
      artist: performer ? peopleOf(performer) : null,
      album: files.length === 1 ? album : title,
      durationMs: lengthMs(file.length),
      artworkUrl: `${API}/services/img/${encodeURIComponent(identifier)}`,
      source: {
        kind: "url",
        url: `${API}/download/${encodeURIComponent(identifier)}/${encodePath(file.name)}`,
        headers: {}
      }
    }));
  };
  var sides = async (identifiers) => {
    const found = [];
    for (let at = 0; at < identifiers.length; at += AT_ONCE) {
      const batch = await Promise.allSettled(identifiers.slice(at, at + AT_ONCE).map(recordingsOf));
      for (const result of batch) {
        if (result.status === "fulfilled") found.push(...result.value);
        else raygun.log.warn("record skipped:", String(result.reason));
      }
    }
    return found;
  };
  var searchSides = async (q, rows) => {
    const where = q ? `(${q}) AND ${allowed()}` : allowed();
    const params = [
      `q=${encodeURIComponent(where)}`,
      "fl[]=identifier",
      `rows=${rows}`,
      `sort[]=${encodeURIComponent("downloads desc")}`,
      "output=json"
    ];
    const answer = await fetchJson(`${API}/advancedsearch.php?${params.join("&")}`);
    return (answer.response?.docs ?? []).map((doc) => doc.identifier).filter(Boolean);
  };
  var genre = (id, name, tag, values) => ({
    chart: { id: `genre:${id}`, name, tag, group: "genre" },
    q: `genre:(${values.map((value) => `"${value}"`).join(" OR ")})`
  });
  var decade = (start) => {
    const end = Math.min(start + 9, cutoff());
    return {
      chart: {
        id: `decade:${start}`,
        name: end < start + 9 ? `${start}–${end}` : `The ${start}s`,
        group: "decade"
      },
      q: `year:[${start} TO ${end}]`
    };
  };
  var listed = () => [
    {
      chart: {
        id: "top",
        name: "Most played 78s",
        description: "The records listened to most on the Internet Archive",
        group: "top"
      },
      q: ""
    },
    decade(1890),
    decade(1900),
    decade(1910),
    decade(1920),
    genre("jazz", "Early jazz", "Jazz", ["Jazz"]),
    genre("blues", "Early blues", "Blues", ["Blues"]),
    genre("ragtime", "Ragtime", null, ["Ragtime", "Cakewalk", "Two-Step", "One-Step"]),
    genre("popular", "Popular songs", null, ["Popular Music"]),
    genre("opera", "Opera and operetta", "Classical", ["Opera", "Operetta"]),
    genre("classical", "Classical", "Classical", ["Classical", "Orchestral"]),
    genre("dance", "Dance bands", null, ["Fox Trot", "Fox-Trot", "Foxtrot", "Waltz", "Polka"]),
    genre("comedy", "Comedy and novelty", null, [
      "Comedy",
      "Comic Song",
      "Comic",
      "Novelty",
      "Monologue",
      "Sketch",
      "Laughing Record"
    ]),
    genre("folk", "Folk and old time", "Folk", [
      "Folk",
      "Hillbilly",
      "Old Time",
      "Irish",
      "Yodeling"
    ]),
    genre("sacred", "Sacred songs", null, ["Sacred", "Religious", "Hymn", "Gospel", "Spiritual"]),
    genre("hawaiian", "Hawaiian", null, ["Hawaiian"]),
    genre("latin", "Tango and Latin", "Latin", ["Tango", "Latin"]),
    genre("marches", "Marches and bands", null, ["March", "Military", "Band"]),
    genre("broadway", "Broadway and musicals", null, ["Broadway", "Musical"]),
    genre("christmas", "Christmas", null, ["Christmas"])
  ];
  var TAGS = {
    jazz: "genre:jazz",
    dixieland: "genre:jazz",
    blues: "genre:blues",
    ragtime: "genre:ragtime",
    classical: "genre:classical",
    opera: "genre:opera",
    folk: "genre:folk",
    country: "genre:folk",
    "old time": "genre:folk",
    gospel: "genre:sacred",
    christian: "genre:sacred",
    latin: "genre:latin",
    tango: "genre:latin",
    "show tunes": "genre:broadway",
    musicals: "genre:broadway",
    christmas: "genre:christmas"
  };
  var lists = /* @__PURE__ */ new Map();
  var listOf = ({ chart, q }) => {
    const kept = lists.get(chart.id);
    if (kept && Date.now() - kept.at < LIST_TTL_MS) return kept.tracks;
    const current = {
      at: Date.now(),
      tracks: searchSides(q, LIST_MOST + 10).then(sides).then((tracks) => tracks.filter((track) => !SLUR.test(track.title)).slice(0, LIST_MOST))
    };
    current.tracks.catch(() => {
      if (lists.get(chart.id) === current) lists.delete(chart.id);
    });
    lists.set(chart.id, current);
    return current.tracks;
  };
  var listById = (id) => {
    const found = listed().find((each) => each.chart.id === id);
    if (!found) throw new Error("The Great 78 Project has no such chart");
    return found;
  };
  registerProvider(
    defineProvider({
      kind: "music.metadata",
      id: "records",
      name: "Great 78 Project",
      async search({ query, limit }) {
        const words = escapeQuery(query).split(/\s+/).filter(Boolean).map((word) => word.length >= 3 ? `${word}*` : word).join(" ");
        if (!words) return { tracks: [] };
        const rows = Math.max(1, Math.min(limit, 12));
        const found = await searchSides(`title:(${words}) OR creator:(${words})`, rows);
        return { tracks: (await sides(found)).slice(0, limit) };
      }
    })
  );
  registerProvider(
    defineProvider({
      kind: "music.charts",
      id: "charts",
      name: "Great 78 Project",
      charts: async () => ({ charts: listed().map((each) => each.chart) }),
      async chart({ id, limit }) {
        const tracks = await listOf(listById(id));
        return { tracks: tracks.slice(0, Math.max(1, limit)) };
      },
      async tagSongs({ tag, aliases, kind, limit }) {
        if (kind !== "genre") return { tracks: [] };
        const id = [tag, ...aliases].map((name) => TAGS[name.toLowerCase()]).find((found2) => Boolean(found2));
        if (!id) return { tracks: [] };
        const found = listById(id);
        const tracks = await listOf(found);
        return { tracks: tracks.slice(0, Math.max(1, limit)), name: `${found.chart.name} on 78s` };
      }
    })
  );
})();
