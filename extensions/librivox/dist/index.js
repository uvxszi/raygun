// dev.raygun.librivox 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
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
  var SEARCH = "https://archive.org/advancedsearch.php";
  var PAGE_SIZE = 30;
  var first = (value) => (Array.isArray(value) ? value[0] : value)?.trim() || null;
  var escape = (text) => text.replace(/[+\-!(){}[\]^"~*?:/\\&|]/g, " ").trim();
  var plain = (html) => html ? html.replace(/<[^>]+>/g, " ").replace(/&amp;/g, "&").replace(/&quot;/g, '"').replace(/&#39;|&rsquo;/g, "'").replace(/\s+/g, " ").trim() : null;
  var toBook = (doc) => {
    const number = first(doc.call_number);
    const title = first(doc.title);
    if (!number || !/^\d+$/.test(number) || !title) return null;
    return {
      id: doc.identifier,
      title,
      author: first(doc.creator),
      artworkUrl: `https://archive.org/services/img/${encodeURIComponent(doc.identifier)}`,
      feedUrl: `https://librivox.org/rss/${number}`,
      runtime: doc.runtime ?? null,
      language: first(doc.language),
      description: plain(first(doc.description))?.slice(0, 600) ?? null
    };
  };
  var find = async (query, sort, page) => {
    const params = [
      `q=${encodeURIComponent(`collection:librivoxaudio AND ${query}`)}`,
      ...["identifier", "title", "creator", "call_number", "runtime", "language", "description"].map(
        (field) => `fl[]=${field}`
      ),
      `sort[]=${encodeURIComponent(sort)}`,
      `rows=${PAGE_SIZE}`,
      `page=${Math.max(1, page)}`,
      "output=json"
    ];
    const answer = await fetchJson(`${SEARCH}?${params.join("&")}`);
    const docs = answer.response?.docs ?? [];
    const found = answer.response?.numFound ?? 0;
    return {
      books: docs.map(toBook).filter((book) => book !== null),
      hasMore: Math.max(1, page) * PAGE_SIZE < found
    };
  };
  registerProvider(
    defineProvider({
      kind: "audiobook.directory",
      id: "books",
      name: "LibriVox",
      async search({ query, page }) {
        const words = escape(query);
        if (!words) return find("mediatype:audio", "downloads desc", page);
        return find(`(${words})`, "downloads desc", page);
      },
      async popular({ page }) {
        return find("mediatype:audio", "downloads desc", page);
      },
      async latest({ page }) {
        return find("mediatype:audio", "publicdate desc", page);
      },
      async subject({ subject, page }) {
        const words = escape(subject);
        if (!words) return { books: [], hasMore: false };
        return find(`subject:(${words})`, "downloads desc", page);
      }
    })
  );
})();
