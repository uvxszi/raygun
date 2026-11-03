// dev.raygun.tapas 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var SITE = "https://tapas.io";
  var API = "https://story-api.tapas.io/cosmos/api/v1/landing";
  var PAGE_SIZE = 25;
  var EPISODES_PER_PAGE = 20;
  var PARALLEL_PAGES = 4;
  var MAX_EPISODE_PAGES = 50;
  var coverOf = (series) => {
    const path = series.assetProperty?.bookCoverImage?.path ?? series.assetProperty?.thumbnailImage?.path;
    return path?.startsWith("https://") ? `${path}.jpg` : null;
  };
  var landing = async (path, params) => {
    const query = Object.entries({ category_type: "COMIC", subtab_id: 17, ...params }).map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`).join("&");
    const response = await raygun.http.fetch(`${API}/${path}?${query}`);
    if (!response.ok) throw new Error(`Tapas answered HTTP ${response.status}`);
    const body = await response.json();
    const items = (body.data?.items ?? []).filter((series) => Number.isSafeInteger(series.seriesId) && series.title?.trim()).map((series) => ({
      id: String(series.seriesId),
      title: series.title.trim(),
      coverUrl: coverOf(series),
      // Tapas shows these to signed-in adults only.
      nsfw: series.mature === true
    }));
    return { items, hasNextPage: body.meta?.pagination?.last === false };
  };
  var ranking = (page) => landing("ranking", { size: PAGE_SIZE, page: Math.max(page, 1) - 1 });
  var byGenre = (page, sort, genre, price) => landing("genre", {
    pageSize: PAGE_SIZE,
    page: Math.max(page, 1) - 1,
    sort_option: sort,
    bm_type: price,
    ...genre ? { genre_type: genre } : {}
  });
  var GENRES = [
    { value: "ROMANCE_FANTASY", label: "Romance fantasy" },
    { value: "ROMANCE", label: "Romance" },
    { value: "DRAMA", label: "Drama" },
    { value: "ACTION_FANTASY", label: "Action fantasy" },
    { value: "ACTION", label: "Action" },
    { value: "THRILLER_HORROR", label: "Thriller and horror" },
    { value: "BL", label: "BL" },
    { value: "GL", label: "GL" },
    { value: "LGBTQ", label: "LGBTQ+" },
    { value: "COMEDY", label: "Comedy" },
    { value: "FANTASY", label: "Fantasy" }
  ];
  var SORTS = [
    { value: "POPULAR", label: "Popular" },
    { value: "NEWEST_EPISODE", label: "Latest episode" },
    { value: "NEWEST_SERIES", label: "Newest series" }
  ];
  var PRICES = [
    { value: "ALL", label: "Any" },
    { value: "FREE", label: "Free to read" },
    { value: "WUF", label: "Wait until free" }
  ];
  var filters = async () => [
    {
      kind: "select",
      key: "genre",
      label: "Genre",
      default: "any",
      options: [{ value: "any", label: "Any" }, ...GENRES]
    },
    { kind: "select", key: "sort", label: "Sort by", default: "POPULAR", options: SORTS },
    { kind: "select", key: "price", label: "Episodes", default: "ALL", options: PRICES }
  ];
  var pick = (chosen, key, options) => {
    const value = chosen?.[key];
    return typeof value === "string" && options.some((option) => option.value === value) ? value : null;
  };
  var unmarked = (text) => text.replace(/#\/?_h_i_g_h_L_i_g_h_t_#/g, "").replace(/\s+/g, " ").trim();
  var searchPage = async (query, page) => {
    const response = await raygun.http.fetch(
      `${SITE}/search?q=${encodeURIComponent(query)}&t=COMICS&pageNumber=${Math.max(page, 1)}`
    );
    if (!response.ok) throw new Error(`Tapas search answered HTTP ${response.status}`);
    const html = await response.text();
    const seen = /* @__PURE__ */ new Set();
    const items = [];
    for (const result of raygun.html.select(html, ".search-item-wrap")) {
      const link = result.select(".title-section .title a")[0];
      const id = link?.attr("data-series-id") ?? "";
      const title = link ? unmarked(link.text) : "";
      if (!/^\d+$/.test(id) || !title || seen.has(id)) continue;
      seen.add(id);
      const image = result.select(".item-thumb-wrap img")[0]?.attr("src");
      items.push({ id, title, coverUrl: image?.startsWith("https://") ? image : null });
    }
    return {
      items,
      hasNextPage: raygun.html.select(html, "a.paging__button--next").length > 0
    };
  };
  var search = (query, page, chosen) => {
    const text = query.trim();
    if (text) return searchPage(text, page);
    return byGenre(
      page,
      pick(chosen, "sort", SORTS) ?? "POPULAR",
      pick(chosen, "genre", GENRES),
      pick(chosen, "price", PRICES) ?? "ALL"
    );
  };
  var seriesNumber = (id) => {
    if (!/^\d+$/.test(id)) throw new Error(`Tapas has no series ${id}`);
    return id;
  };
  var textsOf = (html, selector) => [
    ...new Set(
      raygun.html.select(html, selector).map((element) => element.text.replace(/\s+/g, " ").trim()).filter(Boolean)
    )
  ];
  var statusOf = (schedule) => {
    const words = schedule.join(" ");
    if (/complete/i.test(words)) return "completed";
    if (/hiatus|break/i.test(words)) return "hiatus";
    if (/update|every|daily|weekly/i.test(words)) return "ongoing";
    return "unknown";
  };
  var details = async (id) => {
    const response = await raygun.http.fetch(`${SITE}/series/${seriesNumber(id)}/info`);
    if (response.status === 404) throw new Error("Tapas no longer has this series");
    if (!response.ok) throw new Error(`Tapas answered HTTP ${response.status}`);
    const html = await response.text();
    const title = textsOf(html, ".info__right .title-wrapper .title")[0] ?? textsOf(html, ".info__right .title")[0] ?? "Untitled";
    const cover = raygun.html.select(html, ".thumb.js-thumbnail img")[0]?.attr("src");
    return {
      id,
      title,
      coverUrl: cover?.startsWith("https://") ? cover : null,
      authors: textsOf(html, ".creator a.creator__item"),
      artists: [],
      description: textsOf(html, ".description__body")[0] ?? null,
      genres: textsOf(html, ".genre-btn-group a.genre-btn"),
      status: statusOf(textsOf(html, ".schedule-label")),
      url: response.url.startsWith(SITE) ? response.url : `${SITE}/series/${id}/info`
    };
  };
  var episodePage = async (id, page) => {
    const response = await raygun.http.fetch(
      `${SITE}/series/${id}/episodes?page=${page}&sort=NEWEST&large=true&last_access=0`,
      { headers: { Accept: "application/json" } }
    );
    if (!response.ok) throw new Error(`Tapas's episode list answered HTTP ${response.status}`);
    return (await response.json()).data ?? {};
  };
  var chapters = async (id) => {
    const series = seriesNumber(id);
    const first = await episodePage(series, 1);
    const lists = [first.episodes ?? []];
    const total = first.pagination?.total;
    if (typeof total === "number") {
      const last = Math.min(Math.ceil(total / EPISODES_PER_PAGE), MAX_EPISODE_PAGES);
      for (let start = 2; start <= last; start += PARALLEL_PAGES) {
        const batch = [];
        for (let page = start; page < start + PARALLEL_PAGES && page <= last; page += 1) {
          batch.push(episodePage(series, page).then((data) => data.episodes ?? []));
        }
        lists.push(...await Promise.all(batch));
      }
    } else {
      let more = first.pagination?.has_next === true;
      for (let page = 2; more && page <= MAX_EPISODE_PAGES; page += 1) {
        const next = await episodePage(series, page);
        lists.push(next.episodes ?? []);
        more = next.pagination?.has_next === true;
      }
    }
    const seen = /* @__PURE__ */ new Set();
    return lists.flat().filter(
      (episode) => Number.isSafeInteger(episode.id) && !seen.has(episode.id) && Boolean(seen.add(episode.id)) && episode.free === true && episode.scheduled !== true
    ).map((episode) => {
      const date = episode.publish_date ? Date.parse(episode.publish_date) : NaN;
      return {
        id: String(episode.id),
        name: episode.title?.trim() || null,
        number: typeof episode.scene === "number" ? episode.scene : null,
        dateUpload: Number.isFinite(date) ? date : null
      };
    });
  };
  var pages = async (chapterId) => {
    if (!/^\d+$/.test(chapterId)) throw new Error(`Tapas has no episode ${chapterId}`);
    const response = await raygun.http.fetch(`${SITE}/episode/${chapterId}`);
    if (!response.ok) throw new Error(`Tapas answered HTTP ${response.status}`);
    const html = await response.text();
    const images = raygun.html.select(html, "img.content__img").map((image) => image.attr("data-src") ?? image.attr("src") ?? "").filter((url) => url.startsWith("https://"));
    if (images.length === 0) {
      throw new Error("Tapas keeps this episode for readers with coins or an account");
    }
    return images;
  };
  registerProvider(
    defineProvider({
      kind: "manga.source",
      id: "tapas",
      name: "Tapas",
      popular: ({ page }) => ranking(page),
      latest: ({ page }) => byGenre(page, "NEWEST_EPISODE", null, "ALL"),
      search: ({ query, page, filters: chosen }) => search(query, page, chosen),
      filters,
      details: ({ id }) => details(id),
      chapters: async ({ id }) => ({ chapters: await chapters(id) }),
      pages: async ({ chapterId }) => ({ pages: await pages(chapterId) })
    })
  );
})();
