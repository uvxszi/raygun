// dev.raygun.wikisource 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var WIKI = "https://en.wikisource.org";
  var API = `${WIKI}/w/api.php`;
  var PAGE_SIZE = 30;
  var AGENT = "Raygun/1.0 (https://github.com/uvxszi/raygun)";
  var HEADERS = { "User-Agent": AGENT, "Api-User-Agent": AGENT };
  var MAX_CONTAINERS = 24;
  var MAIN_TEXT = 5e3;
  var NOT_WORKS = '-incategory:"Versions_pages" -incategory:"Disambiguation_pages"';
  var LITERATURE = [
    "Novels",
    "Short_stories",
    "Collections_of_short_stories",
    "Poems",
    "Collections_of_poetry",
    "Plays",
    "Children's_books",
    "Fairy_tales",
    "Essays"
  ].join("|");
  var GENRES = [
    { value: "all", label: "All" },
    { value: "Novels", label: "Novels" },
    { value: "Short stories", label: "Short stories" },
    { value: "Collections of short stories", label: "Story collections" },
    { value: "Poems", label: "Poems" },
    { value: "Collections of poetry", label: "Poetry collections" },
    { value: "Plays", label: "Plays" },
    { value: "Children's books", label: "Children’s books" },
    { value: "Fairy tales", label: "Fairy tales" },
    { value: "Adventure novels", label: "Adventure" },
    { value: "Mystery", label: "Mystery" },
    { value: "Ghost stories", label: "Ghost stories" },
    { value: "Science fiction short stories", label: "Science fiction" },
    { value: "Romance novels", label: "Romance" },
    { value: "Essays", label: "Essays" },
    { value: "Speeches", label: "Speeches" },
    { value: "Biographies", label: "Biographies" },
    { value: "Travel", label: "Travel" },
    { value: "Philosophy", label: "Philosophy" }
  ];
  var SORTS = [
    { value: "relevance", label: "Best match" },
    { value: "incoming_links_desc", label: "Most linked" },
    { value: "create_timestamp_desc", label: "Newest" },
    { value: "last_edit_desc", label: "Recently edited" }
  ];
  var api = async (params) => {
    const query = Object.entries({ format: "json", formatversion: 2, ...params }).map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`).join("&");
    const response = await raygun.http.fetch(`${API}?${query}`, { headers: HEADERS });
    if (!response.ok) throw new Error(`Wikisource answered HTTP ${response.status}`);
    const answer = await response.json();
    if (answer.error) throw new Error(`Wikisource: ${answer.error.info ?? "the request failed"}`);
    return answer;
  };
  var decode = (text) => text.replace(/&nbsp;|&#160;/g, " ").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#0?39;|&apos;/g, "'").replace(
    /&#x([0-9a-f]+);/gi,
    (_match, code) => String.fromCodePoint(parseInt(code, 16))
  ).replace(/&#(\d+);/g, (_match, code) => String.fromCodePoint(Number(code))).replace(/&amp;/g, "&");
  var INVISIBLE = new RegExp(`[${String.fromCharCode(8203, 8288)}]`, "g");
  var plainText = (html) => decode(
    html.replace(/<style[\s\S]*?<\/style>/g, "").replace(/<br\s*\/?>|<\/(?:p|div|li|dd|dt|td|th|h\d)>/gi, " ").replace(/<[^>]+>/g, "")
  ).replace(INVISIBLE, "").replace(/\s+/g, " ").trim();
  var categoryName = (category) => (category.category ?? category.title?.replace(/^Category:/, "") ?? "").replace(/_/g, " ");
  var isOpenLicence = (name) => /^(PD-|CC[- ]|CC0|Public domain)/i.test(name);
  var isGenre = (name) => !isOpenLicence(name) && !/^(Validated|Proofread|Featured|Problematic|Not proofread|Spoken works|Versions pages|Works |Index |Pages |Texts |Wikisource|Novels adapted|Newbery|Author|Translations)/i.test(
    name
  ) && !/\bworks$|\d{3,4}s?\b/i.test(name);
  var workOf = (title) => title.split("/")[0] ?? title;
  var pageUrl = (title) => `${WIKI}/wiki/${title.replace(/ /g, "_").split("/").map(encodeURIComponent).join("/")}`;
  var cleanImage = (url) => url ? (url.startsWith("//") ? `https:${url}` : url).replace(/\?utm_[^#]*$/, "") : null;
  var works = async (titles) => {
    if (titles.length === 0) return [];
    const pages = /* @__PURE__ */ new Map();
    const renamed = /* @__PURE__ */ new Map();
    let next = {};
    for (let round = 0; round < 3; round += 1) {
      const answer = await api({
        action: "query",
        titles: titles.join("|"),
        prop: "categories|pageprops|pageimages",
        cllimit: "max",
        clprop: "hidden",
        ppprop: "disambiguation",
        piprop: "thumbnail",
        pithumbsize: 400,
        redirects: 1,
        ...next
      });
      for (const redirect of answer.query?.redirects ?? []) renamed.set(redirect.from, redirect.to);
      for (const page of answer.query?.pages ?? []) {
        const known = pages.get(page.title);
        pages.set(page.title, {
          ...known,
          ...page,
          categories: [...known?.categories ?? [], ...page.categories ?? []]
        });
      }
      if (!answer.continue?.clcontinue) break;
      next = { clcontinue: answer.continue.clcontinue };
    }
    const seen = /* @__PURE__ */ new Set();
    return titles.flatMap((asked) => {
      const title = renamed.get(asked) ?? asked;
      const page = pages.get(title);
      if (!page || page.missing || seen.has(title)) return [];
      seen.add(title);
      const names = (page.categories ?? []).map(categoryName);
      const listable = names.some(isOpenLicence) && !names.includes("Versions pages") && !("disambiguation" in (page.pageprops ?? {}));
      return listable ? [{ id: title, title, coverUrl: cleanImage(page.thumbnail?.source) }] : [];
    });
  };
  var searchWorks = async (search, sort, page) => {
    const offset = (Math.max(1, page) - 1) * PAGE_SIZE;
    const answer = await api({
      action: "query",
      list: "search",
      srnamespace: 0,
      srlimit: PAGE_SIZE,
      sroffset: offset,
      srsearch: search,
      srsort: sort,
      srprop: "",
      srinfo: "totalhits"
    });
    const hits = answer.query?.search ?? [];
    const titles = [...new Set(hits.map((hit) => workOf(hit.title)))];
    return {
      items: await works(titles),
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
        prop: "text|categories",
        redirects: 1,
        disableeditsection: 1,
        disablelimitreport: 1,
        disabletoc: 1
      }).then((answer) => {
        if (!answer.parse) throw new Error("Wikisource has no page by this name");
        return {
          title: answer.parse.title,
          html: answer.parse.text,
          categories: answer.parse.categories ?? []
        };
      });
      page.catch(() => parsedPages.delete(title));
      parsedPages.set(title, page);
      if (parsedPages.size > 12) parsedPages.delete(parsedPages.keys().next().value);
    }
    return page;
  };
  var bodyOf = (html) => {
    const data = html.indexOf('id="ws-data"');
    let start = 0;
    if (data >= 0) {
      const end = html.indexOf("</div></div>", data);
      start = end >= 0 ? end + "</div></div>".length : data;
    }
    const licence = html.indexOf('<div class="licenseContainer', start);
    return html.slice(start, licence >= 0 ? licence : html.length);
  };
  var headerField = (html, field) => {
    const value = new RegExp(`<span id="ws-${field}">([\\s\\S]*?)</span>`).exec(html)?.[1];
    return value ? plainText(value) || null : null;
  };
  var subpageLinks = (html, parent) => {
    const prefix = `${parent}/`;
    const seen = /* @__PURE__ */ new Set();
    const links = [];
    for (const match of html.matchAll(
      /<a href="\/wiki\/[^"]*"[^>]*?\btitle="([^"]+)"[^>]*>([\s\S]*?)<\/a>/g
    )) {
      const title = decode(match[1] ?? "");
      if (!title.startsWith(prefix) || seen.has(title)) continue;
      seen.add(title);
      links.push({ title, label: plainText(match[2] ?? "") || title.slice(prefix.length) });
    }
    return links;
  };
  var allSubpages = async (root) => {
    const titles = [];
    let next = {};
    for (let round = 0; round < 4; round += 1) {
      const answer = await api({
        action: "query",
        list: "allpages",
        apnamespace: 0,
        apprefix: `${root}/`,
        aplimit: "max",
        ...next
      });
      titles.push(...(answer.query?.allpages ?? []).map((page) => page.title));
      if (!answer.continue?.apcontinue) break;
      next = { apcontinue: answer.continue.apcontinue };
    }
    return titles;
  };
  var inTurns = async (items, width, task) => {
    const results = [];
    for (let start = 0; start < items.length; start += width) {
      results.push(...await Promise.all(items.slice(start, start + width).map(task)));
    }
    return results;
  };
  var chapterList = async (id) => {
    const main = await parsed(id);
    const body = bodyOf(main.html);
    const top = subpageLinks(body, main.title);
    if (top.length === 0) return [{ id: main.title, name: "Full text", number: 1 }];
    const all = await allSubpages(main.title).catch(() => []);
    const containers = top.filter((link) => all.some((title) => title.startsWith(`${link.title}/`))).slice(0, MAX_CONTAINERS);
    const inner = new Map(
      await inTurns(containers, 4, async (link) => {
        const page = await parsed(link.title).catch(() => null);
        return [link.title, page ? subpageLinks(bodyOf(page.html), link.title) : []];
      })
    );
    const listed = [];
    const ownText = plainText(body.replace(/<a\b[\s\S]*?<\/a>/g, "")).length;
    if (ownText > MAIN_TEXT)
      listed.push({ title: main.title, label: headerField(main.html, "title") ?? main.title });
    for (const link of top) {
      const children = inner.get(link.title) ?? [];
      if (children.length === 0) listed.push(link);
      for (const child of children)
        listed.push({ title: child.title, label: `${link.label} · ${child.label}` });
    }
    const seen = /* @__PURE__ */ new Set();
    return listed.filter((link) => !seen.has(link.title) && seen.add(link.title)).map((link, index) => ({ id: link.title, name: link.label, number: index + 1 }));
  };
  var details = async (id) => {
    const [page, listed] = await Promise.all([parsed(id), works([id]).catch(() => [])]);
    const html = page.html;
    const notes = /<div class="wst-header-content[^"]*"[^>]*>([\s\S]*?)<\/div>/.exec(html)?.[1];
    const about = notes ? plainText(notes).replace(
      /\s*one or more chapters are available in a spoken word format\.?/i,
      ""
    ) : "";
    const year = headerField(html, "year");
    const translator = headerField(html, "translator");
    const description = [
      about && about.charAt(0).toUpperCase() + about.slice(1),
      [year && `Published ${year}.`, translator && `Translated by ${translator}.`].filter(Boolean).join(" ")
    ].filter(Boolean).join("\n\n");
    const author = headerField(html, "author");
    return {
      id: page.title,
      title: headerField(html, "title") ?? page.title,
      coverUrl: listed[0]?.coverUrl ?? null,
      authors: author ? author.split(/\s*(?:,|;| and )\s*/).filter(Boolean) : [],
      description: description || null,
      genres: page.categories.filter((category) => !category.hidden).map(categoryName).filter(isGenre).slice(0, 8),
      status: "completed",
      url: pageUrl(page.title)
    };
  };
  var pick = (filters2, key, options) => {
    const value = filters2?.[key];
    return typeof value === "string" && options.some((option) => option.value === value) ? value : null;
  };
  var filters = [
    { kind: "select", key: "genre", label: "Genre", options: GENRES },
    { kind: "select", key: "sort", label: "Sort", options: SORTS }
  ];
  registerProvider(
    defineProvider({
      kind: "novel.source",
      id: "texts",
      name: "Wikisource",
      popular: ({ page }) => searchWorks(`incategory:"Featured_texts" ${NOT_WORKS}`, "incoming_links_desc", page),
      latest: ({ page }) => searchWorks(
        `incategory:"Validated_texts" incategory:"${LITERATURE}" ${NOT_WORKS}`,
        "create_timestamp_desc",
        page
      ),
      search: ({ query, page, filters: values }) => {
        const words = query.trim();
        const genre = pick(values, "genre", GENRES);
        const search = [
          words,
          genre && genre !== "all" && `incategory:"${genre.replace(/ /g, "_")}"`,
          NOT_WORKS
        ].filter(Boolean).join(" ");
        const sort = pick(values, "sort", SORTS) ?? (words ? "relevance" : "incoming_links_desc");
        return searchWorks(search, sort, page);
      },
      filters: async () => filters,
      details: ({ id }) => details(id),
      chapters: async ({ id }) => ({ chapters: await chapterList(id) }),
      passage: async ({ chapterId }) => {
        const page = await parsed(chapterId);
        const html = bodyOf(page.html).replace(
          /<span class="pagenum[^"]*"[^>]*>(?:<span[^>]*>[^<]*<\/span>)?<\/span>/g,
          ""
        );
        return { html, baseUrl: pageUrl(page.title) };
      }
    })
  );
})();
