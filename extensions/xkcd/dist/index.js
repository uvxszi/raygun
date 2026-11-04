// dev.raygun.xkcd 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var SITE = "https://xkcd.com";
  var SERIES_ID = "xkcd";
  var COVER = "https://imgs.xkcd.com/static/terrible_small_logo.png";
  var ARCHIVE_TTL_MS = 30 * 6e4;
  var SERIES = { id: SERIES_ID, title: "xkcd", coverUrl: COVER };
  var ONE_SERIES = { items: [SERIES], hasNextPage: false };
  var archive = null;
  var readArchive = async () => {
    const response = await raygun.http.fetch(`${SITE}/archive/`);
    if (!response.ok) throw new Error(`xkcd answered HTTP ${response.status}`);
    const html = await response.text();
    const comics2 = [];
    for (const link of raygun.html.select(html, "#middleContainer a[href]")) {
      const number = /^\/(\d+)\/$/.exec(link.attr("href") ?? "")?.[1];
      if (!number) continue;
      const [year, month, day] = (link.attr("title") ?? "").split("-").map(Number);
      const date = year && month && day ? Date.UTC(year, month - 1, day) : NaN;
      comics2.push({
        number: Number(number),
        title: link.text.trim() || `xkcd ${number}`,
        date: Number.isFinite(date) ? date : null
      });
    }
    if (comics2.length === 0) throw new Error("xkcd's archive had no comics in it");
    return comics2;
  };
  var comics = () => {
    if (!archive || Date.now() - archive.at > ARCHIVE_TTL_MS) {
      const current = { at: Date.now(), comics: readArchive() };
      current.comics.catch(() => {
        if (archive === current) archive = null;
      });
      archive = current;
    }
    return archive.comics;
  };
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
    if (matches("xkcd randall munroe")) return ONE_SERIES;
    const found = (await comics()).some(
      (comic) => matches(comic.title) || words.join(" ") === String(comic.number)
    );
    return found ? ONE_SERIES : none;
  };
  var details = async () => ({
    id: SERIES_ID,
    title: "xkcd",
    coverUrl: COVER,
    authors: ["Randall Munroe"],
    artists: ["Randall Munroe"],
    description: "A webcomic of romance, sarcasm, math, and language.",
    genres: ["Comedy", "Science"],
    status: "ongoing",
    url: SITE
  });
  var chapters = async () => (await comics()).map((comic) => ({
    id: String(comic.number),
    name: comic.title,
    number: comic.number,
    dateUpload: comic.date
  }));
  var absolute = (url) => url.startsWith("//") ? `https:${url}` : url;
  var pages = async (chapterId) => {
    if (!/^\d+$/.test(chapterId)) throw new Error(`xkcd has no comic ${chapterId}`);
    const response = await raygun.http.fetch(`${SITE}/${chapterId}/`);
    if (response.status === 404) throw new Error(`xkcd has no comic ${chapterId}`);
    if (!response.ok) throw new Error(`xkcd answered HTTP ${response.status}`);
    const html = await response.text();
    const image = raygun.html.select(html, "#comic img")[0];
    if (!image) throw new Error("This xkcd is interactive; it works only on xkcd.com");
    const large = raygun.html.select(html, "#comic a[href]")[0]?.attr("href") ?? "";
    if (/^(?:https?:)?\/\/imgs\.xkcd\.com\/comics\/.+\.(?:png|jpe?g|gif)$/i.test(large)) {
      return [absolute(large)];
    }
    const sharp = /(\S+)\s+2x/.exec(image.attr("srcset") ?? "")?.[1];
    const url = absolute(sharp ?? image.attr("src") ?? "");
    if (!url.startsWith("https://")) throw new Error("This xkcd has no picture to show");
    return [url];
  };
  registerProvider(
    defineProvider({
      kind: "manga.source",
      id: "xkcd",
      name: "xkcd",
      popular: async () => ONE_SERIES,
      latest: async () => ONE_SERIES,
      search: ({ query, page }) => search(query, page),
      details: () => details(),
      chapters: async () => ({ chapters: await chapters() }),
      pages: async ({ chapterId }) => ({ pages: await pages(chapterId) })
    })
  );
})();
