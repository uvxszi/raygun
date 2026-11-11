// dev.raygun.openstax 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };
  var selectValue = (values, filter) => {
    const chosen = values?.[filter.key];
    const known = (value) => typeof value === "string" && filter.options.some((option) => option.value === value);
    if (known(chosen)) return chosen;
    if (known(filter.default)) return filter.default;
    return filter.options[0]?.value ?? "";
  };

  // src/index.ts
  var SITE = "https://openstax.org";
  var FIELDS = [
    "title",
    "cnx_id",
    "book_state",
    "license_name",
    "license_url",
    "license_version",
    "cover_url",
    "book_subjects",
    "book_categories",
    "publish_date",
    "description",
    "authors",
    "content_warning_text"
  ];
  var CATALOGUE_PAGE = 250;
  var RELEASE = `${SITE}/rex/release.json`;
  var HEADERS = {
    "User-Agent": "Raygun/1.0 ( https://github.com/uvxszi/raygun )",
    Accept: "application/json"
  };
  var PAGE_SIZE = 30;
  var KEEP_HOURS = 6;
  var UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
  var CHAPTER_ID = /^([0-9a-f-]{36}):([0-9a-f-]{36})$/;
  var SECTION_NUMBER = /^\d+(?:\.\d+)+$/;
  var SUBJECTS = [
    "Math",
    "Science",
    "Social Sciences",
    "Humanities",
    "Business",
    "Computer Science",
    "Nursing",
    "College Success"
  ];
  var SUBJECT_NAMES = {
    Matemáticas: "Math",
    Ciencia: "Science",
    Empresarial: "Business"
  };
  var LANGUAGES = {
    en: "English",
    es: "Spanish",
    pl: "Polish",
    fr: "French",
    de: "German"
  };
  var decode = (text) => text.replace(/&nbsp;|&#160;/g, " ").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#0?39;|&apos;/g, "'").replace(
    /&#x([0-9a-f]+);/gi,
    (_match, code) => String.fromCodePoint(parseInt(code, 16))
  ).replace(/&#(\d+);/g, (_match, code) => String.fromCodePoint(Number(code))).replace(/&amp;/g, "&");
  var inline = (html) => decode(html.replace(/<[^>]+>/g, "")).replace(/\s+/g, " ").trim();
  var plainText = (html) => decode(
    html.replace(/<(script|style)\b[\s\S]*?<\/\1>/gi, "").replace(/<\/(?:p|div|li|ul|ol|h\d|blockquote)>|<br\s*\/?>/gi, "\n\n").replace(/<[^>]+>/g, "")
  ).split(/\n\s*\n/).map((paragraph) => paragraph.replace(/\s+/g, " ").trim()).filter(Boolean).join("\n\n");
  var words = (text) => text.normalize("NFKD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  var unique = (names) => [...new Set(names.filter(Boolean))];
  var nameList = (names, most) => names.length <= most ? names.join(", ") : `${names.slice(0, most).join(", ")} and ${names.length - most} more`;
  var isWeb = (url) => typeof url === "string" && /^https?:\/\/\S+$/i.test(url.trim());
  var licenceOf = (url, name, version) => {
    const address = isWeb(url) ? url.trim() : null;
    const creativeCommons = /creativecommons\.org\/licenses\/([a-z-]+)\/(\d+(?:\.\d+)?)/i.exec(
      address ?? ""
    );
    if (creativeCommons) {
      const code = (creativeCommons[1] ?? "").toUpperCase();
      return { name: `CC ${code} ${creativeCommons[2] ?? ""}`.trim(), url: address };
    }
    const named = [name?.trim(), version?.trim()].filter(Boolean).join(" ");
    return named ? { name: named, url: address } : null;
  };
  var getJson = async (url, missing) => {
    const response = await raygun.http.fetch(url, { headers: HEADERS });
    if (response.status === 404) throw new Error(missing);
    if (!response.ok) throw new Error(`OpenStax answered with an error (HTTP ${response.status})`);
    try {
      return await response.json();
    } catch {
      throw new Error("OpenStax's answer couldn't be read");
    }
  };
  var kept = /* @__PURE__ */ new Map();
  var remembered = (key, hours, load) => {
    const known = kept.get(key);
    if (known && Date.now() - known.at < hours * 36e5) return known.value;
    const value = load();
    kept.set(key, { at: Date.now(), value });
    value.catch(() => kept.delete(key));
    return value;
  };
  var people = (book) => {
    const listed = (book.authors ?? []).flatMap((author) => {
      const name = author?.value?.name?.replace(/\s+/g, " ").trim();
      return name ? [
        {
          name,
          senior: Boolean(author.value?.senior_author),
          top: Boolean(author.value?.display_at_top)
        }
      ] : [];
    });
    const top = unique(listed.filter((author) => author.top).map((author) => author.name));
    const senior = unique([
      ...top,
      ...listed.filter((author) => author.senior).map((author) => author.name)
    ]);
    const contributors = unique(listed.map((author) => author.name)).filter(
      (name) => !senior.includes(name)
    );
    const lead = top.length > 0 ? top : senior.length > 0 ? senior : contributors.slice(0, 3);
    return { lead, senior, contributors };
  };
  var toBook = (item) => {
    const id = item.cnx_id?.trim().toLowerCase() ?? "";
    const title = inline(item.title ?? "");
    const slug = item.meta?.slug?.trim() ?? "";
    if (item.book_state !== "live" || !UUID.test(id) || !title || !slug) return [];
    const subjects = unique(
      (item.book_subjects ?? []).map((subject) => inline(subject?.subject_name ?? ""))
    );
    const topics = unique(subjects.map((subject) => SUBJECT_NAMES[subject] ?? subject));
    const categories = unique(
      (item.book_categories ?? []).map((category) => inline(category?.subject_category ?? ""))
    );
    const { lead, senior, contributors } = people(item);
    return [
      {
        id,
        title,
        slug,
        language: item.meta?.locale?.trim().toLowerCase() || "en",
        lead,
        senior,
        contributors,
        subjects,
        topics,
        categories,
        about: plainText(item.description ?? ""),
        warning: plainText(item.content_warning_text ?? ""),
        coverUrl: isWeb(item.cover_url) ? item.cover_url.trim() : null,
        published: /^\d{4}-\d{2}-\d{2}/.exec(item.publish_date ?? "")?.[0] ?? "",
        licence: licenceOf(item.license_url, item.license_name, item.license_version),
        titleWords: ` ${words(title)} `,
        words: ` ${words([title, ...senior, ...contributors, ...subjects, ...topics, ...categories].join(" "))} `
      }
    ];
  };
  var languageRank = (language) => {
    const at = Object.keys(LANGUAGES).indexOf(language);
    return at < 0 ? Object.keys(LANGUAGES).length : at;
  };
  var topicRank = (topic) => {
    const at = SUBJECTS.indexOf(topic);
    return at < 0 ? SUBJECTS.length : at;
  };
  var subjectOf = (book) => [...book.topics].sort((a, b) => topicRank(a) - topicRank(b) || a.localeCompare(b))[0] ?? "";
  var byTitle = (a, b) => words(a.title).localeCompare(words(b.title)) || a.title.localeCompare(b.title);
  var bySubject = (a, b) => {
    const [first, second] = [subjectOf(a), subjectOf(b)];
    return languageRank(a.language) - languageRank(b.language) || topicRank(first) - topicRank(second) || first.localeCompare(second) || byTitle(a, b);
  };
  var byNewest = (a, b) => (b.published || "0").localeCompare(a.published || "0") || byTitle(a, b);
  var catalogue = () => remembered("catalogue", KEEP_HOURS, async () => {
    const items = [];
    for (let offset = 0; offset < CATALOGUE_PAGE * 4; offset += CATALOGUE_PAGE) {
      const answer = await getJson(
        `${SITE}/apps/cms/api/v2/pages/?type=books.Book&fields=${FIELDS.join(",")}&limit=${CATALOGUE_PAGE}${offset ? `&offset=${offset}` : ""}`,
        "OpenStax's catalogue isn't there"
      );
      const page = answer.items ?? [];
      items.push(...page);
      if (page.length < CATALOGUE_PAGE || items.length >= (answer.meta?.total_count ?? 0)) break;
    }
    const books = items.flatMap(toBook).sort(bySubject);
    if (books.length === 0) throw new Error("OpenStax's catalogue lists no books");
    return { books, byId: new Map(books.map((book) => [book.id, book])) };
  });
  var release = () => remembered("release", KEEP_HOURS, async () => {
    const answer = await getJson(
      RELEASE,
      "OpenStax's list of book versions isn't there"
    );
    const archive = answer.archiveUrl?.trim() ?? "";
    if (!/^\/apps\/archive\/[\w.-]+$/.test(archive)) {
      throw new Error("OpenStax's list of book versions couldn't be read");
    }
    const versions = /* @__PURE__ */ new Map();
    for (const [id, book] of Object.entries(answer.books ?? {})) {
      const version = book?.defaultVersion?.trim() ?? "";
      if (UUID.test(id.toLowerCase()) && /^[\w.-]+$/.test(version) && !book?.retired) {
        versions.set(id.toLowerCase(), version);
      }
    }
    return { archive, versions };
  });
  var shelf = async () => {
    const [{ books }, { versions }] = await Promise.all([catalogue(), release()]);
    return books.filter((book) => versions.has(book.id));
  };
  var summary = (book) => ({
    id: book.id,
    title: book.title,
    coverUrl: book.coverUrl,
    authors: book.lead.slice(0, 6)
  });
  var pageOf = (books, page) => {
    const start = (Math.max(1, Math.floor(page) || 1) - 1) * PAGE_SIZE;
    return {
      items: books.slice(start, start + PAGE_SIZE).map(summary),
      hasNextPage: start + PAGE_SIZE < books.length
    };
  };
  var ALL = { value: "all", label: "All" };
  var SORT = {
    kind: "select",
    key: "sort",
    label: "Sort",
    options: [
      { value: "relevance", label: "Best match" },
      { value: "subject", label: "Subject" },
      { value: "newest", label: "Newest" },
      { value: "title", label: "Title" }
    ],
    default: "relevance"
  };
  var filtersFor = (books) => {
    const topics = unique(books.flatMap((book) => book.topics)).sort(
      (a, b) => topicRank(a) - topicRank(b) || a.localeCompare(b)
    );
    const languages = unique(books.map((book) => book.language)).sort(
      (a, b) => languageRank(a) - languageRank(b) || a.localeCompare(b)
    );
    const subject = {
      kind: "select",
      key: "subject",
      label: "Subject",
      options: [ALL, ...topics.map((topic) => ({ value: topic }))],
      default: "all"
    };
    const language = {
      kind: "select",
      key: "language",
      label: "Language",
      options: [
        ALL,
        ...languages.map((code) => ({ value: code, label: LANGUAGES[code] ?? code.toUpperCase() }))
      ],
      default: "all"
    };
    return { subject, language, sort: SORT };
  };
  var searchBooks = async (typed, page, values) => {
    const books = await shelf();
    const filters = filtersFor(books);
    const subject = selectValue(values, filters.subject);
    const language = selectValue(values, filters.language);
    const sort = selectValue(values, filters.sort);
    const wanted = words(typed).split(" ").filter(Boolean);
    const has = (text, word) => text.includes(` ${word}`);
    const found = books.filter(
      (book) => (subject === "all" || book.topics.includes(subject)) && (language === "all" || book.language === language) && wanted.every((word) => has(book.words, word))
    );
    if (sort === "newest") found.sort(byNewest);
    else if (sort === "title") found.sort(byTitle);
    else if (sort === "relevance" && wanted.length > 0) {
      const inTitle = (book) => wanted.every((word) => has(book.titleWords, word));
      found.sort((a, b) => Number(inTitle(b)) - Number(inTitle(a)));
    }
    return pageOf(found, page);
  };
  var contentsKept = /* @__PURE__ */ new Map();
  var contentsOf = async (book) => {
    const { archive, versions } = await release();
    const version = versions.get(book);
    if (!version) throw new Error("OpenStax no longer offers this book");
    const address = `${SITE}${archive}/contents/${book}@${version}.json`;
    let contents = contentsKept.get(address);
    if (!contents) {
      contents = getJson(address, "OpenStax no longer offers this book");
      contents.catch(() => contentsKept.delete(address));
      contentsKept.set(address, contents);
      if (contentsKept.size > 6) contentsKept.delete(contentsKept.keys().next().value ?? address);
    }
    return contents;
  };
  var CLASS = /\bclass\s*=\s*"([^"]*)"/i;
  var spanText = (html, name) => {
    const tags = /<(\/?)span\b([^>]*)>/gi;
    let from = -1;
    let depth = 0;
    for (let tag = tags.exec(html); tag; tag = tags.exec(html)) {
      const [whole, closing, attributes = ""] = tag;
      if (from < 0) {
        if (!closing && (CLASS.exec(attributes)?.[1] ?? "").split(/\s+/).includes(name)) {
          from = tag.index + whole.length;
          depth = 1;
        }
        continue;
      }
      depth += closing ? -1 : 1;
      if (depth === 0) return inline(html.slice(from, tag.index));
    }
    return from < 0 ? "" : inline(html.slice(from));
  };
  var titleOf = (html) => {
    const number = spanText(html, "os-number");
    const text = spanText(html, "os-text");
    return number || text ? { number, text } : { number: "", text: inline(html) };
  };
  var leaves = (tree) => {
    const found = [];
    const walk = (node, context, depth) => {
      if (depth > 8) return;
      for (const child of node.contents ?? []) {
        const { number, text } = titleOf(child.title ?? "");
        if (child.contents?.length) {
          walk(child, number ? [number] : [...context, text].filter(Boolean), depth + 1);
          continue;
        }
        const index = child.toc_target_type === "index" || !child.toc_target_type && /^(?:index|indice)$/i.test(child.slug ?? "");
        const page = (child.id ?? "").split("@")[0]?.trim().toLowerCase() ?? "";
        if (index || !UUID.test(page)) continue;
        const own = number && text ? `${number}${/^[\d.]+$/.test(number) ? " " : ": "}${text}` : number || text;
        if (!own) continue;
        const around = number && context[context.length - 1] === number ? context.slice(0, -1) : context;
        found.push({ page, name: SECTION_NUMBER.test(number) ? own : [...around, own].join(" · ") });
      }
    };
    if (tree) walk(tree, [], 0);
    return found;
  };
  var bookId = (id) => {
    const book = id.trim().toLowerCase();
    if (!UUID.test(book)) throw new Error("OpenStax has no book with this id");
    return book;
  };
  var chapterList = async (id) => {
    const book = bookId(id);
    const pages = leaves((await contentsOf(book)).tree);
    if (pages.length === 0) throw new Error("This book's contents on OpenStax list no pages");
    const seen = /* @__PURE__ */ new Set();
    return pages.filter((leaf) => !seen.has(leaf.page) && seen.add(leaf.page)).map((leaf, index) => ({ id: `${book}:${leaf.page}`, name: leaf.name, number: index + 1 }));
  };
  var details = async (id) => {
    const wanted = bookId(id);
    const book = (await catalogue()).byId.get(wanted);
    if (!book) throw new Error("OpenStax no longer lists this book");
    let licence = book.licence;
    try {
      const contents = await contentsOf(book.id);
      licence = licenceOf(contents.license?.url, contents.license?.name) ?? licence;
    } catch (error) {
      raygun.log.info(`OpenStax's contents of ${book.title} couldn't be read: ${String(error)}`);
    }
    const credits = [
      book.senior.length > 0 && `Senior contributing authors: ${nameList(book.senior, 20)}.`,
      book.contributors.length > 0 && `Contributing authors: ${nameList(book.contributors, 20)}.`
    ].filter(Boolean).join("\n");
    return {
      id: book.id,
      title: book.title,
      coverUrl: book.coverUrl,
      authors: book.senior.length > 0 ? book.senior : book.contributors.slice(0, 12),
      description: [
        book.about,
        book.warning && `Content note: ${book.warning}`,
        credits,
        book.subjects.length > 0 && `Subjects: ${book.subjects.join(", ")}.`,
        licence ? `Licence: ${licence.name} (from OpenStax's book data).` : "Licence: as the book's page on OpenStax states."
      ].filter(Boolean).join("\n\n"),
      genres: unique([
        ...book.subjects,
        ...book.categories.filter((category) => category !== book.title)
      ]).slice(0, 12),
      status: "completed",
      url: `${SITE}/details/books/${encodeURIComponent(book.slug)}`
    };
  };
  var bodyOf = (xhtml) => {
    const body = /<body\b[^>]*>([\s\S]*)<\/body>/i.exec(xhtml);
    return body ? body[1] ?? "" : xhtml.replace(/<head\b[\s\S]*?<\/head>/i, "");
  };
  var passage = async (chapterId) => {
    const parts = CHAPTER_ID.exec(chapterId.trim().toLowerCase());
    const [book = "", page = ""] = parts ? parts.slice(1) : [];
    if (!UUID.test(book) || !UUID.test(page)) throw new Error("OpenStax has no page with this id");
    const { archive, versions } = await release();
    const version = versions.get(book);
    if (!version) throw new Error("OpenStax no longer offers this book");
    const contents = `${SITE}${archive}/contents/`;
    const answer = await getJson(
      `${contents}${book}@${version}:${page}.json`,
      "This page isn't in the book's current version on OpenStax"
    );
    const html = bodyOf(answer.content ?? "");
    if (!inline(html) && !/<img\b/i.test(html)) throw new Error("This page has no text on OpenStax");
    return { html, baseUrl: contents };
  };
  registerProvider(
    defineProvider({
      kind: "novel.source",
      id: "books",
      name: "OpenStax",
      popular: async ({ page }) => pageOf(await shelf(), page),
      latest: async ({ page }) => pageOf([...await shelf()].sort(byNewest), page),
      search: ({ query, page, filters }) => searchBooks(query, page, filters),
      filters: async () => {
        const { subject, language, sort } = filtersFor(await shelf());
        return [subject, language, sort];
      },
      details: ({ id }) => details(id),
      chapters: async ({ id }) => ({ chapters: await chapterList(id) }),
      passage: ({ chapterId }) => passage(chapterId)
    })
  );
})();
