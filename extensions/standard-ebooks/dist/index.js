// dev.raygun.standard-ebooks 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var SITE = "https://standardebooks.org";
  var PER_PAGE = 48;
  var HEADERS = {
    Accept: "application/xhtml+xml",
    "User-Agent": "Raygun/1.0 (https://github.com/uvxszi/raygun)"
  };
  var FRAME_PAGES = /* @__PURE__ */ new Set(["titlepage", "imprint", "halftitlepage", "colophon", "uncopyright"]);
  var BOOK_ID = /^[a-z0-9-]+(\/[a-z0-9_-]+){1,3}$/;
  var PAGE_NAME = /^[a-z0-9_.-]+$/;
  var SUBJECTS = [
    { value: "all", label: "All" },
    { value: "adventure", label: "Adventure" },
    { value: "autobiography", label: "Autobiography" },
    { value: "biography", label: "Biography" },
    { value: "childrens", label: "Children’s" },
    { value: "comedy", label: "Comedy" },
    { value: "drama", label: "Drama" },
    { value: "fantasy", label: "Fantasy" },
    { value: "fiction", label: "Fiction" },
    { value: "horror", label: "Horror" },
    { value: "memoir", label: "Memoir" },
    { value: "mystery", label: "Mystery" },
    { value: "nonfiction", label: "Nonfiction" },
    { value: "philosophy", label: "Philosophy" },
    { value: "poetry", label: "Poetry" },
    { value: "satire", label: "Satire" },
    { value: "science-fiction", label: "Science fiction" },
    { value: "shorts", label: "Short stories" },
    { value: "spirituality", label: "Spirituality" },
    { value: "travel", label: "Travel" }
  ];
  var SORTS = [
    { value: "relevance", label: "Best match" },
    { value: "popularity", label: "Most popular" },
    { value: "newest", label: "Newest editions" },
    { value: "author-alpha", label: "Author (A–Z)" },
    { value: "reading-ease", label: "Easiest to read" },
    { value: "length", label: "Shortest first" }
  ];
  var decode = (text) => text.replace(/&nbsp;/g, " ").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(
    /&#x([0-9a-f]+);/gi,
    (_match, code) => String.fromCodePoint(parseInt(code, 16))
  ).replace(/&#(\d+);/g, (_match, code) => String.fromCodePoint(Number(code))).replace(/&amp;/g, "&");
  var WORD_JOINER = new RegExp(String.fromCharCode(8288), "g");
  var plainText = (html) => decode(html.replace(/<br\s*\/?>|<\/(?:p|div|li|h\d)>/gi, " ").replace(/<[^>]+>/g, "")).replace(WORD_JOINER, "").replace(/\s+/g, " ").trim();
  var paragraphs = (html) => [...html.matchAll(/<p[^>]*>([\s\S]*?)<\/p>/g)].map((match) => plainText(match[1] ?? "")).filter(Boolean).join("\n\n");
  var absolute = (path) => path.startsWith("http") ? path : `${SITE}${path}`;
  var fetchPage = async (path) => {
    const response = await raygun.http.fetch(`${SITE}${path}`, { headers: HEADERS });
    if (response.status === 404) return null;
    if (!response.ok) throw new Error(`Standard Ebooks answered HTTP ${response.status}`);
    return { text: await response.text(), url: response.url };
  };
  var bookId = (id) => {
    if (!BOOK_ID.test(id)) throw new Error("Not a Standard Ebooks book");
    return id;
  };
  var books = (xhtml) => [...xhtml.matchAll(/<li typeof="schema:Book" about="\/ebooks\/([^"]+)">([\s\S]*?)<\/li>/g)].map((match) => {
    const id = decode(match[1] ?? "");
    const item = match[2] ?? "";
    const title = /<span property="schema:name">([\s\S]*?)<\/span>/.exec(item)?.[1];
    const cover = /<img src="([^"]+)"/.exec(item)?.[1];
    return {
      id,
      title: title ? plainText(title) : "",
      coverUrl: cover ? absolute(decode(cover)) : null
    };
  }).filter((book) => BOOK_ID.test(book.id) && book.title !== "");
  var pick = (filters2, key, options) => {
    const value = filters2?.[key];
    return typeof value === "string" && options.some((option) => option.value === value) ? value : null;
  };
  var catalogue = async ({ query = "", subject = null, sort }, page) => {
    const params = [
      query && `query=${encodeURIComponent(query)}`,
      subject && subject !== "all" && `tags%5B%5D=${encodeURIComponent(subject)}`,
      `sort=${encodeURIComponent(sort)}`,
      `per-page=${PER_PAGE}`,
      `page=${Math.max(1, page)}`
    ].filter(Boolean);
    const found = await fetchPage(`/ebooks?${params.join("&")}`);
    if (!found) return { items: [], hasNextPage: false };
    const items = books(found.text);
    return { items, hasNextPage: items.length > 0 && /rel="next"/.test(found.text) };
  };
  var tocEntries = (xhtml) => {
    const nav = /<nav[^>]*id="toc"[^>]*>([\s\S]*?)<\/nav>/.exec(xhtml)?.[1] ?? "";
    const entries = [];
    let depth = 0;
    for (const match of nav.matchAll(
      /<(\/?)ol\b[^>]*>|<a href="text\/([^"#]+)[^"]*"[^>]*>([\s\S]*?)<\/a>/g
    )) {
      if (match[2] === void 0) {
        depth += match[1] ? -1 : 1;
        continue;
      }
      entries.push({ file: match[2], label: plainText(match[3] ?? ""), depth });
    }
    return entries;
  };
  var chapterList = (id, entries) => {
    const chapters = [];
    const open = [];
    const seen = /* @__PURE__ */ new Set();
    entries.forEach((entry, index) => {
      while ((open[open.length - 1]?.depth ?? -1) >= entry.depth) open.pop();
      const within = open.filter((parent) => !FRAME_PAGES.has(parent.file)).map((parent) => parent.label);
      open.push(entry);
      const heads = (entries[index + 1]?.depth ?? 0) > entry.depth;
      if (FRAME_PAGES.has(entry.file) || heads || seen.has(entry.file) || !PAGE_NAME.test(entry.file)) {
        return;
      }
      seen.add(entry.file);
      chapters.push({
        id: `${id}/text/${entry.file}`,
        name: [...within, entry.label || entry.file].join(" · "),
        number: chapters.length + 1
      });
    });
    return chapters;
  };
  var details = async (id) => {
    const found = await fetchPage(`/ebooks/${bookId(id)}`);
    if (!found) throw new Error("This book is no longer in the Standard Ebooks catalogue");
    const page = found.text;
    const title = /<h1 property="schema:name">([\s\S]*?)<\/h1>/.exec(page)?.[1];
    if (!title) throw new Error("Standard Ebooks sent a page without a book");
    const heading = /<hgroup>([\s\S]*?)<\/hgroup>/.exec(page)?.[1] ?? "";
    const authors = [
      ...heading.matchAll(
        /property="schema:author"[^>]*>\s*<span property="schema:name">([\s\S]*?)<\/span>/g
      )
    ].map((match) => plainText(match[1] ?? ""));
    const facts = /<aside id="reading-ease">([\s\S]*?)<\/aside>/.exec(page)?.[1] ?? "";
    const translators = /<p>(Translated by [\s\S]*?)<\/p>/.exec(facts)?.[1];
    const genres = [...facts.matchAll(/<a href="\/subjects\/[^"]+">([\s\S]*?)<\/a>/g)].map(
      (match) => plainText(match[1] ?? "")
    );
    const about = /<div property="schema:description">([\s\S]*?)<\/div>/.exec(page)?.[1];
    const summary = /<meta property="schema:abstract" content="([^"]*)"/.exec(page)?.[1];
    const description = [
      about ? paragraphs(about) : summary ? decode(summary) : "",
      translators && plainText(translators)
    ].filter(Boolean).join("\n\n");
    const cover = /<meta property="schema:thumbnailUrl" content="([^"]+)"/.exec(page)?.[1];
    return {
      id,
      title: plainText(title),
      coverUrl: cover ? absolute(decode(cover)) : null,
      authors,
      description: description || null,
      genres,
      status: "completed",
      url: `${SITE}/ebooks/${id}`
    };
  };
  var filters = [
    { kind: "select", key: "subject", label: "Subject", options: SUBJECTS },
    { kind: "select", key: "sort", label: "Sort", options: SORTS }
  ];
  registerProvider(
    defineProvider({
      kind: "novel.source",
      id: "books",
      name: "Standard Ebooks",
      popular: ({ page }) => catalogue({ sort: "popularity" }, page),
      latest: ({ page }) => catalogue({ sort: "newest" }, page),
      search: ({ query, page, filters: values }) => {
        const words = query.trim();
        return catalogue(
          {
            query: words,
            subject: pick(values, "subject", SUBJECTS),
            sort: pick(values, "sort", SORTS) ?? (words ? "relevance" : "popularity")
          },
          page
        );
      },
      filters: async () => filters,
      details: ({ id }) => details(id),
      chapters: async ({ id }) => {
        const found = await fetchPage(`/ebooks/${bookId(id)}/text`);
        if (!found) throw new Error("This book is no longer in the Standard Ebooks catalogue");
        const chapters = chapterList(id, tocEntries(found.text));
        if (chapters.length === 0) throw new Error("Standard Ebooks lists no chapters for this book");
        return { chapters };
      },
      passage: async ({ chapterId }) => {
        const at = chapterId.lastIndexOf("/text/");
        const book = chapterId.slice(0, Math.max(at, 0));
        const name = chapterId.slice(at + "/text/".length);
        if (at < 0 || !PAGE_NAME.test(name)) throw new Error("Not a Standard Ebooks chapter");
        const found = await fetchPage(`/ebooks/${bookId(book)}/text/${name}`);
        if (!found) throw new Error("This chapter is no longer in the book");
        const main = /<main\b[^>]*>([\s\S]*?)<\/main>/.exec(found.text)?.[1];
        if (!main) throw new Error("Standard Ebooks sent a chapter page without its text");
        const html = main.replace(/<a\b[^>]*epub:type="noteref"[^>]*>[\s\S]*?<\/a>/g, "");
        return { html, baseUrl: found.url };
      }
    })
  );
})();
