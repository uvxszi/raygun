// dev.raygun.webtoons 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var SITE = "https://www.webtoons.com";
  var MOBILE = "https://m.webtoons.com";
  var IMAGE_HEADERS = { Referer: `${SITE}/` };
  var PAGE_SIZE = 30;
  var LIST_TTL_MS = 10 * 6e4;
  var LANGUAGES = {
    English: "en",
    Spanish: "es",
    French: "fr",
    German: "de",
    Indonesian: "id",
    Thai: "th",
    "Chinese (Traditional)": "zh-hant"
  };
  var KNOWN_LANGUAGES = new Set(Object.values(LANGUAGES));
  var language = () => LANGUAGES[raygun.settings.get("language") ?? ""] ?? "en";
  var seriesId = (series) => `${series.lang}:${series.kind}:${series.titleNo}`;
  var parseSeriesId = (id) => {
    const [lang = "", kind, titleNo = ""] = id.split(":");
    if (!KNOWN_LANGUAGES.has(lang) || kind !== "webtoon" && kind !== "canvas" || !/^\d+$/.test(titleNo)) {
      throw new Error(`WEBTOON has no series ${id}`);
    }
    return { lang, kind, titleNo };
  };
  var seriesUrl = (series) => `${SITE}/${series.lang}/${series.kind === "canvas" ? "canvas" : "series"}/series/list?title_no=${series.titleNo}`;
  var fixUrl = (url) => url.trim().replace(/ /g, "%20");
  var fetchPage = async (url) => {
    const response = await raygun.http.fetch(url, {
      // Episodes of series for grown-ups open only past the site's age question; the app's NSFW
      // switch is that answer.
      headers: raygun.content.showNsfw() ? { Cookie: "ageGatePass=true" } : {}
    });
    if (response.status === 404) throw new Error("WEBTOON no longer has this page");
    if (!response.ok) throw new Error(`WEBTOON answered HTTP ${response.status}`);
    return { html: await response.text(), url: response.url };
  };
  var get = async (url) => (await fetchPage(url)).html;
  var remembered = /* @__PURE__ */ new Map();
  var cached = (key, load) => {
    const hit = remembered.get(key);
    if (hit && Date.now() - hit.at < LIST_TTL_MS) return hit.value;
    const value = load();
    remembered.set(key, { at: Date.now(), value });
    value.catch(() => remembered.delete(key));
    while (remembered.size > 40) remembered.delete(remembered.keys().next().value);
    return value;
  };
  var CARD_LINK = /^https:\/\/www\.webtoons\.com\/([a-z-]+)\/([^/]+)\/[^/]+\/list\?title_no=(\d+)/;
  var cardOf = (card) => {
    const match = CARD_LINK.exec(card.attr("href") ?? "");
    const title = card.select(".title, .subj")[0]?.text.replace(/\s+/g, " ").trim();
    if (!match || !title || !KNOWN_LANGUAGES.has(match[1])) return null;
    const image = card.select("img")[0]?.attr("src");
    return {
      id: seriesId({
        lang: match[1],
        kind: match[2] === "canvas" ? "canvas" : "webtoon",
        titleNo: match[3]
      }),
      title,
      coverUrl: image?.startsWith("http") ? fixUrl(image) : null,
      coverHeaders: IMAGE_HEADERS
    };
  };
  var cardsIn = (html) => {
    const seen = /* @__PURE__ */ new Set();
    const cards = [];
    for (const element of raygun.html.select(
      html,
      "ul.webtoon_list li a, .challenge_lst li a.challenge_item"
    )) {
      const card = cardOf(element);
      if (card && !seen.has(card.id)) {
        seen.add(card.id);
        cards.push(card);
      }
    }
    return cards;
  };
  var cardsOf = (url) => get(url).then(cardsIn);
  var unique = (lists) => {
    const seen = /* @__PURE__ */ new Set();
    return lists.flat().filter((card) => !seen.has(card.id) && Boolean(seen.add(card.id)));
  };
  var allOf = async (urls) => {
    const results = await Promise.allSettled(urls.map(cardsOf));
    const lists = results.flatMap((result) => result.status === "fulfilled" ? [result.value] : []);
    const failure = results.find((result) => result.status === "rejected");
    if (lists.length === 0 && failure?.status === "rejected") throw failure.reason;
    return unique(lists);
  };
  var slice = (cards, page) => {
    const start = (Math.max(page, 1) - 1) * PAGE_SIZE;
    return {
      items: cards.slice(start, start + PAGE_SIZE),
      hasNextPage: start + PAGE_SIZE < cards.length
    };
  };
  var linksPageAfter = (html, page) => raygun.html.select(html, 'a[href*="page="]').some((link) => new RegExp(`[?&]page=${page + 1}(?:&|$)`).test(link.attr("href") ?? ""));
  var rankings = (lang) => cached(
    `rankings:${lang}`,
    () => allOf(
      ["popular", "trending", "originals", "canvas"].map(
        (name) => `${SITE}/${lang}/ranking/${name}`
      )
    )
  );
  var latest = (lang) => cached(`latest:${lang}`, () => cardsOf(`${SITE}/${lang}/originals?sortOrder=UPDATE`));
  var GENRES = [
    ["Action", "action", "ACTION"],
    ["Comedy", "comedy", "COMEDY"],
    ["Drama", "drama", "DRAMA"],
    ["Fantasy", "fantasy", "FANTASY"],
    ["Graphic novel", "graphic_novel", null],
    ["Heartwarming", "heartwarming", "HEARTWARMING"],
    ["Historical", "historical", "HISTORICAL"],
    ["Horror", "horror", "HORROR"],
    ["Mystery", "mystery", "CRIME_MYSTERY"],
    ["Romance", "romance", "ROMANCE"],
    ["Romantic fantasy", null, "ROMANTIC_FANTASY"],
    ["Sci-fi", "sf", "SF"],
    ["Slice of life", "slice_of_life", "SLICE_OF_LIFE"],
    ["Sports", "sports", "SPORTS"],
    ["Superhero", "super_hero", "SUPER_HERO"],
    ["Supernatural", "supernatural", "SUPERNATURAL"],
    ["Thriller", "thriller", "THRILLER"],
    ["Informative", "tiptoon", "TIPTOON"],
    ["All ages", null, "ALL_AGES"],
    ["Animals", null, "ANIMALS"],
    ["BL / GL", null, "BL_GL"],
    ["Inspirational", null, "INSPIRATIONAL"],
    ["Post-apocalyptic", null, "POST_APOCALYPTIC"],
    ["School", null, "SCHOOL"],
    ["Short story", null, "SHORT_STORY"],
    ["Zombies", null, "ZOMBIES"]
  ];
  var genreValue = ([, originals, canvas]) => originals ?? canvas.toLowerCase();
  var SORTS = [
    { value: "MANA", label: "Popular" },
    { value: "LIKEIT", label: "Most liked" },
    { value: "UPDATE", label: "Latest episode" }
  ];
  var filters = async () => [
    {
      kind: "select",
      key: "type",
      label: "Series",
      default: "all",
      options: [
        { value: "all", label: "Originals and Canvas" },
        { value: "webtoon", label: "Originals" },
        { value: "canvas", label: "Canvas" }
      ]
    },
    {
      kind: "select",
      key: "genre",
      label: "Genre",
      default: "any",
      options: [
        { value: "any", label: "Any" },
        ...GENRES.map((genre) => ({ value: genreValue(genre), label: genre[0] }))
      ]
    },
    { kind: "select", key: "sort", label: "Sort by", default: "MANA", options: SORTS }
  ];
  var choice = (chosen, key, fallback) => {
    const value = chosen?.[key];
    return typeof value === "string" ? value : fallback;
  };
  var searchSection = async (lang, section, query, page) => {
    const html = await get(
      `${SITE}/${lang}/search/${section}?keyword=${encodeURIComponent(query)}&page=${page}`
    );
    return { items: cardsIn(html), hasNextPage: linksPageAfter(html, page) };
  };
  var genreSection = async (lang, kind, genre, sort, page) => {
    const [, originals, canvas] = genre;
    if (kind === "webtoon") {
      if (!originals) return { items: [], hasNextPage: false };
      const cards = await cached(
        `genre:${lang}:${originals}:${sort}`,
        () => cardsOf(`${SITE}/${lang}/genres/${originals}?sortOrder=${sort}`)
      );
      return slice(cards, page);
    }
    const html = await get(
      `${SITE}/${lang}/canvas/list?genreTab=${canvas ?? "ALL"}&sortOrder=${sort}&page=${page}`
    );
    return { items: cardsIn(html), hasNextPage: linksPageAfter(html, page) };
  };
  var both = async (pages2) => {
    const [originals, canvas] = await Promise.allSettled(pages2);
    const answered = [originals, canvas].flatMap(
      (result) => result.status === "fulfilled" ? [result.value] : []
    );
    if (answered.length === 0 && originals.status === "rejected") throw originals.reason;
    return {
      items: unique(answered.map((answer) => answer.items)),
      hasNextPage: answered.some((answer) => answer.hasNextPage)
    };
  };
  var search = async (query, page, chosen) => {
    const lang = language();
    const at = Math.max(page, 1);
    const type = choice(chosen, "type", "all");
    const picked = choice(chosen, "sort", "MANA");
    const sort = SORTS.some((option) => option.value === picked) ? picked : "MANA";
    const text = query.trim();
    if (text) {
      if (type === "webtoon") return searchSection(lang, "originals", text, at);
      if (type === "canvas") return searchSection(lang, "canvas", text, at);
      return both([
        searchSection(lang, "originals", text, at),
        searchSection(lang, "canvas", text, at)
      ]);
    }
    const genre = GENRES.find((option) => genreValue(option) === choice(chosen, "genre", "any"));
    if (genre) {
      if (type === "webtoon" || type === "canvas") {
        return genreSection(lang, type, genre, sort, at);
      }
      return both([
        genreSection(lang, "webtoon", genre, sort, at),
        genreSection(lang, "canvas", genre, sort, at)
      ]);
    }
    if (type === "canvas") {
      const html = await get(`${SITE}/${lang}/canvas/list?genreTab=ALL&sortOrder=${sort}&page=${at}`);
      return { items: cardsIn(html), hasNextPage: linksPageAfter(html, at) };
    }
    if (sort === "UPDATE") return slice(await latest(lang), at);
    const ranked = await rankings(lang);
    return slice(
      type === "webtoon" ? ranked.filter((card) => card.id.includes(":webtoon:")) : ranked,
      at
    );
  };
  var STATUS_WORDS = [
    // "COMPLETED", "TERMINÉ", "TAMAT", "จบแล้ว", "完結", "ABGESCHLOSSEN", "FINALIZADO".
    [/complete|termin|tamat|จบ|完結|abgeschlossen|finaliz/i, "completed"],
    [/hiatus|pause/i, "hiatus"],
    [/up|every|tous|cada|jeden|setiap|ทุก|每/i, "ongoing"]
  ];
  var textOf = (html, selector) => raygun.html.select(html, selector)[0]?.text.replace(/\s+/g, " ").trim() || null;
  var metaOf = (html, property) => raygun.html.select(html, `meta[property="${property}"]`)[0]?.attr("content")?.trim() || null;
  var details = async (id) => {
    const series = parseSeriesId(id);
    const { html, url } = await fetchPage(seriesUrl(series));
    const title = metaOf(html, "og:title") ?? textOf(html, ".detail_header .info .subj") ?? "Untitled";
    const authorArea = raygun.html.select(html, ".detail_header .author_area")[0];
    const authors = authorArea ? authorArea.text.replace(/author info/gi, "").split(",").map(
      (name) => name.replace(/\s+/g, " ").trim().replace(/\s*(?:\.{3}|…)$/, "").trim()
    ).filter(Boolean) : [];
    const genres = raygun.html.select(html, ".detail_header .info .genre").map((genre) => genre.text.replace(/\s+/g, " ").trim()).filter(Boolean);
    const schedule = textOf(html, "#_asideDetail .day_info") ?? "";
    const status = STATUS_WORDS.find(([words]) => words.test(schedule))?.[1] ?? "unknown";
    const cover = metaOf(html, "og:image");
    return {
      id,
      title,
      coverUrl: cover?.startsWith("http") ? fixUrl(cover) : null,
      coverHeaders: IMAGE_HEADERS,
      authors: [...new Set(authors)],
      artists: [],
      description: textOf(html, "#_asideDetail .summary"),
      genres: [...new Set(genres)],
      status,
      url: url.startsWith(SITE) ? url : seriesUrl(series)
    };
  };
  var chapters = async (id) => {
    const series = parseSeriesId(id);
    const extra = series.kind === "canvas" ? `&readingLanguageCode=${series.lang}` : "";
    const response = await raygun.http.fetch(
      `${MOBILE}/api/v1/${series.kind}/${series.titleNo}/episodes?pageSize=99999${extra}`,
      { headers: { Referer: `${MOBILE}/` } }
    );
    if (!response.ok) throw new Error(`WEBTOON's episode list answered HTTP ${response.status}`);
    const episodes = (await response.json()).result?.episodeList ?? [];
    return episodes.filter((episode) => Number.isInteger(episode.episodeNo)).map((episode) => ({
      id: `${id}:${episode.episodeNo}`,
      name: episode.episodeTitle?.trim() || null,
      number: episode.episodeNo,
      dateUpload: episode.exposureDateMillis ?? null
    })).reverse();
  };
  var pages = async (chapterId) => {
    const cut = chapterId.lastIndexOf(":");
    const series = parseSeriesId(chapterId.slice(0, cut));
    const episode = chapterId.slice(cut + 1);
    if (!/^\d+$/.test(episode)) throw new Error(`WEBTOON has no episode ${chapterId}`);
    const section = series.kind === "canvas" ? "canvas" : "series";
    const html = await get(
      `${SITE}/${series.lang}/${section}/series/episode/viewer?title_no=${series.titleNo}&episode_no=${episode}`
    );
    const images = raygun.html.select(html, "#_imageList img").map((image) => image.attr("data-url") ?? image.attr("src") ?? "").filter((url) => url.startsWith("http") && !url.includes("/bg_transparency."));
    if (images.length === 0) {
      throw new Error("WEBTOON shows this episode only on its website or app");
    }
    return images.map((url) => ({ url: fixUrl(url), headers: IMAGE_HEADERS }));
  };
  registerProvider(
    defineProvider({
      kind: "manga.source",
      id: "webtoons",
      name: "WEBTOON",
      popular: async ({ page }) => slice(await rankings(language()), page),
      latest: async ({ page }) => slice(await latest(language()), page),
      search: ({ query, page, filters: chosen }) => search(query, page, chosen),
      filters,
      details: ({ id }) => details(id),
      chapters: async ({ id }) => ({ chapters: await chapters(id) }),
      pages: async ({ chapterId }) => ({ pages: await pages(chapterId) })
    })
  );
})();
