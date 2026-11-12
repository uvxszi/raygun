// dev.raygun.nasa-svs 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };
  var fetchText = async (url, init) => {
    const response = await raygun.http.fetch(url, init);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status} from ${url}`);
    }
    return response.text();
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
  var SCHEME = /^[a-z][a-z0-9+.-]*:/i;
  var PARTS = /^([a-z][a-z0-9+.-]*:)\/\/([^/?#]*)([^?#]*)(\?[^#]*)?(#.*)?$/i;
  var normalizePath = (path) => {
    const segments = [];
    const parts = path.split("/");
    parts.forEach((part, index) => {
      const last = index === parts.length - 1;
      if (part === "..") {
        if (segments.length > 1) segments.pop();
        if (last) segments.push("");
      } else if (part === ".") {
        if (last) segments.push("");
      } else {
        segments.push(part);
      }
    });
    const joined = segments.join("/");
    return joined.startsWith("/") ? joined : `/${joined}`;
  };
  var resolveUrl = (base, href) => {
    const link = href.trim();
    if (SCHEME.test(link)) return link;
    const parts = PARTS.exec(base.trim());
    if (!parts) throw new Error(`resolveUrl needs an absolute base address, not ${base}`);
    const [, scheme = "", host = "", path = "", search = ""] = parts;
    if (link.startsWith("//")) return `${scheme}${link}`;
    const origin = `${scheme}//${host}`;
    if (link === "") return `${origin}${path || "/"}${search}`;
    if (link.startsWith("#")) return `${origin}${path || "/"}${search}${link}`;
    if (link.startsWith("?")) return `${origin}${path || "/"}${link}`;
    const cut = link.search(/[?#]/);
    const [linkPath, rest] = cut < 0 ? [link, ""] : [link.slice(0, cut), link.slice(cut)];
    const directory = path.slice(0, path.lastIndexOf("/") + 1) || "/";
    const joined = linkPath.startsWith("/") ? linkPath : `${directory}${linkPath}`;
    return `${origin}${normalizePath(joined)}${rest}`;
  };
  var selectValue = (values, filter) => {
    const chosen = values?.[filter.key];
    const known = (value) => typeof value === "string" && filter.options.some((option) => option.value === value);
    if (known(chosen)) return chosen;
    if (known(filter.default)) return filter.default;
    return filter.options[0]?.value ?? "";
  };

  // src/index.ts
  var SITE = "https://svs.gsfc.nasa.gov";
  var CARDS = 8;
  var select = (key, label, options) => ({
    kind: "select",
    key,
    label,
    options: options.map(([value, name]) => ({ value, label: name })),
    default: options[0][0]
  });
  var SORT = select("sort", "Sort by", [
    ["best", "Best match"],
    ["popularity", "Most viewed lately"],
    ["release_date", "Newest"]
  ]);
  var TYPE = select("type", "Type", [
    ["all", "All videos"],
    ["visualization", "Visualizations"],
    ["animation", "Animations"],
    ["produced_video", "Produced videos"],
    ["broll", "B-roll"]
  ]);
  var CATEGORY = select("category", "Science", [
    ["all", "All sciences"],
    ["Earth", "Earth"],
    ["Sun", "The Sun"],
    ["Planets & Moons", "Planets and moons"],
    ["Universe", "The universe"]
  ]);
  var STUDIO = select("studio", "Studio", [
    ["all", "All studios"],
    ["SVS", "Scientific Visualization Studio"],
    ["GMS", "Goddard Media Studios"],
    ["CIL", "Conceptual Image Lab"]
  ]);
  var MUSIC = /\bmusic\b|\bsoundtrack\b|\bcomposer\b|universal production|killer tracks|extreme music|audio network|associated production|musicbed|artlist|epidemic sound|premiumbeat|soundstripe|audiojungle|pond5/i;
  var LANGUAGES = {
    en: "English",
    es: "Spanish",
    fr: "French",
    de: "German",
    pt: "Portuguese",
    it: "Italian",
    ja: "Japanese",
    zh: "Chinese",
    ko: "Korean"
  };
  var https = (url) => (url ?? "").replace(/^http:\/\//i, "https://");
  var plain = (html) => (html ?? "").replace(/<br\s*\/?>|<\/p>|<\/li>/gi, "\n").replace(/<[^>]*>/g, "").replace(/&#(\d+);/g, (_match, code) => String.fromCodePoint(Number(code))).replace(/&nbsp;/g, " ").replace(/&quot;/g, '"').replace(/&#39;|&apos;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&").replace(/[ \t]+/g, " ").replace(/\s*\n\s*/g, "\n").replace(/\n{2,}/g, "\n\n").trim();
  var oneLine = (html) => plain(html).replace(/\s+/g, " ").trim();
  var cardsOf = (html) => [...html.matchAll(/<li[^>]*id="card_search_result_(\d+)"[\s\S]*?<\/li>/g)].flatMap((card) => {
    const body = card[0];
    const id = card[1];
    const title = oneLine(/<h3[^>]*>([\s\S]*?)<\/h3>/.exec(body)?.[1] ?? "");
    const picture = /<img[^>]*\ssrc="([^"]+)"/.exec(body)?.[1];
    const type = oneLine(/bi-[a-z-]+[^"]*"[^>]*><\/span>([^<]*)<\/span>/.exec(body)?.[1] ?? "");
    if (!title) return [];
    return [
      {
        id,
        title,
        coverUrl: picture ? https(resolveUrl(SITE, picture)) : null,
        subtitle: type || null
      }
    ];
  });
  var refused = null;
  var refusedPages = () => {
    if (!refused) {
      let saved = null;
      try {
        saved = raygun.storage.get("refused");
      } catch {
      }
      refused = new Set(Array.isArray(saved) ? saved : []);
    }
    return refused;
  };
  var refuse = (id) => {
    const pages2 = refusedPages();
    if (pages2.has(id)) return;
    pages2.add(id);
    try {
      raygun.storage.set("refused", [...pages2].slice(-500));
    } catch {
    }
  };
  var listPage = async (params, page) => {
    const html = await fetchText(
      `${SITE}/api/search/results/list?${query({
        ...params,
        media: "movies",
        sort_by_0: "DESC",
        chunk: Math.max(1, page) - 1
      })}`
    );
    const cards = cardsOf(html);
    const skip = refusedPages();
    return {
      items: cards.filter((card) => !skip.has(card.id)),
      hasNextPage: cards.length >= CARDS
    };
  };
  var pages = /* @__PURE__ */ new Map();
  var pageOf = (id) => {
    if (!/^\d+$/.test(id)) return Promise.reject(new Error("The SVS has no such page"));
    let page = pages.get(id);
    if (!page) {
      page = fetchJson(`${SITE}/api/${id}/`);
      page.catch(() => pages.delete(id));
      pages.set(id, page);
      if (pages.size > 60) pages.delete(pages.keys().next().value);
    }
    return page;
  };
  var wordsOf = (page) => [
    page.description,
    ...(page.media_groups ?? []).flatMap((group) => [
      group.title,
      group.caption,
      group.description
    ]),
    ...(page.credits ?? []).map((credit) => credit.role)
  ].map((text) => plain(text)).join("\n");
  var openPage = async (id) => {
    const page = await pageOf(id);
    if (MUSIC.test(wordsOf(page))) {
      refuse(id);
      throw new Error(
        "This SVS video has licensed music, which isn't in the public domain, so Raygun doesn't offer it"
      );
    }
    return page;
  };
  var filesOf = (group) => (group.items ?? []).flatMap((item) => item.instance ? [item.instance] : []);
  var isMovie = (file) => file.media_type === "Movie" && /\.(?:mp4|m4v|webm|mov)$/i.test(file.filename ?? "");
  var videoGroups = (page) => (page.media_groups ?? []).filter(
    (group) => /video/i.test(group.widget ?? "") && filesOf(group).some(isMovie)
  );
  var videosOf = (group) => {
    const files = filesOf(group);
    const subtitles = files.filter((file) => /\.(?:vtt|srt)$/i.test(file.filename ?? "")).sort(
      (one, other) => Number(/\.srt$/i.test(one.filename)) - Number(/\.srt$/i.test(other.filename))
    ).flatMap((file) => {
      const lang = /\.([a-z]{2})(?:[_-][A-Za-z]{2})?\.(?:vtt|srt)$/i.exec(file.filename)?.[1];
      return [{ url: https(file.url), lang: lang ?? "en", label: LANGUAGES[lang ?? "en"] ?? lang }];
    }).filter(
      (subtitle, index, all) => all.findIndex((each) => each.lang === subtitle.lang) === index
    );
    return files.filter(isMovie).map((file) => ({
      url: https(file.url),
      height: file.height ?? 0,
      webm: /\.webm$/i.test(file.filename ?? "")
    })).filter((file) => file.url.startsWith("https://")).sort((one, other) => other.height - one.height || Number(one.webm) - Number(other.webm)).map((file) => ({
      url: file.url,
      label: file.height ? `${file.height}p${file.webm ? " WebM" : ""}` : "Video",
      ...file.height ? { quality: file.height } : {},
      subtitles
    }));
  };
  var episodeId = (page, group) => `${page}:${group}`;
  var episodeVideos = async (id) => {
    const [pageId, groupId] = id.split(":");
    const page = await openPage(pageId ?? "");
    const group = videoGroups(page).find((each) => String(each.id) === groupId);
    if (!group) throw new Error("This SVS video is no longer on its page");
    const videos = videosOf(group);
    if (videos.length === 0) throw new Error("This SVS video has no file Raygun can play");
    return videos;
  };
  registerProvider(
    defineProvider({
      kind: "video.source",
      id: "videos",
      name: "NASA Scientific Visualization Studio",
      popular: ({ page }) => listPage({ sort_by_1: "popularity" }, page),
      latest: ({ page }) => listPage({ sort_by_1: "release_date" }, page),
      search({ query: typed, page, filters }) {
        const words = typed.trim();
        const sort = selectValue(filters, SORT);
        const params = {
          sort_by_1: sort === "best" ? words ? "relevance" : "popularity" : sort
        };
        if (words) params.search = words;
        const type = selectValue(filters, TYPE);
        if (type !== "all") params.result_type = type;
        const category = selectValue(filters, CATEGORY);
        if (category !== "all") params.nasa_science_categories = category;
        const studio = selectValue(filters, STUDIO);
        if (studio !== "all") params.studio = studio;
        return listPage(params, page);
      },
      filters: async () => [SORT, TYPE, CATEGORY, STUDIO],
      async details({ id }) {
        const page = await openPage(id);
        const year = Number((page.release_date ?? "").slice(0, 4));
        const people = (page.credits ?? []).map(
          (credit) => `${credit.role ?? "Credit"}: ${(credit.people ?? []).map((person) => person.name).filter(Boolean).join(", ")}`
        );
        return {
          id,
          title: page.title?.trim() || `SVS page ${id}`,
          coverUrl: page.main_image?.url ? https(page.main_image.url) : null,
          description: [
            plain(page.description),
            people.join("\n"),
            "Public domain (NASA media): https://svs.gsfc.nasa.gov/help/"
          ].filter(Boolean).join("\n\n"),
          genres: [
            .../* @__PURE__ */ new Set([
              ...page.nasa_science_categories ?? [],
              ...page.missions ?? [],
              ...page.keywords ?? []
            ])
          ].slice(0, 8),
          type: "other",
          year: year >= 1800 && year <= 2200 ? year : null,
          status: "completed",
          url: `${SITE}/${id}/`
        };
      },
      async episodes({ id }) {
        const page = await openPage(id);
        const groups = videoGroups(page);
        if (groups.length === 0) throw new Error("This SVS page has no video Raygun can play");
        const released = Date.parse(page.release_date ?? "");
        return {
          episodes: groups.map((group, index) => ({
            id: episodeId(id, group.id),
            name: plain(group.title) || plain(group.caption).slice(0, 80) || (groups.length === 1 ? "Video" : `Video ${index + 1}`),
            number: index + 1,
            dateUpload: Number.isFinite(released) ? released : null
          }))
        };
      },
      // One server, resolved at once: the site serves every file itself.
      servers: async ({ episodeId: id }) => ({
        servers: [{ id, name: "NASA SVS", videos: await episodeVideos(id) }]
      }),
      videos: async ({ serverId }) => ({ videos: await episodeVideos(serverId) })
    })
  );
})();
