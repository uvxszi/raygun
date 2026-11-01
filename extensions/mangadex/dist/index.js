// dev.raygun.mangadex 1.2.0. Built by raygun-ext from src/index.ts; do not edit.
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
  var FEED_PAGE = 500;
  var MAX_FEED_PAGES = 10;
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
  var language = () => (raygun.settings.get("language") ?? "en").trim().toLowerCase() || "en";
  var ratings = () => {
    const chosen = RATINGS[raygun.settings.get("contentRating") ?? ""] ?? RATINGS["safe and suggestive"];
    return raygun.content.showNsfw() ? chosen : chosen.filter((rating) => !ADULT.includes(rating));
  };
  var isAdult = (record) => ADULT.includes(record.attributes.contentRating ?? "");
  var query = (params) => params.map(([key, value]) => `${encodeURIComponent(key)}=${encodeURIComponent(String(value))}`).join("&");
  var get = async (path, params) => {
    const response = await raygun.http.fetch(`${API}${path}?${query(params)}`, {
      headers: HEADERS
    });
    if (response.status === 429)
      throw new Error("MangaDex is rate limiting requests; try again shortly");
    if (!response.ok) throw new Error(`MangaDex answered HTTP ${response.status}`);
    return response.json();
  };
  var localized = (values, preferred) => {
    if (!values) return null;
    return values[preferred] ?? values.en ?? Object.values(values)[0] ?? null;
  };
  var titleOf = (record) => {
    const lang = language();
    const direct = localized(record.attributes.title, lang);
    const alt = record.attributes.altTitles?.find((title) => title[lang] ?? title.en);
    return (alt?.[lang] ?? direct ?? alt?.en ?? "Untitled").trim();
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
    const result = await get("/manga", [
      ["limit", PAGE_SIZE],
      ["offset", offset],
      ["includes[]", "cover_art"],
      ["availableTranslatedLanguage[]", language()],
      ["hasAvailableChapters", "true"],
      [`order[${order[0]}]`, order[1]],
      ...ratings().map((rating) => ["contentRating[]", rating]),
      ...extra
    ]);
    return {
      items: result.data.map(summary),
      hasNextPage: offset + result.data.length < result.total
    };
  };
  var details = async (id) => {
    const { data } = await get(`/manga/${id}`, [
      ["includes[]", "cover_art"],
      ["includes[]", "author"],
      ["includes[]", "artist"]
    ]);
    const names = (type) => data.relationships.filter((relation) => relation.type === type).map((relation) => relation.attributes?.name).filter((name) => Boolean(name));
    return {
      id: data.id,
      title: titleOf(data),
      coverUrl: coverOf(data),
      coverHeaders: HEADERS,
      authors: names("author"),
      artists: names("artist"),
      description: localized(data.attributes.description, language())?.trim() || null,
      genres: [
        ...(data.attributes.tags ?? []).filter((tag) => tag.attributes.group === "genre" || tag.attributes.group === "theme").map((tag) => localized(tag.attributes.name, "en")).filter((name) => Boolean(name)),
        // The app hides titles with an adult genre while NSFW content is off.
        ...isAdult(data) ? [data.attributes.contentRating === "erotica" ? "Erotica" : "Pornographic"] : []
      ],
      status: STATUSES[data.attributes.status ?? ""] ?? "unknown",
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
        ...RATINGS.everything.map((rating) => ["contentRating[]", rating])
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
  var tagFilter = async () => {
    if (tagCache) return tagCache;
    const result = await get("/manga/tag", []);
    const groupOrder = ["genre", "theme", "format", "content"];
    const options = result.data.map((tag) => ({
      value: tag.id,
      label: localized(tag.attributes.name, "en") ?? tag.id,
      group: tag.attributes.group
    })).filter((tag) => tag.group !== "content" || raygun.content.showNsfw()).sort(
      (a, b) => groupOrder.indexOf(a.group) - groupOrder.indexOf(b.group) || a.label.localeCompare(b.label)
    ).map(({ value, label }) => ({ value, label }));
    tagCache = { kind: "tags", key: "tags", label: "Tags", options };
    return tagCache;
  };
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
    const tags = chosen.tags;
    if (tags && typeof tags === "object") {
      for (const id of tags.include) extra.push(["includedTags[]", id]);
      for (const id of tags.exclude) extra.push(["excludedTags[]", id]);
    }
    const sort = typeof chosen.sort === "string" ? chosen.sort : "";
    return listMangas(SORTS[sort] ?? SORTS[text.trim() ? "relevance" : "popular"], page, extra);
  };
  registerProvider(
    defineProvider({
      kind: "manga.source",
      id: "mangadex",
      name: "MangaDex",
      popular: ({ page }) => listMangas(["followedCount", "desc"], page),
      latest: ({ page }) => listMangas(["latestUploadedChapter", "desc"], page),
      search: ({ query: text, page, filters: chosen }) => search(text, page, chosen),
      filters,
      details: ({ id }) => details(id),
      chapters: async ({ id }) => ({ chapters: await chapters(id) }),
      pages: async ({ chapterId }) => ({ pages: await pages(chapterId) })
    })
  );
})();
