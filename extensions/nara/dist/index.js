// dev.raygun.nara 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };
  var needsSetup = (reason) => {
    const host = raygun;
    if (typeof host.needsSetup === "function") host.needsSetup(reason);
    throw new Error(reason);
  };
  var query = (params) => Object.entries(params).flatMap(([name, value]) => {
    if (value === void 0 || value === null) return [];
    const values = Array.isArray(value) ? value : [value];
    return values.map((item) => `${encodeURIComponent(name)}=${encodeURIComponent(item)}`);
  }).join("&");
  var selectValue = (values, filter) => {
    const chosen = values?.[filter.key];
    const known = (value) => typeof value === "string" && filter.options.some((option) => option.value === value);
    if (known(chosen)) return chosen;
    if (known(filter.default)) return filter.default;
    return filter.options[0]?.value ?? "";
  };

  // src/records.ts
  var hitsOf = (answer) => {
    const hits = (typeof answer.body === "object" ? answer.body.hits : void 0) ?? answer.hits;
    const total = typeof hits?.total === "number" ? hits.total : hits?.total?.value ?? 0;
    const records2 = (hits?.hits ?? []).flatMap(
      (hit) => hit._source?.record ? [hit._source.record] : []
    );
    return { records: records2, total };
  };
  var isMp4 = (object) => /\bmp4\b/i.test(object.objectType ?? "") || /\.mp4$/i.test(object.objectFilename ?? "");
  var fileUrl = (address) => address.replace(
    /^https?:\/\/s3(?:\.dualstack\.[a-z0-9-]+)?\.amazonaws\.com\/NARAprodstorage\/lz\//i,
    "https://catalog.archives.gov/medialz/"
  );
  var reelNumber = (object) => {
    const named = /reel\s*(\d+)/i.exec(object.objectDesignator ?? "");
    if (named) return Number(named[1]);
    const file = /[-_]r(\d+)\.mp4$/i.exec(object.objectFilename ?? "");
    return file ? Number(file[1]) : null;
  };
  var reelsOf = (record) => {
    const seen = /* @__PURE__ */ new Set();
    const reels = (record.digitalObjects ?? []).filter((object) => {
      if (!isMp4(object) || !object.objectUrl || !/^https?:\/\//i.test(object.objectUrl)) {
        return false;
      }
      const name = (object.objectFilename ?? object.objectUrl.split("/").pop() ?? "").toLowerCase();
      if (seen.has(name)) return false;
      seen.add(name);
      return true;
    });
    return reels.map((object, index) => ({ object, index, reel: reelNumber(object) })).sort(
      (one, other) => (one.reel ?? Number.MAX_SAFE_INTEGER) - (other.reel ?? Number.MAX_SAFE_INTEGER) || one.index - other.index
    ).map(({ object }) => object);
  };
  var isOpen = (record) => record.useRestriction?.status === "Unrestricted" && record.accessRestriction?.status === "Unrestricted" && reelsOf(record).length > 0;
  var KEEP_CAPS = /* @__PURE__ */ new Set([
    "US",
    "USA",
    "USS",
    "USN",
    "USAF",
    "USMC",
    "USCG",
    "UN",
    "NATO",
    "NASA",
    "FBI",
    "CIA",
    "TV",
    "WW",
    "WWI",
    "WWII",
    "II",
    "III",
    "IV",
    "VI",
    "VII",
    "VIII",
    "IX",
    "XI",
    "XII",
    "DC",
    "NY",
    "SW",
    "SE",
    "NW",
    "NE",
    "ETO",
    "CBI",
    "POW",
    "POWS",
    "GI",
    "GIS",
    "ROTC",
    "NCO",
    "AEF",
    "MP"
  ]);
  var SMALL = /* @__PURE__ */ new Set(["a", "an", "and", "at", "by", "for", "in", "of", "on", "or", "the", "to"]);
  var tidyTitle = (title) => {
    const trimmed = title.replace(/\s+/g, " ").trim();
    if (/[a-z]/.test(trimmed)) return trimmed;
    let first = true;
    return trimmed.replace(/[A-Z][A-Z'’]*|\d+[A-Z]*/g, (word, at) => {
      const bare = word.replace(/['’].*$/, "");
      const designation = /^-\d/.test(trimmed.slice(at + word.length));
      let out;
      if (KEEP_CAPS.has(bare) || designation || /\d/.test(word)) out = word;
      else if (!first && SMALL.has(word.toLowerCase())) out = word.toLowerCase();
      else out = word.charAt(0) + word.slice(1).toLowerCase();
      first = false;
      return out;
    });
  };
  var idOf = (record) => record.naId === void 0 || record.naId === null || String(record.naId).trim() === "" ? null : String(record.naId).trim();
  var summaryOf = (record) => {
    const id = idOf(record);
    if (!id || !record.title || !isOpen(record)) return [];
    return [{ id, title: tidyTitle(record.title), coverUrl: null }];
  };
  var yearOf = (record) => {
    const dated = (record.productionDates ?? []).find((date) => date.year);
    const year = dated?.year ?? Number((dated?.logicalDate ?? "").slice(0, 4));
    return year && year >= 1800 && year <= 2200 ? year : null;
  };
  var heading = (text) => text.replace(/\s*\([^)]*\d{4}[^)]*\)\s*$/, "").replace(/\.$/, "");
  var paragraphs = (text, most) => {
    const plain = (text ?? "").replace(/<[^>]+>/g, " ").replace(/[ \t]+/g, " ").trim();
    return plain.length > most ? `${plain.slice(0, most).replace(/\s+\S*$/, "")}…` : plain;
  };
  var descriptionOf = (record) => {
    const series = (record.ancestors ?? []).find(
      (ancestor) => ancestor.levelOfDescription === "series"
    );
    const group = (record.ancestors ?? []).find(
      (ancestor) => ancestor.levelOfDescription === "recordGroup"
    );
    const makers = [
      ...record.creators ?? [],
      ...(series?.creators ?? []).filter((creator) => creator.creatorType !== "Predecessor")
    ].map((creator) => creator.heading ? heading(creator.heading) : "").filter(Boolean);
    const id = idOf(record);
    return [
      paragraphs(record.scopeAndContentNote, 2e3),
      record.shotList ? `Shot list: ${paragraphs(record.shotList, 2e3)}` : "",
      makers.length > 0 ? `Made by: ${[...new Set(makers)].slice(0, 2).join("; ")}` : "",
      [
        series?.title ? `Series: ${series.title}` : "",
        group?.title ? `Record Group ${group.recordGroupNumber ?? ""}: ${group.title}`.replace("  ", " ") : ""
      ].filter(Boolean).join("\n"),
      `National Archives Identifier ${id}${record.localIdentifier ? ` (${record.localIdentifier})` : ""}. Unrestricted use and access.`
    ].filter(Boolean).join("\n\n");
  };
  var episodesOf = (record) => {
    const id = idOf(record);
    const reels = reelsOf(record);
    if (reels.length === 1) return [{ id: `${id}/0`, name: "Film" }];
    return reels.map((reel, index) => ({
      id: `${id}/${index}`,
      name: reel.objectDesignator?.trim() || `Reel ${index + 1}`,
      number: index + 1
    }));
  };

  // src/index.ts
  var API = "https://catalog.archives.gov/api/v2";
  var AGENT = "Raygun/1.0 ( https://github.com/uvxszi/raygun )";
  var PAGE_SIZE = 60;
  var DEEPEST = 1e4;
  var ARMY_LIBRARY_COPIES = 25061;
  var RECORD_GROUP = {
    kind: "select",
    key: "recordGroup",
    label: "Record group",
    options: [
      { value: "all", label: "All record groups" },
      { value: "111", label: "Army Signal Corps (111)" },
      { value: "18", label: "Army Air Forces (18)" },
      { value: "342", label: "Air Force (342)" },
      { value: "80", label: "Navy, to 1947 (80)" },
      { value: "428", label: "Navy, from 1947 (428)" },
      { value: "127", label: "Marine Corps (127)" },
      { value: "26", label: "Coast Guard (26)" },
      { value: "330", label: "Secretary of Defense (330)" },
      { value: "255", label: "NASA (255)" },
      { value: "306", label: "Information Agency (306)" },
      { value: "208", label: "Office of War Information (208)" },
      { value: "16", label: "Agriculture (16)" },
      { value: "79", label: "National Park Service (79)" },
      { value: "200", label: "Gift collections (200)" }
    ],
    default: "all"
  };
  var DECADE = {
    kind: "select",
    key: "decade",
    label: "Decade",
    options: [
      { value: "any", label: "Any time" },
      ...[1910, 1920, 1930, 1940, 1950, 1960, 1970, 1980, 1990, 2e3].map((start) => ({
        value: String(start),
        label: `${start}s`
      }))
    ],
    default: "any"
  };
  var apiKey = () => {
    const key = raygun.settings.get("apiKey")?.trim();
    if (!key) {
      return needsSetup(
        "The National Archives Catalog needs your own API key: ask for one by email at Catalog_API@nara.gov and enter it in this source's settings"
      );
    }
    return key;
  };
  var refused = () => needsSetup(
    "The National Archives Catalog didn't accept the API key: check it in this source's settings"
  );
  var remember = (map, key, value, most) => {
    map.delete(key);
    map.set(key, value);
    if (map.size > most) map.delete(map.keys().next().value);
  };
  var pages = /* @__PURE__ */ new Map();
  var records = /* @__PURE__ */ new Map();
  var keep = (record) => {
    const id = idOf(record);
    if (id) remember(records, id, record, 400);
  };
  var search = async (params) => {
    const key = apiKey();
    const address = `${API}/records/search?${query(params)}`;
    const response = await raygun.http.fetch(address, {
      headers: { Accept: "application/json", "x-api-key": key, "User-Agent": AGENT }
    });
    if (response.status === 401 || response.status === 403) refused();
    if (response.status === 429) {
      throw new Error(
        "This API key has used up the National Archives Catalog's monthly queries; they start again on the first of next month"
      );
    }
    if (!response.ok)
      throw new Error(`The National Archives Catalog answered HTTP ${response.status}`);
    if ((response.headers["content-type"] ?? "").includes("text/html")) refused();
    let answer;
    try {
      answer = await response.json();
    } catch {
      throw new Error("The National Archives Catalog's answer couldn't be read");
    }
    if (answer.statusCode === 401 || answer.statusCode === 403) refused();
    if (answer.statusCode !== void 0 && answer.statusCode !== 200) {
      const said = typeof answer.body === "string" ? answer.body : answer.message;
      throw new Error(
        `The National Archives Catalog couldn't answer this search${said ? `: ${said}` : ""}`
      );
    }
    const found = hitsOf(answer);
    for (const record of found.records) if (isOpen(record)) keep(record);
    return found;
  };
  var filmPage = async (params, page) => {
    const asked = Math.max(1, page);
    if ((asked - 1) * PAGE_SIZE >= DEEPEST) return { items: [], hasNextPage: false };
    const all = {
      ...params,
      typeOfMaterials: "Moving Images",
      availableOnline: true,
      limit: PAGE_SIZE,
      page: asked
    };
    const seen = query(all);
    const kept = pages.get(seen);
    if (kept) return kept;
    const { records: found, total } = await search(all);
    const listed = {
      items: found.flatMap(summaryOf),
      hasNextPage: found.length > 0 && asked * PAGE_SIZE < Math.min(total, DEEPEST)
    };
    remember(pages, seen, listed, 60);
    return listed;
  };
  var film = async (id) => {
    let record = records.get(id);
    if (!record) {
      const { records: found } = await search({ naId: id });
      record = found.find((candidate) => idOf(candidate) === id);
      if (!record) throw new Error("This film is no longer in the National Archives Catalog");
    }
    if (!isOpen(record)) {
      throw new Error(
        "The National Archives marks this film restricted, or has no MP4 of it, so it isn’t offered"
      );
    }
    keep(record);
    return record;
  };
  var videosOf = async (episodeId) => {
    const cut = episodeId.lastIndexOf("/");
    const id = cut < 0 ? episodeId : episodeId.slice(0, cut);
    const index = cut < 0 ? 0 : Number(episodeId.slice(cut + 1));
    const reel = reelsOf(await film(id))[index];
    if (!reel?.objectUrl) throw new Error("The National Archives Catalog no longer has this reel");
    return [{ url: fileUrl(reel.objectUrl), label: "MP4" }];
  };
  registerProvider(
    defineProvider({
      kind: "video.source",
      id: "films",
      name: "National Archives",
      popular: ({ page }) => filmPage({ q: "*", ancestorNaId: ARMY_LIBRARY_COPIES }, page),
      search({ query: words, page, filters }) {
        const group = selectValue(filters, RECORD_GROUP);
        const decade = selectValue(filters, DECADE);
        const params = { q: words.trim() || "*" };
        if (group !== "all") params.recordGroupNumber = group;
        if (decade !== "any") {
          params.startDate = decade;
          params.endDate = String(Number(decade) + 9);
        }
        return filmPage(params, page);
      },
      filters: async () => [RECORD_GROUP, DECADE],
      async details({ id }) {
        const record = await film(id);
        return {
          id,
          title: tidyTitle(record.title ?? id),
          coverUrl: null,
          description: descriptionOf(record),
          genres: [],
          type: "movie",
          year: yearOf(record),
          status: "completed",
          url: `https://catalog.archives.gov/id/${encodeURIComponent(id)}`
        };
      },
      episodes: async ({ id }) => ({ episodes: episodesOf(await film(id)) }),
      // One server, resolved at once: the catalogue serves its files itself.
      servers: async ({ episodeId }) => ({
        servers: [{ id: episodeId, name: "National Archives", videos: await videosOf(episodeId) }]
      }),
      videos: async ({ serverId }) => ({ videos: await videosOf(serverId) })
    })
  );
})();
