// dev.raygun.mangadex 1.3.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var API = "https://api.mangadex.org";
  var HEADERS = { "User-Agent": "Raygun/1.0 (+https://github.com/uvxszi/raygun)" };
  var COVERS = "https://uploads.mangadex.org/covers";
  var PAGE_SIZE = 24;
  var MAX_RESULTS = 1e4;
  var FEED_PAGE = 500;
  var MAX_FEED_PAGES = 10;
  var LATEST_CHAPTERS = 100;
  var RATINGS = {
    safe: ["safe"],
    "safe and suggestive": ["safe", "suggestive"],
    everything: ["safe", "suggestive", "erotica", "pornographic"]
  };
  var ADULT = ["erotica", "pornographic"];
  var STATUSES = {
    ongoing: "ongoing",
    completed: "completed",
    hiatus: "hiatus",
    cancelled: "cancelled"
  };
  var DEMOGRAPHICS = {
    shounen: "Shounen",
    shoujo: "Shoujo",
    seinen: "Seinen",
    josei: "Josei"
  };
  var LANGUAGES = {
    English: "en",
    "Spanish (Latin America)": "es-la",
    "Spanish (Spain)": "es",
    "Portuguese (Brazil)": "pt-br",
    "Portuguese (Portugal)": "pt",
    French: "fr",
    German: "de",
    Italian: "it",
    Russian: "ru",
    Ukrainian: "uk",
    Polish: "pl",
    Czech: "cs",
    Hungarian: "hu",
    Romanian: "ro",
    Turkish: "tr",
    Arabic: "ar",
    Persian: "fa",
    Hebrew: "he",
    Indonesian: "id",
    Malay: "ms",
    Filipino: "tl",
    Vietnamese: "vi",
    Thai: "th",
    Japanese: "ja",
    Korean: "ko",
    "Chinese (Simplified)": "zh",
    "Chinese (Traditional)": "zh-hk"
  };
  var language = () => {
    const chosen = (raygun.settings.get("language") ?? "").trim();
    if (LANGUAGES[chosen]) return LANGUAGES[chosen];
    const code = chosen.toLowerCase();
    return /^[a-z]{2}(-[a-z]{2})?$/.test(code) ? code : "en";
  };
  var ratings = () => {
    const choice = (raygun.settings.get("contentRating") ?? "").trim().toLowerCase();
    const chosen = RATINGS[choice] ?? RATINGS["safe and suggestive"];
    return raygun.content.showNsfw() ? chosen : chosen.filter((rating) => !ADULT.includes(rating));
  };
  var ratingParams = (list) => list.map((rating) => ["contentRating[]", rating]);
  var isAdult = (record) => ADULT.includes(record.attributes.contentRating ?? "");
  var query = (params) => params.map(([key, value]) => `${encodeURIComponent(key)}=${encodeURIComponent(String(value))}`).join("&");
  var get = async (path, params) => {
    const response = await raygun.http.fetch(`${API}${path}?${query(params)}`, {
      headers: HEADERS
    });
    if (response.status === 429)
      throw new Error("MangaDex is rate limiting requests; try again shortly");
    if (response.status === 404) throw new Error("MangaDex no longer has this title or chapter");
    if (!response.ok) throw new Error(`MangaDex answered HTTP ${response.status}`);
    return response.json();
  };
  var localized = (values, preferred) => {
    if (!values) return null;
    return values[preferred] ?? values.en ?? Object.values(values)[0] ?? null;
  };
  var titleOf = (record) => {
    const main = record.attributes.title ?? {};
    const alternatives = record.attributes.altTitles ?? [];
    const inLanguage = (code) => main[code] ?? alternatives.find((title2) => title2[code])?.[code];
    const lang = language();
    const title = inLanguage(lang) ?? (lang === "en" ? void 0 : inLanguage("en")) ?? Object.values(main)[0];
    return title?.trim() || "Untitled";
  };
  var coverOf = (record) => {
    const file = record.relationships.find((relation) => relation.type === "cover_art")?.attributes?.fileName;
    return file ? `${COVERS}/${record.id}/${file}.512.jpg` : null;
  };
  var summary = (record) => ({
    id: record.id,
    title: titleOf(record),
    coverUrl: coverOf(record),
    coverHeaders: HEADERS,
    nsfw: isAdult(record)
  });
  var listMangas = async (order, page, extra = []) => {
    const offset = (Math.max(page, 1) - 1) * PAGE_SIZE;
    if (offset + PAGE_SIZE > MAX_RESULTS) return { items: [], hasNextPage: false };
    const result = await get("/manga", [
      ["limit", PAGE_SIZE],
      ["offset", offset],
      ["includes[]", "cover_art"],
      ["availableTranslatedLanguage[]", language()],
      ["hasAvailableChapters", "true"],
      [`order[${order[0]}]`, order[1]],
      ...ratingParams(ratings()),
      ...extra
    ]);
    const next = offset + result.data.length;
    return {
      items: result.data.map(summary),
      hasNextPage: next < result.total && next + PAGE_SIZE <= MAX_RESULTS
    };
  };
  var latestShown = /* @__PURE__ */ new Map();
  var latest = async (page) => {
    const at = Math.max(page, 1);
    const offset = (at - 1) * LATEST_CHAPTERS;
    if (offset + LATEST_CHAPTERS > MAX_RESULTS) return { items: [], hasNextPage: false };
    const lang = language();
    const allowed = ratings();
    const feed = await get("/chapter", [
      ["limit", LATEST_CHAPTERS],
      ["offset", offset],
      ["translatedLanguage[]", lang],
      ["order[readableAt]", "desc"],
      ["includeFutureUpdates", "0"],
      ["includeExternalUrl", "0"],
      ["includeEmptyPages", "0"],
      ...ratingParams(allowed)
    ]);
    const key = `${lang}|${allowed.join(",")}`;
    if (at === 1) {
      for (const shown of [...latestShown.keys()]) {
        if (shown.startsWith(`${key}|`)) latestShown.delete(shown);
      }
    }
    const earlier = /* @__PURE__ */ new Set();
    for (let before = 1; before < at; before += 1) {
      for (const id of latestShown.get(`${key}|${before}`) ?? []) earlier.add(id);
    }
    const ids = [
      ...new Set(
        feed.data.flatMap(
          (chapter) => chapter.relationships.filter((relation) => relation.type === "manga").map((relation) => relation.id)
        )
      )
    ].filter((id) => !earlier.has(id));
    latestShown.set(`${key}|${at}`, ids);
    const next = offset + feed.data.length;
    const hasNextPage = next < feed.total && next + LATEST_CHAPTERS <= MAX_RESULTS;
    if (ids.length === 0) return { items: [], hasNextPage };
    const series = await get("/manga", [
      ["limit", ids.length],
      ...ids.map((id) => ["ids[]", id]),
      ["includes[]", "cover_art"],
      ...ratingParams(allowed)
    ]);
    const byId = new Map(series.data.map((record) => [record.id, record]));
    return {
      items: ids.flatMap((id) => {
        const record = byId.get(id);
        return record ? [summary(record)] : [];
      }),
      hasNextPage
    };
  };
  var completion = async (record) => {
    const { lastChapter, lastVolume } = record.attributes;
    if (!lastChapter && !lastVolume) return "completed";
    try {
      const aggregate = await get(`/manga/${record.id}/aggregate`, [
        ["translatedLanguage[]", language()]
      ]);
      const volumes = Object.values(aggregate.volumes ?? {});
      const chapters2 = new Set(
        volumes.flatMap(
          (volume) => Object.values(volume.chapters ?? {}).map((chapter) => chapter.chapter)
        )
      );
      const reached = lastChapter ? chapters2.has(lastChapter) : volumes.some((volume) => volume.volume === lastVolume);
      return reached ? "completed" : "publishingFinished";
    } catch {
      return "completed";
    }
  };
  var details = async (id) => {
    const { data } = await get(`/manga/${id}`, [
      ["includes[]", "cover_art"],
      ["includes[]", "author"],
      ["includes[]", "artist"]
    ]);
    const names = (type) => [
      ...new Set(
        data.relationships.filter((relation) => relation.type === type).map((relation) => relation.attributes?.name?.trim()).filter((name) => Boolean(name))
      )
    ];
    const status = STATUSES[data.attributes.status ?? ""] ?? "unknown";
    const demographic = DEMOGRAPHICS[data.attributes.publicationDemographic ?? ""];
    return {
      id: data.id,
      title: titleOf(data),
      coverUrl: coverOf(data),
      coverHeaders: HEADERS,
      authors: names("author"),
      artists: names("artist"),
      description: localized(data.attributes.description, language())?.trim() || null,
      genres: [
        ...demographic ? [demographic] : [],
        ...(data.attributes.tags ?? []).filter((tag) => tag.attributes.group === "genre" || tag.attributes.group === "theme").map((tag) => localized(tag.attributes.name, "en")).filter((name) => Boolean(name)),
        // The app hides titles with an adult genre while NSFW content is off.
        ...isAdult(data) ? [data.attributes.contentRating === "erotica" ? "Erotica" : "Pornographic"] : []
      ],
      status: status === "completed" ? await completion(data) : status,
      url: `https://mangadex.org/title/${data.id}`
    };
  };
  var chapters = async (id) => {
    const found = [];
    for (let page = 0; page < MAX_FEED_PAGES; page += 1) {
      const result = await get(`/manga/${id}/feed`, [
        ["limit", FEED_PAGE],
        ["offset", page * FEED_PAGE],
        ["translatedLanguage[]", language()],
        ["order[volume]", "desc"],
        ["order[chapter]", "desc"],
        ["includes[]", "scanlation_group"],
        // Chapters a group holds back until later, hosted elsewhere or without pages can't be read.
        ["includeFuturePublishAt", "0"],
        ["includeExternalUrl", "0"],
        ["includeEmptyPages", "0"],
        ...ratingParams(RATINGS.everything)
      ]);
      for (const record of result.data) {
        if (record.attributes.externalUrl || record.attributes.pages === 0) continue;
        const number = record.attributes.chapter === null ? null : Number(record.attributes.chapter);
        const label = record.attributes.chapter ? `Chapter ${record.attributes.chapter}` : "Oneshot";
        const title = record.attributes.title?.trim();
        found.push({
          id: record.id,
          name: title ? `${label}: ${title}` : label,
          number: number !== null && Number.isFinite(number) ? number : null,
          volume: record.attributes.volume,
          scanlator: record.relationships.filter((relation) => relation.type === "scanlation_group").map((relation) => relation.attributes?.name).filter(Boolean).join(", ") || null,
          dateUpload: record.attributes.publishAt ? Date.parse(record.attributes.publishAt) : null
        });
      }
      if ((page + 1) * FEED_PAGE >= result.total) break;
    }
    return found;
  };
  var pages = async (chapterId) => {
    const server = await get(`/at-home/server/${chapterId}`, []);
    const saver = raygun.settings.get("dataSaver") === true;
    const files = saver ? server.chapter.dataSaver : server.chapter.data;
    const folder = saver ? "data-saver" : "data";
    return files.map((file) => ({
      url: `${server.baseUrl}/${folder}/${server.chapter.hash}/${file}`,
      headers: HEADERS
    }));
  };
  var SORTS = {
    relevance: ["relevance", "desc"],
    popular: ["followedCount", "desc"],
    latest: ["latestUploadedChapter", "desc"],
    rating: ["rating", "desc"],
    newest: ["createdAt", "desc"],
    title: ["title", "asc"]
  };
  var tagCache = null;
  var tags = () => {
    if (!tagCache) {
      const groupOrder = ["genre", "theme", "format", "content"];
      tagCache = get("/manga/tag", []).then(
        (result) => result.data.map((tag) => ({
          value: tag.id,
          label: localized(tag.attributes.name, "en") ?? tag.id,
          group: tag.attributes.group
        })).sort(
          (a, b) => groupOrder.indexOf(a.group) - groupOrder.indexOf(b.group) || a.label.localeCompare(b.label)
        )
      );
      tagCache.catch(() => {
        tagCache = null;
      });
    }
    return tagCache;
  };
  var tagFilter = async () => ({
    kind: "tags",
    key: "tags",
    label: "Tags",
    // Content tags (gore, sexual violence) are adult-leaning; leave them out unless NSFW is on.
    options: (await tags()).filter((tag) => tag.group !== "content" || raygun.content.showNsfw()).map(({ value, label }) => ({ value, label }))
  });
  var filters = async () => [
    {
      kind: "select",
      key: "sort",
      label: "Sort by",
      default: "relevance",
      options: [
        { value: "relevance", label: "Best match" },
        { value: "popular", label: "Most followed" },
        { value: "latest", label: "Latest upload" },
        { value: "rating", label: "Top rated" },
        { value: "newest", label: "Recently added" },
        { value: "title", label: "Title" }
      ]
    },
    {
      kind: "select",
      key: "status",
      label: "Status",
      default: "any",
      options: [
        { value: "any", label: "Any" },
        { value: "ongoing", label: "Ongoing" },
        { value: "completed", label: "Completed" },
        { value: "hiatus", label: "Hiatus" },
        { value: "cancelled", label: "Cancelled" }
      ]
    },
    {
      kind: "select",
      key: "demographic",
      label: "Demographic",
      default: "any",
      options: [
        { value: "any", label: "Any" },
        { value: "shounen", label: "Shounen" },
        { value: "shoujo", label: "Shoujo" },
        { value: "seinen", label: "Seinen" },
        { value: "josei", label: "Josei" }
      ]
    },
    {
      kind: "select",
      key: "origin",
      label: "Original language",
      default: "any",
      options: [
        { value: "any", label: "Any" },
        { value: "ja", label: "Japanese (manga)" },
        { value: "ko", label: "Korean (manhwa)" },
        { value: "zh", label: "Chinese (manhua)" },
        { value: "en", label: "English" }
      ]
    },
    await tagFilter()
  ];
  var search = (text, page, chosen = {}) => {
    const extra = [];
    if (text.trim()) extra.push(["title", text.trim()]);
    const status = chosen.status;
    if (typeof status === "string" && status !== "any") extra.push(["status[]", status]);
    const demographic = chosen.demographic;
    if (typeof demographic === "string" && demographic !== "any") {
      extra.push(["publicationDemographic[]", demographic]);
    }
    const origin = chosen.origin;
    if (typeof origin === "string" && origin !== "any") {
      extra.push(["originalLanguage[]", origin]);
      if (origin === "zh") extra.push(["originalLanguage[]", "zh-hk"]);
    }
    const chosenTags = chosen.tags;
    if (chosenTags && typeof chosenTags === "object") {
      for (const id of chosenTags.include) extra.push(["includedTags[]", id]);
      for (const id of chosenTags.exclude) extra.push(["excludedTags[]", id]);
    }
    const sort = typeof chosen.sort === "string" && (text.trim() || chosen.sort !== "relevance") ? chosen.sort : "";
    return listMangas(SORTS[sort] ?? SORTS[text.trim() ? "relevance" : "popular"], page, extra);
  };
  registerProvider(
    defineProvider({
      kind: "manga.source",
      id: "mangadex",
      name: "MangaDex",
      popular: ({ page }) => listMangas(["followedCount", "desc"], page),
      latest: ({ page }) => latest(page),
      search: ({ query: text, page, filters: chosen }) => search(text, page, chosen),
      filters,
      details: ({ id }) => details(id),
      chapters: async ({ id }) => ({ chapters: await chapters(id) }),
      pages: async ({ chapterId }) => ({ pages: await pages(chapterId) })
    })
  );
})();
