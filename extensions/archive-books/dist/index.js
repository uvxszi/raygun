// dev.raygun.archive-books 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
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
  var LIBRARIES = [
    { value: "americana", label: "American libraries" },
    { value: "toronto", label: "Canadian libraries" },
    { value: "europeanlibraries", label: "European libraries" },
    { value: "iacl", label: "Children's library" },
    { value: "biodiversity", label: "Biodiversity Heritage Library" },
    { value: "smithsonian", label: "Smithsonian Libraries" },
    { value: "library_of_congress", label: "Library of Congress" },
    { value: "cdl", label: "University of California" },
    { value: "universallibrary", label: "Universal Library" },
    { value: "wellcomelibrary", label: "Wellcome Library" }
  ];
  var BASE_QUERY = [
    "mediatype:texts",
    'possible-copyright-status:"NOT_IN_COPYRIGHT"',
    `year:[1 TO ${PUBLIC_DOMAIN_BEFORE - 1}]`,
    "NOT access-restricted-item:true",
    "format:PDF",
    `collection:(${LIBRARIES.map((library) => library.value).join(" OR ")})`
  ].join(" AND ");
  var PDF_FORMATS = [
    "Text PDF",
    "Additional Text PDF",
    "PDF",
    "Grayscale PDF",
    "Image Container PDF"
  ];
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
    { value: "ger", label: "German" },
    { value: "spa", label: "Spanish" },
    { value: "ita", label: "Italian" },
    { value: "lat", label: "Latin" },
    { value: "dut", label: "Dutch" },
    { value: "por", label: "Portuguese" },
    { value: "swe", label: "Swedish" },
    { value: "rus", label: "Russian" }
  ];
  var LANGUAGE_NAMES = {
    ...Object.fromEntries(LANGUAGES.map((language) => [language.value, language.label])),
    fra: "French",
    deu: "German",
    nld: "Dutch",
    grc: "Ancient Greek",
    gre: "Greek",
    heb: "Hebrew",
    hin: "Hindi",
    san: "Sanskrit",
    ara: "Arabic",
    chi: "Chinese",
    jpn: "Japanese",
    dan: "Danish",
    nor: "Norwegian",
    pol: "Polish",
    wel: "Welsh",
    gle: "Irish"
  };
  var escapeQuery = (query) => query.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, " ").trim();
  var strings = (value) => (Array.isArray(value) ? value : value ? [value] : []).map((text) => text.trim()).filter(Boolean);
  var person = (name) => {
    const parts = name.replace(/\s*\([^)]*\)/g, "").split(/\s*,\s*/).map((part) => part.trim()).filter(Boolean);
    while (parts.length > 1 && /\d/.test(parts[parts.length - 1])) parts.pop();
    const [last, first, ...rest] = parts;
    if (rest.length === 0 && last && first && !/\s/.test(last) && /^[\p{L}.' -]+$/u.test(first)) {
      return `${first} ${last}`;
    }
    return parts.join(", ");
  };
  var creators = (value) => [
    ...new Set(
      strings(value).filter((name) => !/^(https?:\/\/|www\.)/i.test(name)).map(person).filter(Boolean)
    )
  ];
  var cover = (identifier) => `${API}/services/img/${encodeURIComponent(identifier)}`;
  var encodePath = (path) => path.split("/").map(encodeURIComponent).join("/");
  var download = (identifier, file) => `${API}/download/${encodeURIComponent(identifier)}/${encodePath(file)}`;
  var plainText = (html) => html.replace(/<br\s*\/?>/gi, "\n").replace(/<\/p>/gi, "\n\n").replace(/<[^>]*>/g, "").replace(/&nbsp;/g, " ").replace(/&quot;/g, '"').replace(/&#39;|&apos;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&").replace(/[ \t]+\n/g, "\n").replace(/\n{3,}/g, "\n\n").trim();
  var languageName = (code) => LANGUAGE_NAMES[code.toLowerCase()] ?? code;
  var searchPage = async (query, sort, page) => {
    const at = Math.max(page, 1);
    const params = [
      `q=${encodeURIComponent(query)}`,
      "fl[]=identifier",
      "fl[]=title",
      // Who wrote each book comes in the same answer, for the cards.
      "fl[]=creator",
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
        coverUrl: cover(doc.identifier),
        authors: creators(doc.creator).slice(0, 6)
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
    },
    {
      kind: "select",
      key: "library",
      label: "Library",
      default: "any",
      options: [{ value: "any", label: "Any" }, ...LIBRARIES]
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
    const library = pick(chosen, "library", LIBRARIES);
    if (library) parts.push(`collection:(${library})`);
    const sort = pick(chosen, "sort", SORTS) ?? (words ? null : SORTS[0].value);
    return searchPage(parts.join(" AND "), sort, page);
  };
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
  var yearOf = (meta) => {
    const found = /\d{4}/.exec(meta?.year ?? meta?.date ?? "")?.[0];
    return found ? Number(found) : null;
  };
  var refusal = (item) => {
    const meta = item.metadata;
    if (!meta || item.is_dark) return "The Internet Archive has no such book";
    const status = (meta["possible-copyright-status"] ?? "").toLowerCase().replace(/[^a-z]/g, "");
    const year = yearOf(meta);
    const inLibrary = strings(meta.collection).some(
      (collection) => LIBRARIES.some((library) => library.value === collection)
    );
    if (status !== "notincopyright" || year === null || year >= PUBLIC_DOMAIN_BEFORE || !inLibrary) {
      return "The Internet Archive doesn't list this book as public domain, so Raygun doesn't open it";
    }
    if (meta["access-restricted-item"] === "true") {
      return "The Internet Archive lends this book only to signed-in readers";
    }
    return null;
  };
  var book = async (identifier) => {
    const item = await fetchItem(identifier);
    const refused = refusal(item);
    if (refused) throw new Error(refused);
    return item;
  };
  var details = async (id) => {
    const item = await book(id);
    const meta = item.metadata;
    const year = yearOf(meta);
    const description = strings(meta.description).map(plainText).join("\n\n");
    const volume = /^v(?:ol)?\.?\s*\d/i.test(meta.volume ?? "") ? meta.volume : null;
    const pages = Number.parseInt(meta.imagecount ?? "", 10);
    const facts = [
      year ? `Published ${year}` : "",
      strings(meta.publisher)[0] ?? "",
      volume ? `Volume ${volume.replace(/^v(?:ol)?\.?\s*/i, "")}` : "",
      pages > 0 ? `${pages} scanned pages` : ""
    ].filter(Boolean);
    const library = strings(meta.contributor)[0];
    const rights = `Public domain in the United States: published more than 95 years ago, and marked not in copyright by the Internet Archive.${library ? ` Scanned from a copy at ${library}.` : ""}`;
    return {
      id,
      title: strings(meta.title)[0] ?? id,
      coverUrl: cover(id),
      authors: creators(meta.creator),
      artists: [],
      description: [facts.join(" · "), description, rights].filter(Boolean).join("\n\n"),
      genres: [
        .../* @__PURE__ */ new Set([
          ...strings(meta.subject).flatMap((subject) => subject.split(/\s*;\s*/)),
          ...strings(meta.language).map(languageName)
        ])
      ].filter(Boolean).slice(0, 12),
      status: "completed",
      url: `${API}/details/${encodeURIComponent(id)}`
    };
  };
  var naturalOrder = (a, b) => {
    const parts = (text) => text.toLowerCase().split(/(\d+)/).map((part, index) => index % 2 === 1 ? Number(part) : part);
    const [left, right] = [parts(a), parts(b)];
    for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
      const [x, y] = [left[index], right[index]];
      if (x === void 0 || y === void 0) return x === void 0 ? -1 : 1;
      if (x === y) continue;
      if (typeof x === "number" && typeof y === "number") return x - y;
      return String(x) < String(y) ? -1 : 1;
    }
    return 0;
  };
  var pdfs = (item) => {
    const files = (item.files ?? []).filter((file) => /\.pdf$/i.test(file.name));
    for (const format of PDF_FORMATS) {
      const found = files.filter((file) => file.format === format);
      if (found.length > 0) return found.sort((a, b) => naturalOrder(a.name, b.name));
    }
    return [];
  };
  var chapterId = (identifier, file) => `${identifier}|${file}`;
  var chapters = async (id) => {
    const item = await book(id);
    const title = strings(item.metadata?.title)[0] ?? id;
    const found = pdfs(item);
    if (found.length === 0) throw new Error("The Internet Archive has no PDF of this book");
    return found.map((file, index) => ({
      id: chapterId(id, file.name),
      name: found.length === 1 ? title : file.title?.trim() || file.name.split("/").pop().replace(/\.pdf$/i, "").replace(/[_]+/g, " ").trim(),
      number: index + 1
    }));
  };
  var passage = async (id) => {
    const cut = id.indexOf("|");
    const identifier = cut > 0 ? id.slice(0, cut) : id;
    const item = await book(identifier);
    const file = cut > 0 ? id.slice(cut + 1) : pdfs(item)[0]?.name;
    if (!file || !(item.files ?? []).some((each) => each.name === file)) {
      throw new Error("This PDF is no longer in the book");
    }
    return { pdf: download(identifier, file) };
  };
  registerProvider(
    defineProvider({
      kind: "novel.source",
      id: "books",
      name: "Internet Archive",
      popular: ({ page }) => searchPage(BASE_QUERY, "downloads desc", page),
      latest: ({ page }) => searchPage(BASE_QUERY, "addeddate desc", page),
      search: ({ query, page, filters: chosen }) => search(query, page, chosen),
      filters,
      details: ({ id }) => details(id),
      chapters: async ({ id }) => ({ chapters: await chapters(id) }),
      passage: ({ chapterId: id }) => passage(id)
    })
  );
})();
