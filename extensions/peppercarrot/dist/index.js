// dev.raygun.peppercarrot 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var SITE = "https://www.peppercarrot.com";
  var SOURCES = `${SITE}/0_sources`;
  var SERIES_ID = "peppercarrot";
  var COVER = `${SOURCES}/0ther/artworks/low-res/2016-02-24_vertical-cover_remake_by-David-Revoy.jpg`;
  var LIST_TTL_MS = 30 * 6e4;
  var LANGUAGES = {
    English: "en",
    French: "fr",
    Spanish: "es",
    "Mexican Spanish": "mx",
    German: "de",
    Italian: "it",
    "Portuguese (Brazil)": "pt",
    Dutch: "nl",
    Polish: "pl",
    Russian: "ru",
    Japanese: "ja",
    "Chinese (Simplified)": "cn",
    Indonesian: "id",
    Vietnamese: "vi",
    Catalan: "ca",
    Danish: "da",
    Finnish: "fi",
    Croatian: "hr",
    Hungarian: "hu",
    Romanian: "ro",
    Esperanto: "eo"
  };
  var language = () => LANGUAGES[raygun.settings.get("language") ?? ""] ?? "en";
  var SERIES = { id: SERIES_ID, title: "Pepper&Carrot", coverUrl: COVER };
  var ONE_SERIES = { items: [SERIES], hasNextPage: false };
  var remembered = /* @__PURE__ */ new Map();
  var cached = (key, load) => {
    const hit = remembered.get(key);
    if (hit && Date.now() - hit.at < LIST_TTL_MS) return hit.value;
    const value = load();
    remembered.set(key, { at: Date.now(), value });
    value.catch(() => remembered.delete(key));
    return value;
  };
  var episodes = () => cached("episodes", async () => {
    const response = await raygun.http.fetch(`${SOURCES}/episodes.json`);
    if (!response.ok) throw new Error(`Pepper&Carrot answered HTTP ${response.status}`);
    const list = await response.json();
    return list.filter(
      (episode) => /^ep\d+_/.test(episode.name) && Number.isInteger(episode.total_pages) && episode.total_pages > 0 && Array.isArray(episode.translated_languages)
    );
  });
  var captions = (lang) => cached(`captions:${lang}`, async () => {
    const found = /* @__PURE__ */ new Map();
    const response = await raygun.http.fetch(`${SITE}/${lang}/webcomics/peppercarrot.html`);
    if (!response.ok) return found;
    const html = await response.text();
    for (const caption of raygun.html.select(html, "figure figcaption")) {
      const link = caption.select("a[href]")[0];
      const folder = /\/webcomic\/(ep\d+_[^/.]+)\.html/.exec(link?.attr("href") ?? "")?.[1];
      const name = link?.text.replace(/\s+/g, " ").trim();
      if (!folder || !name) continue;
      const day = /(\d{4})-(\d{2})-(\d{2})/.exec(caption.text);
      found.set(folder, {
        name,
        date: day ? Date.UTC(Number(day[1]), Number(day[2]) - 1, Number(day[3])) : null
      });
    }
    return found;
  });
  var numberOf = (episode) => Number(/^ep(\d+)_/.exec(episode.name)?.[1] ?? NaN);
  var nameFromFolder = (folder) => folder.replace(/^ep\d+_/, "").replace(/-/g, " ");
  var chapters = async () => {
    const lang = language();
    const [list, names, english] = await Promise.all([
      episodes(),
      captions(lang),
      lang === "en" ? Promise.resolve(/* @__PURE__ */ new Map()) : captions("en")
    ]);
    return list.map((episode) => {
      const caption = names.get(episode.name) ?? english.get(episode.name);
      const number = numberOf(episode);
      return {
        id: episode.name,
        name: caption?.name ?? nameFromFolder(episode.name),
        number: Number.isFinite(number) ? number : null,
        dateUpload: caption?.date ?? english.get(episode.name)?.date ?? null
      };
    }).sort((a, b) => (b.number ?? 0) - (a.number ?? 0));
  };
  var two = (value) => String(value).padStart(2, "0");
  var pages = async (chapterId) => {
    const episode = (await episodes()).find((item) => item.name === chapterId);
    if (!episode) throw new Error(`Pepper&Carrot has no episode ${chapterId}`);
    const chosen = language();
    const lang = episode.translated_languages.includes(chosen) ? chosen : "en";
    const size = raygun.settings.get("fullSize") === true ? "hi-res" : "low-res";
    const number = two(numberOf(episode));
    return Array.from(
      { length: episode.total_pages + 1 },
      (_, page) => `${SOURCES}/${episode.name}/${size}/${lang}_Pepper-and-Carrot_by-David-Revoy_E${number}P${two(page)}.jpg`
    );
  };
  var details = async () => ({
    id: SERIES_ID,
    title: "Pepper&Carrot",
    coverUrl: COVER,
    authors: ["David Revoy"],
    artists: ["David Revoy"],
    description: "Pepper, a young witch, and her cat Carrot in the magical world of Hereva. Free and open source, under the Creative Commons Attribution 4.0 license.",
    genres: ["Fantasy", "Comedy", "Adventure"],
    status: "ongoing",
    url: `${SITE}/${language()}/webcomics/peppercarrot.html`
  });
  var simple = (text) => text.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  var search = async (query, page) => {
    const none = { items: [], hasNextPage: false };
    if (page > 1) return none;
    const words = simple(query).split(" ").filter(Boolean);
    if (words.length === 0) return ONE_SERIES;
    const matches = (text) => {
      const haystack = simple(text);
      return words.every((word) => haystack.includes(word));
    };
    if (matches("pepper carrot david revoy hereva")) return ONE_SERIES;
    const names = await chapters().catch(() => []);
    return names.some((chapter) => matches(chapter.name ?? "")) ? ONE_SERIES : none;
  };
  registerProvider(
    defineProvider({
      kind: "manga.source",
      id: "peppercarrot",
      name: "Pepper&Carrot",
      popular: async () => ONE_SERIES,
      latest: async () => ONE_SERIES,
      search: ({ query, page }) => search(query, page),
      details: () => details(),
      chapters: async () => ({ chapters: await chapters() }),
      pages: async ({ chapterId }) => ({ pages: await pages(chapterId) })
    })
  );
})();
