// dev.raygun.loyalbooks 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var SITE = "https://www.loyalbooks.com";
  var SEARCHED_PAGES = 5;
  var PAGE_SIZE = 30;
  var decode = (text) => text.replace(/<[^>]+>/g, " ").replace(/&amp;/g, "&").replace(/&quot;/g, '"').replace(/&#39;|&rsquo;|&#8217;/g, "'").replace(/&nbsp;/g, " ").replace(/\s+/g, " ").trim();
  var readListing = (html) => {
    const books = [];
    const seen = /* @__PURE__ */ new Set();
    const cells = html.split(/<td class="layout2-blue"/).slice(1);
    for (const cell of cells) {
      const slug = /href="\/book\/([^"/?#]+)"/.exec(cell)?.[1];
      const title = /<b>([\s\S]*?)<\/b>/.exec(cell)?.[1];
      if (!slug || !title || seen.has(slug)) continue;
      seen.add(slug);
      const author = /<\/b><\/a><br>([^<]*)/.exec(cell)?.[1];
      const image = /<img[^>]+src="([^"]+)"/.exec(cell)?.[1];
      books.push({
        id: slug,
        title: decode(title),
        author: author ? decode(author) || null : null,
        feedUrl: `${SITE}/book/${slug}/feed`,
        artworkUrl: !image ? null : image.startsWith("http") ? image : `${SITE}/${image.replace(/^\/+/, "")}`,
        runtime: null,
        language: null,
        description: null
      });
    }
    return books;
  };
  var pageCount = (html) => Number(/Page \d+ of (\d+)/.exec(html)?.[1] ?? "1");
  var listing = async (path, page) => {
    const at = Math.max(1, page);
    const response = await raygun.http.fetch(`${SITE}${path}${at > 1 ? `/${at}` : ""}`);
    if (!response.ok) throw new Error(`HTTP ${response.status} from Loyal Books`);
    const html = await response.text();
    return { books: readListing(html), hasMore: at < pageCount(html) };
  };
  var popularBooks = null;
  var searchable = () => {
    popularBooks ??= Promise.all(
      Array.from(
        { length: SEARCHED_PAGES },
        (_, index) => listing("/Top_100", index + 1).then(
          (found) => found.books,
          () => []
        )
      )
    ).then((pages) => pages.flat());
    return popularBooks;
  };
  var simple = (text) => text.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  registerProvider(
    defineProvider({
      kind: "audiobook.directory",
      id: "books",
      name: "Loyal Books",
      async search({ query, page }) {
        const words = simple(query).split(" ").filter(Boolean);
        if (words.length === 0) return listing("/Top_100", page);
        const matches = (await searchable()).filter((book) => {
          const text = simple(`${book.title} ${book.author ?? ""}`);
          return words.every((word) => text.includes(word));
        });
        const start = (Math.max(1, page) - 1) * PAGE_SIZE;
        return {
          books: matches.slice(start, start + PAGE_SIZE),
          hasMore: start + PAGE_SIZE < matches.length
        };
      },
      async popular({ page }) {
        return listing("/Top_100", page);
      },
      async subject({ subject, page }) {
        const name = subject.trim().replace(/\s+/g, "_");
        if (!name) return { books: [], hasMore: false };
        return listing(`/genre/${encodeURIComponent(name)}`, page);
      }
    })
  );
})();
