// dev.raygun.gutenberg 1.0.1. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var BOOKS = "https://www.gutenberg.org";
  var MIN_SECTION_TEXT = 120;
  var FEED_PAGE = 25;
  var decode = (text) => text.replace(/&nbsp;/g, " ").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#(\d+);/g, (_match, code) => String.fromCodePoint(Number(code))).replace(/&amp;/g, "&");
  var withoutMarginalia = (html) => html.replace(/<span[^>]*class=["'](?:pagenum|caption)[^"']*["'][^>]*>[\s\S]*?<\/span>/gi, "");
  var plainText = (html) => decode(withoutMarginalia(html).replace(/<[^>]+>/g, " ")).replace(/\{[^}]*\}|\[Pg\s*\w+\]/g, " ").replace(/\s+/g, " ").trim();
  var personName = (name) => {
    const [last, first] = name.split(/,\s*/, 2);
    return first ? `${first} ${last}` : name;
  };
  var cover = (id) => `${BOOKS}/cache/epub/${id}/pg${id}.cover.medium.jpg`;
  var fetchText = async (url) => {
    const response = await raygun.http.fetch(url);
    if (!response.ok) throw new Error(`Project Gutenberg answered HTTP ${response.status}`);
    return { text: await response.text(), url: response.url };
  };
  var entries = (xml) => [...xml.matchAll(/<entry>([\s\S]*?)<\/entry>/g)].map((match) => match[1] ?? "");
  var AUDIO_ICON = "ZHlxyWU8AAAAAElFTkSuQmCC";
  var feed = async (params, page) => {
    const start = (page - 1) * FEED_PAGE + 1;
    const { text } = await fetchText(`${BOOKS}/ebooks/search.opds/?${params}&start_index=${start}`);
    const all = entries(text);
    const items = all.flatMap((entry) => {
      if (entry.includes(AUDIO_ICON)) return [];
      const id = /<id>[^<]*\/ebooks\/(\d+)\.opds<\/id>/.exec(entry)?.[1];
      const title = /<title>([\s\S]*?)<\/title>/.exec(entry)?.[1];
      return id && title ? [{ id, title: decode(title.trim()), coverUrl: cover(id) }] : [];
    });
    return { items, hasNextPage: all.length >= FEED_PAGE };
  };
  var body = (html) => {
    const start = /\*\*\*\s*START OF (?:THE|THIS) PROJECT GUTENBERG[^*]*\*\*\*/i.exec(html);
    const end = /\*\*\*\s*END OF (?:THE|THIS) PROJECT GUTENBERG/i.exec(html);
    const from = start ? start.index + start[0].length : html.search(/<body[\s>]/i);
    const to = end ? end.index : html.length;
    return html.slice(Math.max(from, 0), to > from ? to : html.length);
  };
  var splitBook = (html) => {
    const text = body(html);
    for (const level of ["h2", "h3"]) {
      const starts = [...text.matchAll(new RegExp(`<${level}[\\s>]`, "gi"))].map(
        (match) => match.index ?? 0
      );
      if (starts.length < 2) continue;
      const sections = [];
      starts.forEach((start, index) => {
        const sectionHtml = text.slice(start, starts[index + 1] ?? text.length);
        const close = sectionHtml.search(new RegExp(`</${level}>`, "i"));
        const heading = close > 0 ? sectionHtml.slice(0, close) : "";
        const name = plainText(heading) || decode(/alt="([^"]+)"/.exec(heading)?.[1] ?? "").trim();
        const contentLength = sectionHtml.length - Math.max(close, 0);
        if (/^(table of )?contents\.?$/i.test(name) || contentLength < MIN_SECTION_TEXT) return;
        sections.push({ name: name || `Section ${sections.length + 1}`, html: sectionHtml });
      });
      if (sections.length > 0) return sections;
    }
    return [{ name: "Full text", html: text }];
  };
  var lastBook = null;
  var book = async (id) => {
    if (!/^\d+$/.test(id)) throw new Error("Unknown book");
    if (lastBook?.id === id) return lastBook;
    const { text, url } = await fetchText(`${BOOKS}/ebooks/${id}.html.images`);
    lastBook = { id, url, sections: splitBook(text) };
    return lastBook;
  };
  registerProvider(
    defineProvider({
      kind: "novel.source",
      id: "gutenberg",
      name: "Project Gutenberg",
      popular: ({ page }) => feed("sort_order=downloads", page),
      latest: ({ page }) => feed("sort_order=release_date", page),
      search: ({ query, page }) => feed(`query=${encodeURIComponent(query)}`, page),
      details: async ({ id }) => {
        if (!/^\d+$/.test(id)) throw new Error("Unknown book");
        const { text } = await fetchText(`${BOOKS}/ebooks/${id}.opds`);
        const entry = entries(text)[0] ?? "";
        const title = /<title>([\s\S]*?)<\/title>/.exec(entry)?.[1];
        if (!title) throw new Error("This book has no catalogue entry");
        const authors = [...entry.matchAll(/<author>\s*<name>([\s\S]*?)<\/name>/g)].map(
          (match) => personName(decode((match[1] ?? "").trim()))
        );
        const summary = /Summary:\s*([\s\S]*?)<\/p>/.exec(entry)?.[1];
        const subjects = [...entry.matchAll(/<category scheme="[^"]*LCSH" term="([^"]*)"/g)].map((match) => (decode(match[1] ?? "").split(" -- ")[0] ?? "").trim()).filter((subject, index, all) => subject && all.indexOf(subject) === index);
        return {
          id,
          title: decode(title.trim()),
          coverUrl: cover(id),
          authors,
          description: summary ? plainText(summary).replace(/\s*\(This is an automatically generated summary\.\)$/, "") : null,
          genres: subjects.slice(0, 8),
          status: "completed",
          url: `${BOOKS}/ebooks/${id}`
        };
      },
      chapters: async ({ id }) => {
        const { sections } = await book(id);
        const chapters = sections.map((section, index) => ({
          id: `${id}/${index}`,
          name: section.name,
          number: index + 1
        }));
        return { chapters };
      },
      passage: async ({ chapterId }) => {
        const [id = "", index = ""] = chapterId.split("/");
        const { sections, url } = await book(id);
        const section = sections[Number(index)];
        if (!section) throw new Error("This chapter is no longer in the book");
        return { html: withoutMarginalia(section.html), baseUrl: url };
      }
    })
  );
})();
