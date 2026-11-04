// dev.raygun.librivox 1.1.0. Built by raygun-ext from src/index.ts; do not edit.
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
  var API = "https://librivox.org/api/feed/audiobooks/";
  var PAGE_SIZE = 30;
  var API_LIMIT = 20;
  var first = (value) => (Array.isArray(value) ? value[0] : value)?.trim() || null;
  var escape = (text) => text.replace(/[+\-!(){}[\]^"~*?:/\\&|]/g, " ").trim();
  var plain = (html) => html ? html.replace(/<[^>]+>/g, " ").replace(/&amp;/g, "&").replace(/&quot;/g, '"').replace(/&#39;|&rsquo;/g, "'").replace(/\s+/g, " ").trim() : null;
  var feedOf = (number) => `https://librivox.org/rss/${number}`;
  var toBook = (doc) => {
    const number = first(doc.call_number);
    const title = first(doc.title);
    if (!number || !/^\d+$/.test(number) || !title) return null;
    return {
      id: doc.identifier,
      title,
      author: first(doc.creator),
      artworkUrl: `https://archive.org/services/img/${encodeURIComponent(doc.identifier)}`,
      feedUrl: feedOf(number),
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
      hasMore: Math.max(1, page) * PAGE_SIZE < found,
      found
    };
  };
  var fromApi = (book) => {
    if (!book.id || !/^\d+$/.test(book.id) || !book.title?.trim()) return null;
    const authors = (book.authors ?? []).map((author) => `${author.first_name ?? ""} ${author.last_name ?? ""}`.trim()).filter(Boolean);
    return {
      id: `librivox-${book.id}`,
      title: book.title.trim(),
      author: authors.join(", ") || null,
      artworkUrl: book.coverart_thumbnail || book.coverart_jpg || null,
      feedUrl: feedOf(book.id),
      runtime: book.totaltime ?? null,
      language: book.language ?? null,
      description: plain(book.description ?? null)?.slice(0, 600) ?? null
    };
  };
  var api = async (params, limit, offset = 0) => {
    const query = Object.entries({
      ...params,
      format: "json",
      coverart: "1",
      limit: String(limit),
      offset: String(offset),
      fields: "{id,title,description,language,totaltime,authors,coverart_thumbnail,coverart_jpg}"
    }).map(([key, value]) => `${key}=${encodeURIComponent(value)}`).join("&");
    const response = await raygun.http.fetch(`${API}?${query}`);
    if (response.status === 404) return [];
    if (!response.ok) throw new Error(`HTTP ${response.status} from LibriVox`);
    const answer = await response.json();
    return (answer.books ?? []).map(fromApi).filter((book) => book !== null);
  };
  var withoutArticle = (text) => text.replace(/^(the|a|an)\s+/i, "");
  var apiSearch = async (query) => {
    const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
    const surname = words[words.length - 1] ?? "";
    const [byTitle, byAuthor] = await Promise.allSettled([
      api({ title: `^${withoutArticle(query.trim())}` }, API_LIMIT),
      surname.length > 2 ? api({ author: surname }, API_LIMIT) : Promise.resolve([])
    ]);
    if (byTitle.status === "rejected" && byAuthor.status === "rejected") throw byTitle.reason;
    const authored = (byAuthor.status === "fulfilled" ? byAuthor.value : []).filter((book) => {
      const text = `${book.author ?? ""} ${book.title}`.toLowerCase();
      return words.every((word) => text.includes(word));
    });
    return [...byTitle.status === "fulfilled" ? byTitle.value : [], ...authored];
  };
  var join = (pages) => {
    const seen = /* @__PURE__ */ new Set();
    return pages.flat().filter((book) => {
      const key = book.feedUrl ?? book.id;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  };
  var GENRES = {
    adventure: "Action & Adventure Fiction",
    children: "Children's Fiction",
    "fairy tales": "Myths, Legends & Fairy Tales",
    fantasy: "Fantasy Fiction",
    "historical fiction": "Historical Fiction",
    history: "History",
    humor: "Humorous Fiction",
    mystery: "Crime & Mystery Fiction",
    literature: "Literary Fiction",
    philosophy: "Philosophy",
    poetry: "Poetry",
    religion: "Religion",
    romance: "Romance",
    "science fiction": "Science Fiction",
    "short stories": "Short Stories",
    drama: "Dramatic Readings",
    horror: "Horror & Supernatural Fiction"
  };
  registerProvider(
    defineProvider({
      kind: "audiobook.directory",
      id: "books",
      name: "LibriVox",
      async search({ query, page }) {
        const words = escape(query);
        if (!words) return find("mediatype:audio", "downloads desc", page);
        const archive = find(`(title:(${words}) OR creator:(${words}))`, "downloads desc", page).then(
          (found) => found.found === 0 ? find(`(${words})`, "downloads desc", page) : found
        );
        const catalogue = page === 1 ? apiSearch(query) : Promise.resolve([]);
        const [fromArchive, fromCatalogue] = await Promise.allSettled([archive, catalogue]);
        if (fromArchive.status === "rejected" && fromCatalogue.status === "rejected") {
          throw fromArchive.reason;
        }
        const books = join([
          fromArchive.status === "fulfilled" ? fromArchive.value.books : [],
          fromCatalogue.status === "fulfilled" ? fromCatalogue.value : []
        ]);
        return {
          books,
          hasMore: fromArchive.status === "fulfilled" && fromArchive.value.hasMore
        };
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
        const genre = GENRES[subject.trim().toLowerCase()];
        const [fromArchive, fromCatalogue] = await Promise.allSettled([
          find(`subject:(${words})`, "downloads desc", page),
          genre ? api({ genre }, PAGE_SIZE, (Math.max(1, page) - 1) * PAGE_SIZE) : Promise.resolve([])
        ]);
        if (fromArchive.status === "rejected" && fromCatalogue.status === "rejected") {
          throw fromArchive.reason;
        }
        const catalogue = fromCatalogue.status === "fulfilled" ? fromCatalogue.value : [];
        return {
          books: join([fromArchive.status === "fulfilled" ? fromArchive.value.books : [], catalogue]),
          hasMore: fromArchive.status === "fulfilled" && fromArchive.value.hasMore || catalogue.length === PAGE_SIZE
        };
      }
    })
  );
})();
