// dev.raygun.archive-comics 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
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
  var ROWS = 30;
  var PUBLIC_DOMAIN_BEFORE = (/* @__PURE__ */ new Date()).getUTCFullYear() - 95;
  var SCAN_FORMAT = "Single Page Processed JP2 ZIP";
  var BASE_QUERY = [
    "mediatype:texts",
    "collection:comics",
    `date:[1000-01-01 TO ${PUBLIC_DOMAIN_BEFORE - 1}-12-31]`,
    `format:"${SCAN_FORMAT}"`,
    "(licenseurl:*publicdomain* OR licenseurl:*zero* OR (collection:comics AND -collection:comics_inbox))"
  ].join(" AND ");
  var PAGE_WIDTH = 1200;
  var LARGE_PAGE_WIDTH = 2400;
  var SORTS = [
    { value: "downloads desc", label: "Most read" },
    { value: "addeddate desc", label: "Recently added" },
    { value: "date asc", label: "Oldest first" },
    { value: "date desc", label: "Newest first" },
    { value: "titleSorter asc", label: "Title" }
  ];
  var LANGUAGES = [
    { value: "eng", label: "English" },
    { value: "fre", label: "French" },
    { value: "ita", label: "Italian" },
    { value: "ger", label: "German" },
    { value: "spa", label: "Spanish" },
    { value: "dut", label: "Dutch" },
    { value: "por", label: "Portuguese" }
  ];
  var LANGUAGE_NAMES = {
    eng: "English",
    fre: "French",
    ita: "Italian",
    ger: "German",
    spa: "Spanish",
    dut: "Dutch",
    por: "Portuguese"
  };
  var escapeQuery = (query) => query.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, " ").trim();
  var strings = (value) => (Array.isArray(value) ? value : value ? [value] : []).map((text) => text.trim()).filter(Boolean);
  var cover = (identifier) => `${API}/services/img/${encodeURIComponent(identifier)}`;
  var plainText = (html) => html.replace(/<br\s*\/?>/gi, "\n").replace(/<\/p>/gi, "\n\n").replace(/<[^>]*>/g, "").replace(/&nbsp;/g, " ").replace(/&quot;/g, '"').replace(/&#39;|&apos;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&").replace(/\n{3,}/g, "\n\n").trim();
  var searchPage = async (query, sort, page) => {
    const at = Math.max(page, 1);
    const params = [
      `q=${encodeURIComponent(query)}`,
      "fl[]=identifier",
      "fl[]=title",
      `rows=${ROWS}`,
      `page=${at}`,
      ...sort ? [`sort[]=${encodeURIComponent(sort)}`] : [],
      "output=json"
    ];
    const result = await fetchJson(`${API}/advancedsearch.php?${params.join("&")}`);
    const docs = result.response?.docs ?? [];
    return {
      items: docs.filter((doc) => doc.identifier).map((doc) => ({
        id: doc.identifier,
        title: strings(doc.title)[0] ?? doc.identifier,
        coverUrl: cover(doc.identifier)
      })),
      hasNextPage: at * ROWS < (result.response?.numFound ?? 0)
    };
  };
  var filters = async () => [
    { kind: "select", key: "sort", label: "Sort by", default: SORTS[0].value, options: SORTS },
    {
      kind: "select",
      key: "language",
      label: "Language",
      default: "any",
      options: [{ value: "any", label: "Any" }, ...LANGUAGES]
    }
  ];
  var pick = (chosen, key, options) => {
    const value = chosen?.[key];
    return typeof value === "string" && options.some((option) => option.value === value) ? value : null;
  };
  var search = (query, page, chosen) => {
    const words = escapeQuery(query);
    const parts = [`(${BASE_QUERY})`];
    if (words) parts.push(`(title:(${words}) OR creator:(${words}) OR subject:(${words}))`);
    const language = pick(chosen, "language", LANGUAGES);
    if (language) parts.push(`language:(${language} OR "${LANGUAGE_NAMES[language]}")`);
    const sort = pick(chosen, "sort", SORTS) ?? (words ? null : SORTS[0].value);
    return searchPage(parts.join(" AND "), sort, page);
  };
  var items = /* @__PURE__ */ new Map();
  var metadata = (identifier) => {
    let item = items.get(identifier);
    if (!item) {
      item = fetchJson(`${API}/metadata/${encodeURIComponent(identifier)}`);
      item.catch(() => items.delete(identifier));
      items.set(identifier, item);
      if (items.size > 20) items.delete(items.keys().next().value);
    }
    return item;
  };
  var details = async (id) => {
    const item = await metadata(id);
    const meta = item.metadata;
    if (!meta) throw new Error("The Internet Archive has no such item");
    const date = meta.date?.slice(0, 10);
    const description = strings(meta.description).map(plainText).join("\n\n");
    const languages = strings(meta.language).map((code) => LANGUAGE_NAMES[code] ?? code);
    return {
      id,
      title: strings(meta.title)[0] ?? id,
      coverUrl: cover(id),
      authors: strings(meta.creator),
      artists: [],
      description: [date ? `Published ${date}.` : "", description].filter(Boolean).join("\n\n") || null,
      genres: [.../* @__PURE__ */ new Set([...strings(meta.subject), ...languages])].slice(0, 12),
      status: "completed",
      url: `${API}/details/${encodeURIComponent(id)}`
    };
  };
  var books = (item) => (item.files ?? []).filter((file) => file.format === SCAN_FORMAT && file.name.endsWith("_jp2.zip")).map((file) => ({ prefix: file.name.slice(0, -"_jp2.zip".length), title: file.title })).sort((a, b) => a.prefix.localeCompare(b.prefix, "en", { numeric: true }));
  var chapterId = (identifier, prefix) => `${identifier}|${prefix}`;
  var chapters = async (id) => {
    const item = await metadata(id);
    const title = strings(item.metadata?.title)[0] ?? id;
    const published = item.metadata?.date ? Date.parse(item.metadata.date.slice(0, 10)) : NaN;
    const found = books(item);
    return found.map((book, index) => ({
      id: chapterId(id, book.prefix),
      name: found.length === 1 ? title : book.title?.trim() || book.prefix.split("/").pop().replace(/[_]+/g, " ").trim(),
      number: index + 1,
      dateUpload: Number.isFinite(published) ? published : null
    })).reverse();
  };
  var scaleFor = (width, target) => {
    if (!width || width <= target) return 1;
    return 2 ** Math.min(5, Math.floor(Math.log2(width / target)));
  };
  var pages = async (id) => {
    const cut = id.indexOf("|");
    const identifier = cut > 0 ? id.slice(0, cut) : id;
    const item = await metadata(identifier);
    const prefix = cut > 0 ? id.slice(cut + 1) : books(item)[0]?.prefix;
    if (!item.server || !item.dir || !prefix) {
      throw new Error("The Internet Archive has no pages for this item");
    }
    const params = [
      `id=${encodeURIComponent(identifier)}`,
      `itemPath=${encodeURIComponent(item.dir)}`,
      `server=${encodeURIComponent(item.server)}`,
      "format=jsonp",
      `subPrefix=${encodeURIComponent(prefix)}`
    ];
    const reader = await fetchJson(
      `https://${item.server}/BookReader/BookReaderJSIA.php?${params.join("&")}`
    );
    if (reader.data?.data?.isRestricted) {
      throw new Error("The Internet Archive lends this item only to signed-in readers");
    }
    const target = raygun.settings.get("largePages") === true ? LARGE_PAGE_WIDTH : PAGE_WIDTH;
    const urls = (reader.data?.brOptions?.data ?? []).flat().filter((page) => page.uri?.startsWith("https://")).map((page) => `${page.uri}&scale=${scaleFor(page.width, target)}&rotate=0`);
    if (urls.length === 0) throw new Error("The Internet Archive has no pages for this book");
    return urls;
  };
  registerProvider(
    defineProvider({
      kind: "manga.source",
      id: "comics",
      name: "Internet Archive",
      popular: ({ page }) => searchPage(BASE_QUERY, "downloads desc", page),
      latest: ({ page }) => searchPage(BASE_QUERY, "addeddate desc", page),
      search: ({ query, page, filters: chosen }) => search(query, page, chosen),
      filters,
      details: ({ id }) => details(id),
      chapters: async ({ id }) => ({ chapters: await chapters(id) }),
      pages: async ({ chapterId: id }) => ({ pages: await pages(id) })
    })
  );
})();
