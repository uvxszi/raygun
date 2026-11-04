// dev.raygun.naver-webtoon 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var SITE = "https://comic.naver.com";
  var HEADERS = { Referer: `${SITE}/` };
  var PAGE_SIZE = 30;
  var LIST_TTL_MS = 10 * 6e4;
  var PARALLEL_PAGES = 4;
  var MAX_EPISODE_PAGES = 60;
  var DAYS = ["MONDAY", "TUESDAY", "WEDNESDAY", "THURSDAY", "FRIDAY", "SATURDAY", "SUNDAY"];
  var api = async (path) => {
    const response = await raygun.http.fetch(`${SITE}/api${path}`, { headers: HEADERS });
    if (response.status === 404) throw new Error("Naver Webtoon no longer has this series");
    if (!response.ok) throw new Error(`Naver Webtoon answered HTTP ${response.status}`);
    return response.json();
  };
  var summary = (title) => Number.isSafeInteger(title.titleId) && title.titleName?.trim() ? {
    id: String(title.titleId),
    title: title.titleName.trim(),
    coverUrl: title.thumbnailUrl?.startsWith("https://") ? title.thumbnailUrl : null,
    coverHeaders: HEADERS,
    // Naver shows these to signed-in adults only.
    nsfw: title.adult === true
  } : null;
  var summaries = (titles) => {
    const seen = /* @__PURE__ */ new Set();
    return (titles ?? []).flatMap((title) => {
      const item = summary(title);
      if (!item || seen.has(item.id)) return [];
      seen.add(item.id);
      return [item];
    });
  };
  var paged = (titles, info) => ({
    items: summaries(titles),
    hasNextPage: (info?.page ?? 1) < (info?.totalPages ?? 0)
  });
  var slice = (items, page) => {
    const start = (Math.max(page, 1) - 1) * PAGE_SIZE;
    return {
      items: items.slice(start, start + PAGE_SIZE),
      hasNextPage: start + PAGE_SIZE < items.length
    };
  };
  var remembered = /* @__PURE__ */ new Map();
  var weekdays = (order) => {
    const hit = remembered.get(order);
    if (hit && Date.now() - hit.at < LIST_TTL_MS) return hit.value;
    const value = api(`/webtoon/titlelist/weekday?order=${order}`);
    remembered.set(order, { at: Date.now(), value });
    value.catch(() => remembered.delete(order));
    return value;
  };
  var popular = async (page) => {
    const lists = await weekdays("user");
    const today = Math.max(DAYS.indexOf(lists.dayOfWeek ?? ""), 0);
    const days = DAYS.map((_, index) => lists.titleListMap?.[DAYS[(today + index) % 7]] ?? []);
    const longest = Math.max(0, ...days.map((day) => day.length));
    const interleaved = [];
    for (let rank = 0; rank < longest; rank += 1) {
      for (const day of days) {
        const title = day[rank];
        if (title) interleaved.push(title);
      }
    }
    return slice(summaries(interleaved), page);
  };
  var latest = async (page) => {
    const lists = await weekdays("update");
    return slice(summaries(lists.titleListMap?.[lists.dayOfWeek ?? ""] ?? []), page);
  };
  var GENRES = [
    { value: "PURE", label: "Romance" },
    { value: "FANTASY", label: "Fantasy" },
    { value: "ACTION", label: "Action" },
    { value: "DAILY", label: "Daily life" },
    { value: "THRILL", label: "Thriller" },
    { value: "COMIC", label: "Comedy" },
    { value: "DRAMA", label: "Drama" },
    { value: "SENSIBILITY", label: "Emotional" },
    { value: "HISTORICAL", label: "Historical" },
    { value: "SPORTS", label: "Sports" }
  ];
  var SORTS = [
    { value: "USER", label: "Popular" },
    { value: "UPDATE", label: "Latest episode" },
    { value: "VIEW", label: "Most viewed" }
  ];
  var STATUSES = [
    { value: "any", label: "Any" },
    { value: "finished", label: "Finished" }
  ];
  var filters = async () => [
    {
      kind: "select",
      key: "genre",
      label: "Genre",
      default: "any",
      options: [{ value: "any", label: "Any" }, ...GENRES]
    },
    { kind: "select", key: "status", label: "Status", default: "any", options: STATUSES },
    { kind: "select", key: "sort", label: "Sort by", default: "USER", options: SORTS }
  ];
  var pick = (chosen, key, options) => {
    const value = chosen?.[key];
    return typeof value === "string" && options.some((option) => option.value === value) ? value : null;
  };
  var search = async (query, page, chosen) => {
    const at = Math.max(page, 1);
    const text = query.trim();
    if (text) {
      const found = await api(
        `/search/webtoon?keyword=${encodeURIComponent(text)}&page=${at}`
      );
      return paged(found.searchList, found.pageInfo);
    }
    const sort = pick(chosen, "sort", SORTS) ?? "USER";
    const genre = pick(chosen, "genre", GENRES);
    if (genre) {
      const found = await api(
        `/webtoon/titlelist/genre?genre=${genre}&order=${sort}&page=${at}`
      );
      return paged(found.titleList, found.pageInfo);
    }
    if (pick(chosen, "status", STATUSES) === "finished") {
      const found = await api(`/webtoon/titlelist/finished?order=${sort}&page=${at}`);
      return paged(found.titleList, found.pageInfo);
    }
    return sort === "UPDATE" ? latest(at) : popular(at);
  };
  var titleNumber = (id) => {
    if (!/^\d+$/.test(id)) throw new Error(`Naver Webtoon has no series ${id}`);
    return id;
  };
  var details = async (id) => {
    const info = await api(`/article/list/info?titleId=${titleNumber(id)}`);
    const artists = info.communityArtists ?? [];
    const named = (role) => [
      ...new Set(
        artists.filter((artist) => artist.artistTypeList?.includes(role)).map((artist) => artist.name?.trim() ?? "").filter(Boolean)
      )
    ];
    const tags = info.curationTagList ?? [];
    const genres = [
      ...tags.filter((tag) => tag.curationType?.startsWith("GENRE_")),
      ...tags.filter((tag) => !tag.curationType?.startsWith("GENRE_"))
    ].map((tag) => tag.tagName?.trim() ?? "").filter(Boolean).slice(0, 10);
    const status = info.finished ? "completed" : info.rest ? "hiatus" : "ongoing";
    const cover = info.posterThumbnailUrl ?? info.thumbnailUrl;
    return {
      id,
      title: info.titleName?.trim() || "Untitled",
      coverUrl: cover?.startsWith("https://") ? cover : null,
      coverHeaders: HEADERS,
      authors: [...named("ARTIST_WRITER"), ...named("ARTIST_NOVEL_ORIGIN")],
      artists: named("ARTIST_PAINTER"),
      description: info.synopsis?.trim() || null,
      // The app hides titles with an adult genre while NSFW content is off.
      genres: [.../* @__PURE__ */ new Set([...genres, ...info.age?.type === "RATE_18" ? ["Adult"] : []])],
      status,
      url: `${SITE}/webtoon/list?titleId=${id}`
    };
  };
  var dateOf = (text) => {
    const day = /^(\d{2})\.(\d{2})\.(\d{2})$/.exec(text?.trim() ?? "");
    return day ? Date.UTC(2e3 + Number(day[1]), Number(day[2]) - 1, Number(day[3])) : null;
  };
  var chapters = async (id) => {
    const series = titleNumber(id);
    const pageOf = (page) => api(`/article/list?titleId=${series}&page=${page}`).then(
      (list) => list.articleList ?? []
    );
    const first = await api(`/article/list?titleId=${series}&page=1`);
    const lists = [first.articleList ?? []];
    const last = Math.min(first.pageInfo?.totalPages ?? 1, MAX_EPISODE_PAGES);
    for (let start = 2; start <= last; start += PARALLEL_PAGES) {
      const batch = [];
      for (let page = start; page < start + PARALLEL_PAGES && page <= last; page += 1) {
        batch.push(pageOf(page));
      }
      lists.push(...await Promise.all(batch));
    }
    const seen = /* @__PURE__ */ new Set();
    return lists.flat().flatMap((article) => {
      if (!Number.isSafeInteger(article.no) || article.charge || seen.has(article.no)) return [];
      seen.add(article.no);
      return [
        {
          id: `${series}:${article.no}`,
          name: article.subtitle?.trim() || null,
          number: article.no,
          dateUpload: dateOf(article.serviceDateDescription)
        }
      ];
    });
  };
  var pages = async (chapterId) => {
    const [series = "", episode = ""] = chapterId.split(":");
    if (!/^\d+$/.test(series) || !/^\d+$/.test(episode)) {
      throw new Error(`Naver Webtoon has no episode ${chapterId}`);
    }
    let html;
    try {
      const response = await raygun.http.fetch(
        `${SITE}/webtoon/detail?titleId=${series}&no=${episode}`,
        { headers: HEADERS }
      );
      if (!response.ok) throw new Error(`Naver Webtoon answered HTTP ${response.status}`);
      html = await response.text();
    } catch (error) {
      if (String(error).includes("not permitted")) {
        throw new Error("Naver Webtoon shows this episode only to signed-in adults", {
          cause: error
        });
      }
      throw error;
    }
    const inViewer = raygun.html.select(html, "#sectionContWide img");
    const numbered = inViewer.filter(
      (image) => (image.attr("id") ?? "").startsWith("content_image_")
    );
    const images = (numbered.length > 0 ? numbered : inViewer).map((image) => image.attr("src") ?? "").filter((url) => url.startsWith("https://image-comic.pstatic.net/"));
    if (images.length === 0) {
      throw new Error("Naver Webtoon shows this episode only in its app or to signed-in readers");
    }
    return images.map((url) => ({ url, headers: HEADERS }));
  };
  registerProvider(
    defineProvider({
      kind: "manga.source",
      id: "naver",
      // Not Mihon's "Naver Webtoon": its backups name series by addresses whose id is a query
      // parameter, which a Mihon import can't carry over, so they're skipped rather than broken.
      name: "Naver Webtoon (Korean)",
      popular: ({ page }) => popular(page),
      latest: ({ page }) => latest(page),
      search: ({ query, page, filters: chosen }) => search(query, page, chosen),
      filters,
      details: ({ id }) => details(id),
      chapters: async ({ id }) => ({ chapters: await chapters(id) }),
      pages: async ({ chapterId }) => ({ pages: await pages(chapterId) })
    })
  );
})();
