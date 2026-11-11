// dev.raygun.fyyd 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
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
  var API = "https://api.fyyd.de/0.2";
  var MOST = 50;
  var LANGUAGES = {
    Arabic: "ar",
    Catalan: "ca",
    Dutch: "nl",
    English: "en",
    French: "fr",
    German: "de",
    Italian: "it",
    Persian: "fa",
    Polish: "pl",
    Portuguese: "pt",
    Russian: "ru",
    Spanish: "es",
    Swedish: "sv"
  };
  var HEADERS = { Accept: "application/json" };
  var plain = (text) => (text ?? "").replace(/<[^>]+>/g, " ").replace(/&nbsp;/g, " ").replace(/&amp;/g, "&").replace(/&quot;/g, '"').replace(/&#0?39;|&rsquo;/g, "'").replace(/\s+/g, " ").trim();
  var isWeb = (address) => typeof address === "string" && /^https?:\/\//i.test(address);
  var artwork = (podcast) => {
    if (isWeb(podcast.imgURL) && podcast.imgURL.startsWith("https://")) return podcast.imgURL;
    return isWeb(podcast.layoutImageURL) ? podcast.layoutImageURL : null;
  };
  var toResult = (podcast) => {
    const title = plain(podcast.title);
    if (!title || !isWeb(podcast.xmlURL)) return null;
    const status = Number(podcast.status ?? 200);
    if (Number.isFinite(status) && status >= 400) return null;
    const description = plain(podcast.description) || plain(podcast.subtitle);
    return {
      title,
      author: plain(podcast.author) || null,
      feedUrl: podcast.xmlURL,
      artworkUrl: artwork(podcast),
      description: description ? description.slice(0, 600) : null
    };
  };
  var ask = async (path, params) => {
    const answer = await fetchJson(`${API}/${path}?${query(params)}`, { headers: HEADERS });
    if (answer.status === 0)
      throw new Error(`fyyd couldn't answer: ${answer.msg ?? "unknown error"}`);
    const seen = /* @__PURE__ */ new Set();
    return (answer.data ?? []).map(toResult).filter((podcast) => {
      if (!podcast || seen.has(podcast.feedUrl)) return false;
      seen.add(podcast.feedUrl);
      return true;
    });
  };
  var count = (limit) => Math.max(1, Math.min(limit || 20, MOST));
  var simple = (text) => (text ?? "").toLowerCase().replace(/['’]/g, "").replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  var naming = (words) => (podcast) => {
    const about = ` ${simple(`${podcast.title} ${podcast.author ?? ""} ${podcast.description ?? ""}`)} `;
    return words.every((word) => about.includes(` ${word}`));
  };
  registerProvider(
    defineProvider({
      kind: "podcast.directory",
      id: "podcasts",
      name: "fyyd",
      // Titles first; then the full-text search, which matches any word anywhere in a feed, kept to
      // podcasts that name every word.
      async search({ query: text, limit }) {
        const term = text.trim();
        if (!term) return [];
        const words = simple(term).split(" ").filter(Boolean);
        const [byTitle, byTerm] = await Promise.allSettled([
          ask("search/podcast", { title: term, count: count(limit) }),
          ask("search/podcast", { term, count: MOST })
        ]);
        if (byTitle.status === "rejected" && byTerm.status === "rejected") throw byTitle.reason;
        const seen = /* @__PURE__ */ new Set();
        return [
          ...byTitle.status === "fulfilled" ? byTitle.value : [],
          ...byTerm.status === "fulfilled" ? byTerm.value.filter(naming(words)) : []
        ].filter((podcast) => {
          if (seen.has(podcast.feedUrl)) return false;
          seen.add(podcast.feedUrl);
          return true;
        }).slice(0, count(limit));
      },
      async top({ limit }) {
        const language = LANGUAGES[raygun.settings.get("language") ?? "English"];
        const params = { count: count(limit) };
        if (language) params.language = language;
        const hot = await ask("feature/podcast/hot", params);
        return hot.slice(0, count(limit));
      }
    })
  );
})();
