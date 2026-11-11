// dev.raygun.bccampus 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var SITE = "https://opentextbc.ca";
  var PER_REQUEST = 10;
  var AT_ONCE = 5;
  var PAGE_SIZE = 30;
  var HEADERS = {
    Accept: "application/json",
    "User-Agent": "Raygun/1.0 ( https://github.com/uvxszi/raygun )"
  };
  var SLUG = /^[a-z0-9][a-z0-9-]*$/;
  var KINDS = /* @__PURE__ */ new Set(["front-matter", "parts", "chapters", "back-matter"]);
  var BOILERPLATE = /^(?:accessibility statement|about bccampus open education|for students: how to access and use this textbook)$/i;
  var SECURE_HOSTS = /^http:\/\/((?:[\w-]+\.)*(?:opentextbc\.ca|bccampus\.ca|wikimedia\.org))\//i;
  var SUBJECTS = [
    { value: "all", label: "All" },
    { value: "A", label: "Arts" },
    { value: "C", label: "Language and linguistics" },
    { value: "D", label: "Literature and biography" },
    { value: "G", label: "Reference and interdisciplinary" },
    { value: "J", label: "Society and social sciences" },
    { value: "K", label: "Economics, finance and business" },
    { value: "L", label: "Law" },
    { value: "M", label: "Medicine and nursing" },
    { value: "N", label: "History and archaeology" },
    { value: "P", label: "Mathematics and science" },
    { value: "Q", label: "Philosophy and religion" },
    { value: "R", label: "Earth sciences and geography" },
    { value: "T", label: "Technology, engineering and trades" },
    { value: "U", label: "Computing" },
    { value: "V", label: "Health and personal development" },
    { value: "W", label: "Lifestyle, cooking and leisure" },
    { value: "Y", label: "Education and learning" }
  ];
  var LICENCES = [
    { value: "all", label: "All" },
    { value: "CC BY", label: "CC BY" },
    { value: "CC BY-SA", label: "CC BY-SA" },
    { value: "CC BY-NC", label: "CC BY-NC" },
    { value: "CC BY-NC-SA", label: "CC BY-NC-SA" },
    { value: "Public domain", label: "Public domain" }
  ];
  var SORTS = [
    { value: "relevance", label: "Best match" },
    { value: "catalogue", label: "Catalogue order" },
    { value: "title", label: "Title (A–Z)" },
    { value: "newest", label: "Newest" },
    { value: "updated", label: "Recently updated" }
  ];
  var textOf = (value) => typeof value === "string" ? value : "";
  var decode = (text) => textOf(text).replace(/&nbsp;|&#160;/g, " ").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#0?39;|&apos;/g, "'").replace(
    /&#x([0-9a-f]+);/gi,
    (_match, code) => String.fromCodePoint(parseInt(code, 16))
  ).replace(/&#(\d+);/g, (_match, code) => String.fromCodePoint(Number(code))).replace(/&amp;/g, "&");
  var plainText = (html) => decode(textOf(html).replace(/<[^>]+>/g, " ")).replace(/\s+/g, " ").trim();
  var words = (text) => text.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  var getJson = async (path) => {
    let response;
    try {
      response = await raygun.http.fetch(`${SITE}${path}`, { headers: HEADERS });
    } catch {
      throw new Error("BCcampus didn't answer; try again in a minute");
    }
    if (response.status === 404) throw new Error("BCcampus no longer has this book or chapter");
    if (!response.ok) throw new Error(`BCcampus answered with an error (HTTP ${response.status})`);
    return { value: await response.json(), headers: response.headers };
  };
  var openLicence = (licence) => {
    const url = licence?.url ?? "";
    const creativeCommons = /creativecommons\.org\/licenses\/([a-z-]+)\/(\d+(?:\.\d+)?)/i.exec(url);
    if (creativeCommons) {
      return `CC ${(creativeCommons[1] ?? "").toUpperCase()} ${creativeCommons[2] ?? ""}`;
    }
    if (/creativecommons\.org\/publicdomain\/zero\//i.test(url)) return "CC0 1.0";
    if (/creativecommons\.org\/publicdomain\/mark\//i.test(url)) return "Public domain";
    const named = `${licence?.code ?? ""} ${licence?.name ?? ""}`;
    if (/public domain/i.test(named)) return "Public domain";
    if (/\bCC0\b/i.test(named)) return "CC0";
    const code = /\bCC (BY(?:-NC)?(?:-SA|-ND)?)\b/i.exec(named);
    return code ? `CC ${(code[1] ?? "").toUpperCase()}` : null;
  };
  var licenceFamily = (licence) => licence.replace(/ \d.*$/, "");
  var names = (people) => (people ?? []).map((person) => plainText(person.name)).filter(Boolean);
  var secure = (url) => url.replace(SECURE_HOSTS, "https://$1/");
  var toBook = (entry, order) => {
    const meta = entry.metadata;
    const id = /^https?:\/\/opentextbc\.ca\/([a-z0-9-]+)\/?$/.exec(entry.link ?? "")?.[1];
    if (!meta || !id || meta.inCatalog !== true || !meta.name) return null;
    const licence = openLicence(meta.license);
    if (!licence) return null;
    const people = names(meta.author);
    const authors = people.length > 0 ? people : names(meta.editor);
    const about = meta.about ?? [];
    const subjects = new Set(
      about.filter(
        (subject) => decode(subject.name) && /^[A-Z][A-Z0-9]*$/.test(decode(subject.identifier))
      ).map((subject) => decode(subject.identifier).charAt(0))
    );
    const topics = about.map((subject) => decode(subject.name) || decode(subject.identifier));
    const cover = textOf(meta.thumbnailUrl) || textOf(meta.image);
    const updated = Number(meta.lastUpdated) * 1e3;
    const published = Date.parse(textOf(meta.datePublished));
    return {
      id,
      title: plainText(meta.name),
      authors,
      cover: cover ? secure(cover) : null,
      licence,
      subjects,
      text: {
        title: words(decode(meta.name)),
        people: words([...authors, ...names(meta.translator)].join(" ")),
        topics: words([...topics, decode(meta.keywords)].join(" ")),
        summary: words(decode(meta.disambiguatingDescription))
      },
      published: Number.isFinite(published) ? published : Number.isFinite(updated) ? updated : 0,
      updated: Number.isFinite(updated) ? updated : 0,
      order
    };
  };
  var inTurns = async (items, width, task) => {
    const results = [];
    for (let start = 0; start < items.length; start += width) {
      results.push(...await Promise.all(items.slice(start, start + width).map(task)));
    }
    return results;
  };
  var listPage = (page) => getJson(
    `/wp-json/pressbooks/v2/books?per_page=${PER_REQUEST}&page=${page}&_fields=link,metadata`
  );
  var catalogueAt = 0;
  var catalogueBooks = null;
  var catalogue = () => {
    if (catalogueBooks && Date.now() - catalogueAt < 12 * 36e5) return catalogueBooks;
    catalogueAt = Date.now();
    const loading = (async () => {
      const first = await listPage(1);
      const pages = Math.min(Number(first.headers["x-wp-totalpages"]) || 1, 100);
      const rest = await inTurns(
        Array.from({ length: pages - 1 }, (_value, index) => index + 2),
        AT_ONCE,
        async (page) => (await listPage(page)).value
      );
      const entries = [first.value, ...rest].flat();
      const seen = /* @__PURE__ */ new Set();
      return entries.flatMap((entry, order) => {
        const book = toBook(entry, order);
        if (!book || seen.has(book.id)) return [];
        seen.add(book.id);
        return [book];
      });
    })();
    loading.catch(() => {
      if (catalogueBooks === loading) catalogueBooks = null;
    });
    catalogueBooks = loading;
    return loading;
  };
  var summary = (book) => ({
    id: book.id,
    title: book.title,
    coverUrl: book.cover,
    authors: book.authors.slice(0, 6)
  });
  var pageOf = (books, page) => {
    const start = (Math.max(1, page) - 1) * PAGE_SIZE;
    return {
      items: books.slice(start, start + PAGE_SIZE).map(summary),
      hasNextPage: start + PAGE_SIZE < books.length
    };
  };
  var byCatalogue = (first, second) => first.order - second.order;
  var byNewest = (first, second) => second.published - first.published || byCatalogue(first, second);
  var ORDERS = /* @__PURE__ */ new Map([
    ["catalogue", byCatalogue],
    ["title", (first, second) => first.title.localeCompare(second.title)],
    ["newest", byNewest],
    ["updated", (first, second) => second.updated - first.updated || byCatalogue(first, second)]
  ]);
  var score = (book, terms, phrase) => {
    let total = book.text.title.includes(phrase) ? 4 : 0;
    for (const word of terms) {
      const found = (book.text.title.includes(word) ? 3 : 0) + (book.text.people.includes(word) ? 2 : 0) + (book.text.topics.includes(word) ? 1.5 : 0) + (book.text.summary.includes(word) ? 1 : 0);
      if (found === 0) return 0;
      total += found;
    }
    return total;
  };
  var pick = (filters, key, options) => {
    const value = filters?.[key];
    return typeof value === "string" && options.some((option) => option.value === value) ? value : null;
  };
  var search = async (query, page, values) => {
    const subject = pick(values, "subject", SUBJECTS);
    const licence = pick(values, "licence", LICENCES);
    const terms = words(query).split(" ").filter(Boolean);
    const phrase = terms.join(" ");
    const sort = pick(values, "sort", SORTS) ?? (phrase ? "relevance" : "catalogue");
    const order = ORDERS.get(sort);
    const books = (await catalogue()).filter(
      (book) => (!subject || subject === "all" || book.subjects.has(subject)) && (!licence || licence === "all" || licenceFamily(book.licence) === licence)
    );
    if (!phrase) return pageOf([...books].sort(order ?? byCatalogue), page);
    const found = books.map((book) => ({ book, score: score(book, terms, phrase) })).filter((match) => match.score > 0).sort(
      (first, second) => order ? order(first.book, second.book) : second.score - first.score || byCatalogue(first.book, second.book)
    );
    return pageOf(
      found.map((match) => match.book),
      page
    );
  };
  var bookId = (id) => {
    if (!SLUG.test(id)) throw new Error("Not a BCcampus book");
    return id;
  };
  var details = async (id) => {
    const { value: meta } = await getJson(
      `/${bookId(id)}/wp-json/pressbooks/v2/metadata`
    );
    if (!meta.name) throw new Error("BCcampus sent a book without a title");
    const licence = openLicence(meta.license);
    if (!licence) throw new Error("This book is not under an open licence");
    const people = names(meta.author);
    const translators = names(meta.translator);
    const year = textOf(meta.datePublished).slice(0, 4) || textOf(meta.copyrightYear);
    const publisher = plainText(meta.publisher?.name);
    const genres = (meta.about ?? []).map((subject) => decode(subject.name)).filter(Boolean);
    const cover = textOf(meta.thumbnailUrl) || textOf(meta.image);
    return {
      id,
      title: plainText(meta.name),
      coverUrl: cover ? secure(cover) : null,
      authors: people.length > 0 ? people : names(meta.editor),
      description: [
        plainText(meta.disambiguatingDescription),
        [
          translators.length > 0 && `Translated by ${translators.join(", ")}.`,
          publisher && `Published by ${publisher}${year ? ` in ${year}` : ""}.`
        ].filter(Boolean).join(" "),
        `Licence: ${licence}, as stated for this book on opentextbc.ca.`
      ].filter(Boolean).join("\n\n"),
      genres: [...new Set(genres)].slice(0, 8),
      status: "completed",
      url: `${SITE}/${id}/`
    };
  };
  var chapterList = async (id) => {
    const { value: toc } = await getJson(`/${bookId(id)}/wp-json/pressbooks/v2/toc`);
    const parts = toc.parts ?? [];
    const chapters = [];
    const add = (kind, entry, within) => {
      const title = plainText(entry.title ?? "");
      if (!entry.id || !entry.has_post_content || BOILERPLATE.test(title)) return;
      if (entry.metadata?.license && !openLicence(entry.metadata.license)) return;
      chapters.push({
        id: `${id}/${kind}/${entry.id}`,
        name: within && within !== title ? `${within} · ${title}` : title || `${kind} ${entry.id}`,
        number: chapters.length + 1
      });
    };
    for (const entry of toc["front-matter"] ?? []) add("front-matter", entry);
    for (const part of parts) {
      const partTitle = plainText(part.title ?? "");
      add("parts", part);
      for (const chapter of part.chapters ?? []) {
        add("chapters", chapter, parts.length > 1 ? partTitle : void 0);
      }
    }
    for (const entry of toc["back-matter"] ?? []) add("back-matter", entry);
    if (chapters.length === 0) throw new Error("BCcampus lists no chapters for this book");
    return chapters;
  };
  var SHORTCODE = String.raw`\[latex\]((?:(?!\[\/?latex\])[\s\S])*?)\[\/latex\]`;
  var DOLLAR = String.raw`\$latex\s([^$]*?)\$`;
  var latex = (html) => html.replace(
    new RegExp(String.raw`<p\b([^>]*)>\s*(?:${SHORTCODE}|${DOLLAR})\s*<\/p>`, "g"),
    (_match, attributes, shortcode, dollar) => `<p${attributes}>\\[${shortcode ?? dollar ?? ""}\\]</p>`
  ).replace(new RegExp(SHORTCODE, "g"), "\\($1\\)").replace(new RegExp(DOLLAR, "g"), "\\($1\\)");
  var activityNote = (wrapper) => {
    const title = /<iframe\b[^>]*\btitle="([^"]*)"/.exec(wrapper)?.[1];
    const what = title ? `“${plainText(title)}” is an interactive activity` : "An interactive activity";
    return `<p><em>${what} in the online version of this chapter.</em></p>`;
  };
  var cleanChapter = (html) => latex(html).replace(/<div class="h5p-iframe-wrapper">[\s\S]*?<\/iframe>\s*<\/div>/g, activityNote).replace(/\[h5p\b[^\]]*\]/g, "").replace(
    /<button\b[^>]*\bclass="[^"]*glossary-term[^"]*"[^>]*>([\s\S]*?)<\/button>/g,
    "<span>$1</span>"
  ).replace(
    /(<img\b[^>]*?\bsrc=")([^"]+)"/g,
    (_match, start, src) => `${start}${secure(src)}"`
  );
  registerProvider(
    defineProvider({
      kind: "novel.source",
      id: "textbooks",
      name: "BCcampus Open Textbooks",
      popular: async ({ page }) => pageOf(await catalogue(), page),
      latest: async ({ page }) => pageOf([...await catalogue()].sort(byNewest), page),
      search: ({ query, page, filters: values }) => search(query, page, values),
      filters: async () => [
        { kind: "select", key: "subject", label: "Subject", options: SUBJECTS },
        { kind: "select", key: "licence", label: "Licence", options: LICENCES },
        { kind: "select", key: "sort", label: "Sort", options: SORTS }
      ],
      details: ({ id }) => details(id),
      chapters: async ({ id }) => ({ chapters: await chapterList(id) }),
      passage: async ({ chapterId }) => {
        const [book = "", kind = "", post = ""] = chapterId.split("/");
        if (!SLUG.test(book) || !KINDS.has(kind) || !/^\d+$/.test(post)) {
          throw new Error("Not a BCcampus chapter");
        }
        const { value } = await getJson(
          `/${book}/wp-json/pressbooks/v2/${kind}/${post}?_fields=link,content`
        );
        const html = value.content?.rendered ?? "";
        if (!plainText(html) && !/<img\b/.test(html)) {
          throw new Error("This chapter has no text");
        }
        return { html: cleanChapter(html), baseUrl: value.link ?? `${SITE}/${book}/` };
      }
    })
  );
})();
