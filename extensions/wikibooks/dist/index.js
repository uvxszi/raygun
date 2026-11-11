// dev.raygun.wikibooks 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var WIKI = "https://en.wikibooks.org";
  var API = `${WIKI}/w/api.php`;
  var PAGEVIEWS = "https://wikimedia.org/api/rest_v1/metrics/pageviews/top/en.wikibooks/all-access";
  var PAGE_SIZE = 30;
  var HITS = 50;
  var AGENT = "Raygun/1.0 ( https://github.com/uvxszi/raygun )";
  var HEADERS = { "User-Agent": AGENT, "Api-User-Agent": AGENT };
  var BOOK_NAMESPACES = "0|110";
  var BOOK_CATEGORIES = [
    "Book:Wikibooks Stacks/Books",
    "Books by completion status/all books",
    "Featured books",
    "Completed books"
  ].map((name) => `Category:${name}`);
  var NOT_BOOKS = /* @__PURE__ */ new Set(["Wikibooks Stacks", "Main Page"]);
  var OTHER_PAGES = /^(?:Special|Talk|User|Wikibooks|File|MediaWiki|Template|Help|Category|Cookbook|Transwiki|Subject|Module|TimedText|Event|Media|Department|[^:/]+ talk):/i;
  var WHOLE_BOOK_PAGES = /\/(?:print(?:able)?[ _-]?version|pdf(?:[ _-]?version)?|all[ _]chapters|single[ _]page)$/i;
  var MAIN_TEXT = 3e3;
  var DROPPED_CLASSES = /* @__PURE__ */ new Set([
    "noprint",
    "navbox",
    "navigation-not-searchable",
    "hatnote",
    "mw-editsection",
    "catlinks",
    "printfooter",
    "messagebox",
    "ambox",
    "metadata",
    "mw-empty-elt",
    // Boards drawn with CSS, which the reader can't show.
    "chessboard-container"
  ]);
  var SUBJECTS = [
    { value: "all", label: "All" },
    { value: "Computing", label: "Computing" },
    { value: "Engineering", label: "Engineering" },
    { value: "Humanities", label: "Humanities" },
    { value: "Languages", label: "Languages" },
    { value: "Mathematics", label: "Mathematics" },
    { value: "Science", label: "Science" },
    { value: "Social sciences", label: "Social sciences" },
    { value: "Standard curricula", label: "School curricula" },
    { value: "Recreational activities", label: "Recreation" },
    { value: "Miscellaneous", label: "Miscellaneous" },
    { value: "Wikijunior", label: "Wikijunior (for children)" }
  ];
  var STATUSES = [
    { value: "all", label: "All books" },
    { value: "Featured books", label: "Featured" },
    { value: "Completed books", label: "Completed" }
  ];
  var SORTS = [
    { value: "relevance", label: "Best match" },
    { value: "incoming_links_desc", label: "Most linked" },
    { value: "last_edit_desc", label: "Recently edited" },
    { value: "create_timestamp_desc", label: "Newest" }
  ];
  var turn = Promise.resolve();
  var inTurn = (task) => {
    const next = turn.then(task);
    turn = next.catch(() => void 0);
    return next;
  };
  var request = (url) => inTurn(
    () => raygun.http.fetch(url, { headers: HEADERS }).catch(() => {
      throw new Error("Wikibooks didn't answer; try again in a minute");
    })
  );
  var api = async (params) => {
    const query = Object.entries({ format: "json", formatversion: 2, ...params }).map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`).join("&");
    const response = await request(`${API}?${query}`);
    if (!response.ok) throw new Error(`Wikibooks answered with an error (HTTP ${response.status})`);
    const answer = await response.json();
    if (answer.error) {
      throw new Error(`Wikibooks couldn't answer: ${answer.error.info ?? "the request failed"}`);
    }
    return answer;
  };
  var decode = (text) => text.replace(/&nbsp;|&#160;/g, " ").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#0?39;|&apos;/g, "'").replace(
    /&#x([0-9a-f]+);/gi,
    (_match, code) => String.fromCodePoint(parseInt(code, 16))
  ).replace(/&#(\d+);/g, (_match, code) => String.fromCodePoint(Number(code))).replace(/&amp;/g, "&");
  var plainText = (html) => decode(
    html.replace(/<style[\s\S]*?<\/style>/g, "").replace(/<br\s*\/?>|<\/(?:p|div|li|dd|dt|td|th|h\d)>/gi, " ").replace(/<[^>]+>/g, "")
  ).replace(/\s+/g, " ").trim();
  var wikiText = (text) => decode(
    text.replace(/\{\{[^{}]*\}\}/g, "").replace(/\[\[(?:[^\]|]*\|)?([^\]]*)\]\]/g, "$1").replace(/'{2,}/g, "").replace(/<[^>]+>/g, "")
  ).replace(/\s+/g, " ").trim();
  var VOID_ELEMENTS = /* @__PURE__ */ new Set([
    "area",
    "base",
    "br",
    "col",
    "embed",
    "hr",
    "img",
    "input",
    "link",
    "meta",
    "source",
    "track",
    "wbr"
  ]);
  var TAG = /<(\/?)([a-zA-Z][\w:-]*)((?:[^>"']|"[^"]*"|'[^']*')*)>/g;
  var dropElements = (html, drop) => {
    let kept2 = "";
    let from = 0;
    let open = null;
    for (const match of html.matchAll(TAG)) {
      const [whole, closing, name = "", attributes = ""] = match;
      const tag = name.toLowerCase();
      const at = match.index ?? 0;
      const lone = VOID_ELEMENTS.has(tag) || attributes.trimEnd().endsWith("/");
      if (open) {
        if (tag === open.tag && !lone) open.depth += closing ? -1 : 1;
        if (open.depth === 0) {
          open = null;
          from = at + whole.length;
        }
        continue;
      }
      if (closing || !drop(tag, attributes)) continue;
      kept2 += html.slice(from, at);
      from = at + whole.length;
      if (!lone) open = { tag, depth: 1 };
    }
    return open ? kept2 : kept2 + html.slice(from);
  };
  var classes = (attributes) => /\bclass\s*=\s*(?:"([^"]*)"|'([^']*)')/i.exec(attributes)?.slice(1).join(" ").split(/\s+/).filter(Boolean) ?? [];
  var normalTitle = (title) => {
    const spaced = decode(title).replace(/_/g, " ").replace(/\s+/g, " ").trim();
    return spaced.charAt(0).toUpperCase() + spaced.slice(1);
  };
  var bookOf = (title) => title.split("/")[0] ?? title;
  var bookName = (title) => title.replace(/^Wikijunior:\s*/, "Wikijunior: ");
  var pageUrl = (title) => `${WIKI}/wiki/${title.replace(/ /g, "_").split("/").map(encodeURIComponent).join("/")}`;
  var cleanImage = (url) => url ? (url.startsWith("//") ? `https:${url}` : url).replace(/\?utm_[^#]*$/, "") : null;
  var kept = /* @__PURE__ */ new Map();
  var remembered = (key, hours, load) => {
    const known = kept.get(key);
    if (known && Date.now() - known.at < hours * 36e5) return known.value;
    const value = load();
    kept.set(key, { at: Date.now(), value });
    value.catch(() => kept.delete(key));
    return value;
  };
  var shown = /* @__PURE__ */ new Map();
  var unseen = (list, page, items) => {
    let pages = shown.get(list);
    if (!pages) {
      pages = [];
      shown.set(list, pages);
      if (shown.size > 30) shown.delete(shown.keys().next().value ?? list);
    }
    const before = new Set(pages.slice(0, page - 1).flat());
    const fresh = items.filter((item) => !before.has(item.id));
    pages[page - 1] = fresh.map((item) => item.id);
    return fresh;
  };
  var books = async (titles) => {
    const asked = [...new Set(titles)].slice(0, 50);
    if (asked.length === 0) return [];
    const answer = await api({
      action: "query",
      titles: asked.join("|"),
      prop: "categories|pageimages",
      clcategories: BOOK_CATEGORIES.join("|"),
      cllimit: "max",
      piprop: "thumbnail",
      pithumbsize: 400,
      redirects: 1
    });
    const renamed = /* @__PURE__ */ new Map();
    for (const rename of [...answer.query?.normalized ?? [], ...answer.query?.redirects ?? []]) {
      renamed.set(rename.from, rename.to);
    }
    const pages = new Map((answer.query?.pages ?? []).map((page) => [page.title, page]));
    const seen = /* @__PURE__ */ new Set();
    return asked.flatMap((title) => {
      let name = title;
      for (let hop = 0; hop < 3; hop += 1) name = renamed.get(name) ?? name;
      const page = pages.get(name);
      const isBook = page !== void 0 && !page.missing && !page.invalid && (page.ns === 0 || page.ns === 110) && !page.title.includes("/") && !NOT_BOOKS.has(page.title) && (page.categories?.length ?? 0) > 0;
      if (!isBook || seen.has(page.title)) return [];
      seen.add(page.title);
      return [
        { id: page.title, title: bookName(page.title), coverUrl: cleanImage(page.thumbnail?.source) }
      ];
    });
  };
  var monthBack = (back) => {
    const now = /* @__PURE__ */ new Date();
    const month = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth() - back, 1));
    return `${month.getUTCFullYear()}/${String(month.getUTCMonth() + 1).padStart(2, "0")}`;
  };
  var mostRead = () => remembered("popular", 12, async () => {
    for (const back of [1, 2]) {
      const response = await request(`${PAGEVIEWS}/${monthBack(back)}/all-days`);
      if (response.status === 404) continue;
      if (!response.ok) {
        throw new Error(
          `Wikimedia's reading counts answered with an error (HTTP ${response.status})`
        );
      }
      const answer = await response.json();
      const views = /* @__PURE__ */ new Map();
      for (const { article, views: count } of answer.items?.[0]?.articles ?? []) {
        const title = normalTitle(article);
        if (OTHER_PAGES.test(title)) continue;
        const book = bookOf(title);
        views.set(book, (views.get(book) ?? 0) + count);
      }
      return [...views].sort((a, b) => b[1] - a[1]).map(([book]) => book);
    }
    throw new Error("Wikimedia hasn't counted last month's reading yet; try again tomorrow");
  });
  var recentlyCompleted = () => remembered("completed", 12, async () => {
    const answer = await api({
      action: "query",
      list: "categorymembers",
      cmtitle: "Category:Completed books",
      cmnamespace: BOOK_NAMESPACES,
      cmsort: "timestamp",
      cmdir: "desc",
      cmprop: "title",
      cmlimit: "max"
    });
    return (answer.query?.categorymembers ?? []).map((member) => member.title);
  });
  var listPage = async (list, titles, page) => {
    const start = (Math.max(1, page) - 1) * PAGE_SIZE;
    const items = await books(titles.slice(start, start + PAGE_SIZE));
    return { items: unseen(list, page, items), hasNextPage: start + PAGE_SIZE < titles.length };
  };
  var searchBooks = async (search, sort, page) => {
    const offset = (Math.max(1, page) - 1) * HITS;
    const answer = await api({
      action: "query",
      list: "search",
      srsearch: search,
      srnamespace: BOOK_NAMESPACES,
      srlimit: HITS,
      sroffset: offset,
      srsort: sort,
      srprop: "",
      srinfo: "totalhits"
    });
    const hits = answer.query?.search ?? [];
    const items = await books(hits.map((hit) => bookOf(hit.title)));
    return {
      items: unseen(`search ${sort} ${search}`, page, items),
      hasNextPage: hits.length > 0 && offset + hits.length < (answer.query?.searchinfo?.totalhits ?? 0)
    };
  };
  var parsedPages = /* @__PURE__ */ new Map();
  var parsed = (title) => {
    let page = parsedPages.get(title);
    if (!page) {
      page = api({
        action: "parse",
        page: title,
        prop: "text",
        redirects: 1,
        disableeditsection: 1,
        disablelimitreport: 1,
        disabletoc: 1
      }).then((answer) => {
        if (!answer.parse) throw new Error("Wikibooks has no page by this name");
        return { title: answer.parse.title, html: answer.parse.text };
      });
      page.catch(() => parsedPages.delete(title));
      parsedPages.set(title, page);
      if (parsedPages.size > 8) parsedPages.delete(parsedPages.keys().next().value ?? title);
    }
    return page;
  };
  var licenceName = (rights) => {
    const creativeCommons = /creativecommons\.org\/licenses\/([a-z-]+)\/(\d+(?:\.\d+)?)/i.exec(
      rights?.url ?? ""
    );
    if (creativeCommons) {
      return `CC ${(creativeCommons[1] ?? "").toUpperCase()} ${creativeCommons[2] ?? ""}`;
    }
    return rights?.text || "CC BY-SA 4.0";
  };
  var details = async (id) => {
    const answer = await api({
      action: "query",
      titles: id,
      prop: "extracts|pageimages|categories",
      exintro: 1,
      explaintext: 1,
      piprop: "thumbnail",
      pithumbsize: 600,
      cllimit: "max",
      clprop: "hidden",
      meta: "siteinfo",
      siprop: "rightsinfo",
      redirects: 1
    });
    const page = answer.query?.pages?.[0];
    if (!page || page.missing || page.invalid) throw new Error("Wikibooks has no book by this name");
    const categories = page.categories ?? [];
    const names = categories.map((category) => category.title.replace(/^Category:/, ""));
    const shelves = categories.filter((category) => !category.hidden && category.title.startsWith("Category:Shelf:")).map((category) => category.title.slice("Category:Shelf:".length));
    const about = (page.extract ?? "").replace(/\n+/g, "\n\n").trim();
    const licence = licenceName(answer.query?.rightsinfo);
    return {
      id: page.title,
      title: bookName(page.title),
      coverUrl: cleanImage(page.thumbnail?.source),
      authors: [],
      description: [
        about,
        `Written by Wikibooks' contributors. Licence: ${licence}, as Wikibooks states for all of its text.`
      ].filter(Boolean).join("\n\n"),
      genres: [...new Set(shelves)].slice(0, 8),
      status: names.includes("Completed books") ? "completed" : "ongoing",
      url: pageUrl(page.title)
    };
  };
  var leaf = (title, book) => title === book ? bookName(book) : title.slice(book.length + 1).replace(/\//g, " · ");
  var collection = async (book) => {
    const name = `Wikibooks:Collections/${book}`;
    const answer = await api({
      action: "query",
      titles: name,
      prop: "revisions",
      rvprop: "content",
      rvslots: "main",
      redirects: 1
    });
    const page = answer.query?.pages?.[0];
    if (!page || page.missing || page.title !== name) return [];
    const links = [];
    let part = "";
    for (const line of (page.revisions?.[0]?.slots?.main?.content ?? "").split("\n")) {
      const heading = /^;\s*(.+)$/.exec(line);
      if (heading) {
        part = wikiText(heading[1] ?? "");
        continue;
      }
      const link = /^:\s*\[\[\s*([^\]|#]+)(?:#[^\]|]*)?(?:\|([^\]]*))?\]\]/.exec(line);
      if (!link) continue;
      const title = normalTitle(link[1] ?? "");
      if (title !== book && !title.startsWith(`${book}/`) || WHOLE_BOOK_PAGES.test(title)) continue;
      const label = wikiText(link[2] ?? "") || leaf(title, book);
      links.push({ title, label: part ? `${part} · ${label}` : label });
    }
    return links;
  };
  var subpageLinks = (html, book) => {
    const links = [];
    for (const match of html.matchAll(
      /<a href="\/wiki\/[^"]*"[^>]*?\btitle="([^"]+)"[^>]*>([\s\S]*?)<\/a>/g
    )) {
      const title = decode(match[1] ?? "");
      if (!title.startsWith(`${book}/`) || WHOLE_BOOK_PAGES.test(title)) continue;
      const text = plainText(match[2] ?? "");
      links.push({ title, label: /^[\p{Lu}\p{N}]/u.test(text) ? text : leaf(title, book) });
    }
    return links;
  };
  var allSubpages = async (book) => {
    const junior = book.startsWith("Wikijunior:");
    const answer = await api({
      action: "query",
      list: "allpages",
      apnamespace: junior ? 110 : 0,
      apprefix: `${junior ? book.slice("Wikijunior:".length) : book}/`,
      apfilterredir: "nonredirects",
      aplimit: "max"
    });
    return (answer.query?.allpages ?? []).map((page) => page.title).filter((title) => !WHOLE_BOOK_PAGES.test(title)).map((title) => ({ title, label: leaf(title, book) }));
  };
  var chapterList = async (id) => {
    const main = await parsed(id);
    const book = main.title;
    let links = await collection(book);
    if (links.length === 0) links = subpageLinks(main.html, book);
    if (links.length === 0) links = await allSubpages(book);
    const ownText = plainText(cleanPage(main.html).replace(/<a\b[\s\S]*?<\/a>/g, "")).length;
    if (links.length === 0 || ownText > MAIN_TEXT && !links.some((link) => link.title === book)) {
      links.unshift({ title: book, label: bookName(book) });
    }
    const seen = /* @__PURE__ */ new Set();
    return links.filter((link) => !seen.has(link.title) && seen.add(link.title)).map((link, index) => ({ id: link.title, name: link.label, number: index + 1 }));
  };
  var cleanPage = (html) => dropElements(
    html.replace(/<!--[\s\S]*?-->/g, ""),
    (_tag, attributes) => classes(attributes).some(
      (name) => DROPPED_CLASSES.has(name) || name.startsWith("mbox-") || // Pictures of formulas, shown only where MathML can't be: the MathML stays.
      name.startsWith("mwe-math-fallback")
    )
  );
  var pick = (filters2, key, options) => {
    const value = filters2?.[key];
    return typeof value === "string" && options.some((option) => option.value === value) ? value : null;
  };
  var filters = [
    { kind: "select", key: "subject", label: "Subject", options: SUBJECTS },
    { kind: "select", key: "status", label: "Books", options: STATUSES },
    { kind: "select", key: "sort", label: "Sort", options: SORTS }
  ];
  registerProvider(
    defineProvider({
      kind: "novel.source",
      id: "books",
      name: "Wikibooks",
      popular: async ({ page }) => listPage("popular", await mostRead(), page),
      latest: async ({ page }) => listPage("completed", await recentlyCompleted(), page),
      search: ({ query, page, filters: values }) => {
        const words = query.trim();
        const subject = pick(values, "subject", SUBJECTS);
        const status = pick(values, "status", STATUSES);
        const categories = [
          subject && subject !== "all" && `Department:${subject}/all books`,
          status && status !== "all" && status
        ].filter((category) => Boolean(category));
        if (!words && categories.length === 0) categories.push("Book:Wikibooks Stacks/Books");
        const search = [words, ...categories.map((category) => `incategory:"${category}"`)].filter(Boolean).join(" ");
        const sort = pick(values, "sort", SORTS) ?? (words ? "relevance" : "incoming_links_desc");
        return searchBooks(search, sort, page);
      },
      filters: async () => filters,
      details: ({ id }) => details(id),
      chapters: async ({ id }) => ({ chapters: await chapterList(id) }),
      passage: async ({ chapterId }) => {
        const page = await parsed(chapterId);
        return { html: cleanPage(page.html), baseUrl: pageUrl(page.title) };
      }
    })
  );
})();
