// dev.raygun.mangaplus-creators 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var SITE = "https://mangaplus-creators.jp";
  var MAX_EPISODE_PAGES = 50;
  var PARALLEL_PAGES = 4;
  var LANGUAGES = { English: "en", Spanish: "es" };
  var language = () => LANGUAGES[raygun.settings.get("language") ?? ""] ?? "en";
  var get = async (path) => {
    const response = await raygun.http.fetch(`${SITE}${path}`);
    if (response.status === 404) throw new Error("MANGA Plus Creators no longer has this page");
    if (!response.ok) throw new Error(`MANGA Plus Creators answered HTTP ${response.status}`);
    return { html: await response.text(), url: response.url };
  };
  var COVER_ID = /\/u\/title\/([A-Za-z0-9]+)\//;
  var cardsIn = (html) => {
    const seen = /* @__PURE__ */ new Set();
    const items = [];
    for (const card of raygun.html.select(html, ".item-recent, .item-search")) {
      const cover = card.select(".image-area img")[0]?.attr("src") ?? "";
      const id = COVER_ID.exec(cover)?.[1];
      const title = card.select(".title-area .title")[0]?.text.replace(/\s+/g, " ").trim();
      if (!id || !title || seen.has(id)) continue;
      seen.add(id);
      items.push({ id, title, coverUrl: cover.startsWith("https://") ? cover : null });
    }
    return { items, hasNextPage: false };
  };
  var list = async (path, page) => page > 1 ? { items: [], hasNextPage: false } : cardsIn((await get(path)).html);
  var GENRES = [
    { value: "action", label: "Action" },
    { value: "comedy", label: "Comedy" },
    { value: "fantasy", label: "Fantasy" },
    { value: "horror", label: "Horror" },
    { value: "mystery", label: "Mystery" },
    { value: "romance", label: "Romance" },
    { value: "sf", label: "Sci-fi" },
    { value: "slice_of_life", label: "Slice of life" },
    { value: "sports", label: "Sports" },
    { value: "others", label: "Others" }
  ];
  var SORTS = [
    { value: "popular", label: "Popular" },
    { value: "latest_desc", label: "Latest episode" },
    { value: "like_desc", label: "Most liked" }
  ];
  var filters = async () => [
    {
      kind: "select",
      key: "genre",
      label: "Genre",
      default: "any",
      options: [{ value: "any", label: "Any" }, ...GENRES]
    },
    { kind: "select", key: "sort", label: "Sort by", default: "popular", options: SORTS }
  ];
  var pick = (chosen, key, options) => {
    const value = chosen?.[key];
    return typeof value === "string" && options.some((option) => option.value === value) ? value : null;
  };
  var search = (query, page, chosen) => {
    const lang = language();
    const text = query.trim();
    if (text) return list(`/keywords?q=${encodeURIComponent(text)}&s=date&lang=${lang}`, page);
    const genre = pick(chosen, "genre", GENRES);
    const sort = pick(chosen, "sort", SORTS);
    if (genre) {
      const order = sort && sort !== "popular" ? `&s=${sort}` : "";
      return list(`/genres/${genre}?l=${lang}${order}`, page);
    }
    if (sort === "latest_desc") return list(`/titles/recent/?t=episode&l=${lang}`, page);
    return list(`/titles/popular/?p=m&l=${lang}`, page);
  };
  var seriesPath = (id) => {
    if (!/^[A-Za-z0-9]+$/.test(id)) throw new Error(`MANGA Plus Creators has no series ${id}`);
    return `/titles/${id}`;
  };
  var textOf = (html, selector) => raygun.html.select(html, selector)[0]?.text.replace(/\s+/g, " ").trim() || null;
  var details = async (id) => {
    const { html, url } = await get(seriesPath(id));
    const cover = raygun.html.select(html, ".book-box .cover img")[0];
    const coverUrl = cover?.attr("data-src") ?? cover?.attr("src") ?? "";
    const kind = textOf(html, ".book-box .book-submit-type") ?? "";
    const status = /one.?shot/i.test(kind) ? "completed" : "unknown";
    return {
      id,
      title: textOf(html, ".book-box .title") ?? "Untitled",
      coverUrl: coverUrl.startsWith("https://") ? coverUrl : null,
      authors: [
        ...new Set(
          raygun.html.select(html, ".book-box .mod-btn-profile .name").map((name) => name.text.trim()).filter(Boolean)
        )
      ],
      artists: [],
      description: textOf(html, ".book-box .summary"),
      genres: [
        ...new Set(
          raygun.html.select(html, ".book-box .genre-area .tag-genre").map((genre) => genre.text.trim()).filter(Boolean)
        )
      ],
      status,
      url
    };
  };
  var EPISODE_LINK = /\/episodes\/([A-Za-z0-9]+)\/?$/;
  var episodePage = async (path, page) => {
    const { html } = await get(`${path}/?page=${page}`);
    const episodes = raygun.html.select(html, "a.mod-item-series").flatMap((episode) => {
      const episodeId = EPISODE_LINK.exec(episode.attr("href") ?? "")?.[1];
      if (!episodeId) return [];
      const label = episode.select(".number")[0]?.text.replace(/\s+/g, " ").trim() ?? "";
      const number = /#?(\d+(?:\.\d+)?)/.exec(label)?.[1];
      const day = /(\d{4})-(\d{2})-(\d{2})/.exec(episode.select(".first-update")[0]?.text ?? "");
      return [
        {
          id: episodeId,
          name: number ? null : label || "One-shot",
          number: number ? Number(number) : null,
          dateUpload: day ? Date.UTC(Number(day[1]), Number(day[2]) - 1, Number(day[3])) : null
        }
      ];
    });
    const linked = raygun.html.select(html, 'a[href*="page="]').map((link) => Number(/[?&]page=(\d+)/.exec(link.attr("href") ?? "")?.[1] ?? 0));
    return {
      episodes,
      lastLinked: Math.max(page, ...linked),
      more: raygun.html.select(html, "a.next").length > 0
    };
  };
  var chapters = async (id) => {
    const path = seriesPath(id);
    const first = await episodePage(path, 1);
    const lists = [first.episodes];
    let page = 1;
    let more = first.more;
    const linked = Math.min(first.lastLinked, MAX_EPISODE_PAGES);
    while (more && page < linked) {
      const batch = [];
      for (let next = page + 1; next <= Math.min(page + PARALLEL_PAGES, linked); next += 1) {
        batch.push(episodePage(path, next));
      }
      const answers = await Promise.all(batch);
      lists.push(...answers.map((answer) => answer.episodes));
      page += answers.length;
      more = answers[answers.length - 1]?.more ?? false;
    }
    while (more && page < MAX_EPISODE_PAGES) {
      page += 1;
      const next = await episodePage(path, page);
      lists.push(next.episodes);
      more = next.more && next.episodes.length > 0;
    }
    const seen = /* @__PURE__ */ new Set();
    const found = lists.flat().filter((episode) => !seen.has(episode.id) && Boolean(seen.add(episode.id)));
    return found.reverse();
  };
  var pages = async (chapterId) => {
    if (!/^[A-Za-z0-9]+$/.test(chapterId)) {
      throw new Error(`MANGA Plus Creators has no episode ${chapterId}`);
    }
    const { html, url } = await get(`/episodes/${chapterId}/`);
    const viewer = raygun.html.select(html, 'div[react="viewer"]')[0];
    const data = viewer?.attr("data-pages");
    if (!data) throw new Error("MANGA Plus Creators shows this episode only on its website");
    const list2 = JSON.parse(data).pc ?? [];
    const images = list2.filter((page) => page.image_url?.startsWith("https://")).sort((a, b) => (a.page_no ?? 0) - (b.page_no ?? 0)).map((page) => ({ url: page.image_url, headers: { Referer: url } }));
    if (images.length === 0) throw new Error("This MANGA Plus Creators episode has no pages");
    return images;
  };
  registerProvider(
    defineProvider({
      kind: "manga.source",
      id: "creators",
      name: "MANGA Plus Creators",
      popular: ({ page }) => list(`/titles/popular/?p=m&l=${language()}`, page),
      latest: ({ page }) => list(`/titles/recent/?t=episode&l=${language()}`, page),
      search: ({ query, page, filters: chosen }) => search(query, page, chosen),
      filters,
      details: ({ id }) => details(id),
      chapters: async ({ id }) => ({ chapters: await chapters(id) }),
      pages: async ({ chapterId }) => ({ pages: await pages(chapterId) })
    })
  );
})();
