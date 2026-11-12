// dev.raygun.ucsb-cylinders 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };
  var fetchText = async (url, init) => {
    const response = await raygun.http.fetch(url, init);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status} from ${url}`);
    }
    return response.text();
  };
  var query = (params) => Object.entries(params).flatMap(([name, value]) => {
    if (value === void 0 || value === null) return [];
    const values = Array.isArray(value) ? value : [value];
    return values.map((item) => `${encodeURIComponent(name)}=${encodeURIComponent(item)}`);
  }).join("&");

  // src/index.ts
  var SITE = "https://cylinders.library.ucsb.edu";
  var AT_ONCE = 5;
  var SEARCH_MOST = 10;
  var CHART_MOST = 15;
  var LIST_TTL_MS = 60 * 6e4;
  var KEPT_MOST = 800;
  var cutoff = () => (/* @__PURE__ */ new Date()).getUTCFullYear() - 101;
  var SLUR = /\b(?:nigg\w*|coons?|dark(?:e?y|ies)|pickaninn\w*|chinks?|japs?|kikes?|wops?)\b/i;
  var decode = (text) => text.replace(/<[^>]*>/g, " ").replace(/&#(\d+);/g, (_match, code) => String.fromCodePoint(Number(code))).replace(/&nbsp;/g, " ").replace(/&mdash;/g, "—").replace(/&quot;/g, '"').replace(/&#39;|&apos;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&").replace(/\s+/g, " ").trim();
  var latestYear = (text) => {
    const years = [...text.matchAll(/\b(1[89]\d\d)\b/g)].map((match) => Number(match[1]));
    return years.length > 0 ? Math.max(...years) : null;
  };
  var tidy = (text) => text.replace(/[[\]]/g, "").replace(/\s+([:;,.])/g, "$1").replace(/([:;,])\./g, "$1").replace(/\s*[/:;,.]\s*$/, "").replace(/\s+/g, " ").trim();
  var UNISSUED = /home recording|field recording|unissued|unpublished|private recording|test record/i;
  var capitalized = (text) => text.charAt(0).toUpperCase() + text.slice(1);
  var rowsOf = (html) => {
    const rows = /* @__PURE__ */ new Map();
    for (const block of html.split(/<div class="pure-g\b[^"]*">/).slice(1)) {
      const label = decode(/data-label">([^<]*)</.exec(block)?.[1] ?? "").replace(/:$/, "").toLowerCase();
      const entries = [...block.matchAll(/<div class="data-entry">([\s\S]*?)<\/div>/g)].map((entry) => decode(entry[1] ?? "")).filter(Boolean);
      if (label && !rows.has(label)) rows.set(label, entries);
    }
    return rows;
  };
  var resultsOf = (html) => {
    const seen = /* @__PURE__ */ new Set();
    return [...html.matchAll(/<div class="result-row">([\s\S]*?)<hr \/>/g)].flatMap((row) => {
      const mms = /detail\.php\?query_type=mms_id&(?:amp;)?query=(\d+)/.exec(row[1] ?? "")?.[1];
      if (!mms || seen.has(mms)) return [];
      seen.add(mms);
      const year = rowsOf(row[1] ?? "").get("release year")?.join(" ") ?? "";
      return [{ mms, year: latestYear(year) }];
    });
  };
  var cylinderOf = (html) => {
    const title = tidy(decode(/<h1[^>]*>([\s\S]*?)<\/h1>/.exec(html)?.[1] ?? ""));
    const audio = /<audio[^>]*\ssrc="(https:\/\/cylinders\.s3[^"]+\.mp3)"/.exec(html)?.[1];
    const number = /cusb-cyl(\d+)[a-z]?\.mp3/i.exec(audio ?? "")?.[1];
    const rows = rowsOf(html);
    const year = latestYear((rows.get("release year") ?? []).join(" "));
    if (!title || !number || year === null || year > cutoff()) return null;
    const issue = tidy((rows.get("issue number") ?? [])[0] ?? "").replace(/:\s*/, " ");
    const catalogued = [title, ...rows.get("notes") ?? [], ...rows.get("genre") ?? []].join("\n");
    if (UNISSUED.test(catalogued) || !issue && !/commercial/i.test(catalogued)) return null;
    const performers = tidy((rows.get("performers") ?? []).join("; "));
    return {
      number,
      url: audio,
      title: capitalized(title),
      performers: /not identified|^unknown/i.test(performers) || !performers ? null : performers,
      issue: issue || null,
      year
    };
  };
  var KEPT = "cylinders.v1";
  var kept = null;
  var keptCylinders = () => {
    if (!kept) {
      let saved = null;
      try {
        saved = raygun.storage.get(KEPT);
      } catch {
      }
      kept = new Map(Array.isArray(saved) ? saved : []);
    }
    return kept;
  };
  var keep = (mms, cylinder2) => {
    const cylinders = keptCylinders();
    cylinders.delete(mms);
    cylinders.set(mms, cylinder2);
    while (cylinders.size > KEPT_MOST) cylinders.delete(cylinders.keys().next().value);
    try {
      raygun.storage.set(KEPT, [...cylinders.entries()]);
    } catch {
    }
  };
  var cylinder = async (mms) => {
    const cylinders = keptCylinders();
    if (cylinders.has(mms)) return cylinders.get(mms) ?? null;
    const html = await fetchText(`${SITE}/detail.php?${query({ query_type: "mms_id", query: mms })}`);
    const found = cylinderOf(html);
    keep(mms, found);
    return found;
  };
  var trackOf = (mms, found) => ({
    id: mms,
    title: found.title,
    artist: found.performers,
    album: found.issue,
    source: {
      kind: "url",
      url: found.url,
      headers: {}
    }
  });
  var tracksOf = async (results, most) => {
    const wanted = results.filter((result) => result.year === null || result.year <= cutoff());
    const tracks = [];
    for (let at = 0; at < wanted.length && tracks.length < most; at += AT_ONCE) {
      const batch = wanted.slice(at, at + AT_ONCE);
      const read = await Promise.allSettled(batch.map((result) => cylinder(result.mms)));
      read.forEach((answer, index) => {
        if (answer.status === "fulfilled" && answer.value) {
          tracks.push(trackOf(batch[index].mms, answer.value));
        } else if (answer.status === "rejected") {
          raygun.log.warn("cylinder skipped:", String(answer.reason));
        }
      });
    }
    return tracks.slice(0, most);
  };
  var searchPage = async (type, words) => resultsOf(
    await fetchText(`${SITE}/search.php?${query({ nq: 1, query_type: type, query: words })}`)
  );
  var CHARTS = [
    {
      id: "ragtime",
      name: "Ragtime on cylinders",
      tag: null,
      group: "genre",
      subject: "Ragtime music"
    },
    { id: "vaudeville", name: "Vaudeville", tag: null, group: "genre", subject: "Vaudeville" },
    { id: "opera", name: "Opera on cylinders", tag: "Classical", group: "genre", subject: "Operas" },
    { id: "marches", name: "Marches", tag: null, group: "genre", subject: "Marches" },
    { id: "banjo", name: "Banjo", tag: "Folk", group: "genre", subject: "Banjo music" },
    { id: "hawaiian", name: "Hawaiian music", tag: null, group: "genre", subject: "Hawaiian" },
    { id: "yodels", name: "Yodels", tag: null, group: "genre", subject: "Yodels" },
    { id: "hymns", name: "Hymns", tag: null, group: "genre", subject: "Hymns" },
    { id: "christmas", name: "Christmas", tag: null, group: "genre", subject: "Christmas music" }
  ];
  var lists = /* @__PURE__ */ new Map();
  var chartTracks = (id) => {
    const chart = CHARTS.find((each) => each.id === id);
    if (!chart) return Promise.reject(new Error("The Cylinder Audio Archive has no such chart"));
    const listed = lists.get(id);
    if (listed && Date.now() - listed.at < LIST_TTL_MS) return listed.tracks;
    const current = {
      at: Date.now(),
      tracks: searchPage("subject", chart.subject).then((results) => tracksOf(results, CHART_MOST + 5)).then((tracks) => tracks.filter((track) => !SLUR.test(track.title)).slice(0, CHART_MOST))
    };
    current.tracks.catch(() => {
      if (lists.get(id) === current) lists.delete(id);
    });
    lists.set(id, current);
    return current.tracks;
  };
  registerProvider(
    defineProvider({
      kind: "music.metadata",
      id: "cylinders",
      name: "UCSB Cylinder Audio Archive",
      async search({ query: typed, limit }) {
        const words = typed.replace(/\s+/g, " ").trim();
        if (!words) return { tracks: [] };
        const results = await searchPage("keyword", words);
        return { tracks: await tracksOf(results, Math.max(1, Math.min(limit, SEARCH_MOST))) };
      }
    })
  );
  registerProvider(
    defineProvider({
      kind: "music.charts",
      id: "charts",
      name: "UCSB Cylinder Audio Archive",
      charts: async () => ({
        charts: CHARTS.map(({ id, name, tag, group }) => ({ id, name, tag, group }))
      }),
      async chart({ id, limit }) {
        return { tracks: (await chartTracks(id)).slice(0, Math.max(1, limit)) };
      },
      async tagSongs({ tag, aliases, kind, limit }) {
        if (kind !== "genre") return { tracks: [] };
        const names = [tag, ...aliases].map((name) => name.toLowerCase());
        const id = names.includes("ragtime") ? "ragtime" : names.includes("opera") ? "opera" : null;
        if (!id) return { tracks: [] };
        return { tracks: (await chartTracks(id)).slice(0, Math.max(1, limit)) };
      }
    })
  );
})();
