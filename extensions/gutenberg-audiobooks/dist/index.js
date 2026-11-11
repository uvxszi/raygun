// dev.raygun.gutenberg-audiobooks 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
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
  var query = (params) => Object.entries(params).flatMap(([name, value]) => {
    if (value === void 0 || value === null) return [];
    const values = Array.isArray(value) ? value : [value];
    return values.map((item) => `${encodeURIComponent(name)}=${encodeURIComponent(item)}`);
  }).join("&");

  // src/index.ts
  var API = "https://archive.org";
  var COLLECTION = "collection:gutenberg-audiobooks AND mediatype:audio";
  var PAGE_SIZE = 30;
  var first = (value) => (Array.isArray(value) ? value[0] : value)?.trim() || null;
  var escape = (text) => text.replace(/[+\-!(){}[\]^"~*?:/\\&|]/g, " ").trim();
  var titleOf = (value) => first(value)?.split("\n")[0]?.trim() || null;
  var authorOf = (value) => {
    const names = (Array.isArray(value) ? value : value ? [value] : []).map((name) => {
      const parts = name.split(",").map((part) => part.trim());
      const [surname, given] = parts;
      if (!surname) return "";
      if (!given || /^[\d?\s-]+(BCE?|AD)?$/i.test(given)) return surname;
      return `${given.replace(/\s*\([^)]*\)/g, "")} ${surname}`.trim();
    }).filter(Boolean);
    return names.length > 0 ? names.join(", ") : null;
  };
  var parseLength = (length) => {
    if (!length) return null;
    const seconds = length.split(":").reduce((total, part) => total * 60 + Number.parseFloat(part), 0);
    return Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds * 1e3) : null;
  };
  var artworkOf = (identifier) => `${API}/services/img/${encodeURIComponent(identifier)}`;
  var toBook = (doc) => {
    const title = titleOf(doc.title);
    if (!title) return null;
    return {
      id: doc.identifier,
      title,
      author: authorOf(doc.creator),
      artworkUrl: artworkOf(doc.identifier),
      language: first(doc.language)
    };
  };
  var find = async (q, sort, page) => {
    const params = query({
      q: `${COLLECTION} AND ${q}`,
      "fl[]": ["identifier", "title", "creator", "language"],
      "sort[]": sort,
      rows: PAGE_SIZE,
      page: Math.max(1, page),
      output: "json"
    });
    const answer = await fetchJson(`${API}/advancedsearch.php?${params}`);
    const found = answer.response?.numFound ?? 0;
    return {
      books: (answer.response?.docs ?? []).map(toBook).filter((book) => book !== null),
      hasMore: Math.max(1, page) * PAGE_SIZE < found,
      found
    };
  };
  var encodePath = (path) => path.split("/").map(encodeURIComponent).join("/");
  registerProvider(
    defineProvider({
      kind: "audiobook.directory",
      id: "books",
      name: "Gutenberg open audiobooks",
      async search({ query: text, page }) {
        const words = escape(text);
        if (!words) return find("identifier:*", "downloads desc", page);
        const found = await find(`(title:(${words}) OR creator:(${words}))`, "downloads desc", page);
        return found.found === 0 && page <= 1 ? find(`(${words})`, "downloads desc", page) : found;
      },
      async popular({ page }) {
        return find("identifier:*", "downloads desc", page);
      },
      async chapters({ id }) {
        const item = await fetchJson(`${API}/metadata/${encodeURIComponent(id)}`);
        const collections = [item.metadata?.collection ?? []].flat();
        if (!collections.includes("gutenberg-audiobooks")) {
          throw new Error("This book is no longer in the Project Gutenberg audiobook collection");
        }
        const title = titleOf(item.metadata?.title) ?? id;
        const files = (item.files ?? []).filter((file) => file.format === "VBR MP3" || file.format === "MP3").sort((a, b) => a.name.localeCompare(b.name));
        const chapters = files.map((file, index) => ({
          id: file.name,
          title: files.length === 1 ? title : file.title?.trim() || `Part ${index + 1}`,
          audioUrl: `${API}/download/${encodeURIComponent(id)}/${encodePath(file.name)}`,
          audioType: "audio/mpeg",
          durationMs: parseLength(file.length)
        }));
        const source = item.metadata?.source;
        return {
          title,
          author: authorOf(item.metadata?.creator),
          artworkUrl: artworkOf(id),
          description: source ? `Read by a neural voice from Project Gutenberg's text (${source}).` : "Read by a neural voice from Project Gutenberg's text.",
          link: `${API}/details/${encodeURIComponent(id)}`,
          chapters
        };
      }
    })
  );
})();
