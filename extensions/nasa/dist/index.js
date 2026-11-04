// dev.raygun.nasa 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
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
  var API = "https://images-api.nasa.gov";
  var ASSETS = "https://images-assets.nasa.gov";
  var PAGE_SIZE = 30;
  var ENCODES = [
    { name: "large", height: 1080 },
    { name: "medium", height: 720 },
    { name: "small", height: 360 },
    { name: "preview", height: 270 },
    { name: "mobile", height: 180 }
  ];
  var RIGHTS = /©|\(c\)\s*\d{4}|copyright|all rights reserved|getty images|reuters|associated press|\bap photo\b/i;
  var CENTERS = [
    { value: "all", label: "All centers" },
    { value: "HQ", label: "Headquarters" },
    { value: "ARC", label: "Ames" },
    { value: "AFRC", label: "Armstrong" },
    { value: "GRC", label: "Glenn" },
    { value: "GSFC", label: "Goddard" },
    { value: "JPL", label: "Jet Propulsion Laboratory" },
    { value: "JSC", label: "Johnson" },
    { value: "KSC", label: "Kennedy" },
    { value: "LaRC", label: "Langley" },
    { value: "MSFC", label: "Marshall" },
    { value: "SSC", label: "Stennis" }
  ];
  var DECADES = [
    { value: "any", label: "Any time" },
    { value: "2020", label: "2020s" },
    { value: "2010", label: "2010s" },
    { value: "2000", label: "2000s" },
    { value: "1990", label: "1990s" },
    { value: "1980", label: "1980s" },
    { value: "1970", label: "1970s" },
    { value: "1960", label: "1960s" }
  ];
  var assetUrl = (href) => {
    const path = href.replace(/^https?:\/\/[^/]+\//, "");
    return `${ASSETS}/${path.split("/").map(encodeURIComponent).join("/")}`;
  };
  var plainText = (html) => html.replace(/<br\s*\/?>/gi, "\n").replace(/<[^>]+>/g, "").replace(/&nbsp;/g, " ").replace(/&quot;/g, '"').replace(/&#39;|&apos;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&").replace(/[^\S\n]+/g, " ").trim();
  var tidyTitle = (title) => {
    const spaced = title.includes(" ") ? title : title.replace(/_/g, " ");
    const tidy = spaced.replace(/^[A-Z]{2,6}-\d{8}-[A-Za-z0-9]+-\d{4}-\s*/, "").replace(/^[a-z]{2,6}\d{4}m\d+[\s_-]*/i, "").replace(/[\s_-]+(?:1080p?|720p?|480p?|4k|uhd|hd|cc)$/i, "").trim();
    return tidy || title.trim();
  };
  var publicDomain = (data) => !RIGHTS.test([data.title, data.description, data.photographer, data.secondary_creator].join(" "));
  var coverOf = (item) => {
    const pictures = (item.links ?? []).filter(
      (link) => link.render === "image" && (link.width ?? 0) <= 800
    );
    const best = pictures.sort((a, b) => (b.width ?? 0) - (a.width ?? 0))[0];
    return best ? assetUrl(best.href) : null;
  };
  var summaryOf = (item) => {
    const data = item.data?.[0];
    if (!data?.nasa_id || data.media_type !== "video" || !publicDomain(data)) return [];
    return [
      { id: data.nasa_id, title: tidyTitle(data.title ?? data.nasa_id), coverUrl: coverOf(item) }
    ];
  };
  var search = async (params, page, { newestFirst = false } = {}) => {
    const query = Object.entries({
      ...params,
      media_type: "video",
      page_size: PAGE_SIZE,
      page: Math.max(1, page)
    }).map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`).join("&");
    const answer = await fetchJson(`${API}/search?${query}`);
    const items = [...answer.collection?.items ?? []];
    if (newestFirst) {
      const date = (item) => item.data?.[0]?.date_created ?? "";
      items.sort((a, b) => date(b).localeCompare(date(a)));
    }
    const total = answer.collection?.metadata?.total_hits ?? 0;
    return {
      items: items.flatMap(summaryOf),
      hasNextPage: items.length > 0 && ((answer.collection?.links ?? []).some((link) => link.rel === "next") || page * PAGE_SIZE < total)
    };
  };
  var pick = (filters2, key, options) => {
    const value = filters2?.[key];
    return typeof value === "string" && options.some((option) => option.value === value) ? value : null;
  };
  var lookup = async (id) => {
    const answer = await fetchJson(`${API}/search?nasa_id=${encodeURIComponent(id)}`);
    const item = answer.collection?.items?.find((found) => found.data?.[0]?.nasa_id === id);
    if (!item?.data?.[0]) throw new Error("This video is no longer in NASA’s library");
    return item;
  };
  var originalHeight = (metadata) => {
    if (!metadata) return void 0;
    const frame = metadata["XMP:VideoFrameSize"];
    const height = Number(
      frame?.H ?? metadata["QuickTime:ImageHeight"] ?? metadata["Track1:ImageHeight"]
    );
    return Number.isFinite(height) && height > 0 ? height : void 0;
  };
  var videosOf = async (id) => {
    const [assets, metadata] = await Promise.all([
      fetchJson(`${API}/asset/${encodeURIComponent(id)}`),
      fetchJson(`${ASSETS}/video/${encodeURIComponent(id)}/metadata.json`).catch(
        () => null
      )
    ]);
    const files = (assets.collection?.items ?? []).map((item) => item.href);
    const captions = files.filter((href) => /\.(vtt|srt)$/i.test(href));
    captions.sort((a, b) => Number(/\.srt$/i.test(a)) - Number(/\.srt$/i.test(b)));
    const subtitles = captions.slice(0, 1).map((href) => ({
      url: assetUrl(href),
      lang: "en",
      label: "English"
    }));
    const videos = [];
    for (const encode of ENCODES) {
      const href = files.find((file) => file.endsWith(`~${encode.name}.mp4`));
      if (href)
        videos.push({
          url: assetUrl(href),
          label: `${encode.height}p`,
          quality: encode.height,
          subtitles
        });
    }
    const original = files.find((file) => /~orig\.mp4$/i.test(file));
    if (original) {
      const height = originalHeight(metadata);
      videos.push({
        url: assetUrl(original),
        label: height ? `Original · ${height}p` : "Original",
        quality: height,
        subtitles
      });
    }
    if (videos.length === 0) throw new Error("NASA’s library has no MP4 of this video");
    return videos;
  };
  var filters = [
    { kind: "select", key: "center", label: "Center", options: CENTERS },
    { kind: "select", key: "decade", label: "Decade", options: DECADES }
  ];
  registerProvider(
    defineProvider({
      kind: "video.source",
      id: "videos",
      name: "NASA",
      // The library's own list of its most viewed media, videos among them.
      async popular({ page }) {
        if (page > 1) return { items: [], hasNextPage: false };
        const answer = await fetchJson(`${ASSETS}/popular.json`);
        return { items: (answer.collection?.items ?? []).flatMap(summaryOf), hasNextPage: false };
      },
      latest: ({ page }) => search({ year_start: (/* @__PURE__ */ new Date()).getUTCFullYear() - 1 }, page, { newestFirst: true }),
      search: ({ query, page, filters: values }) => {
        const words = query.trim();
        const center = pick(values, "center", CENTERS);
        const decade = pick(values, "decade", DECADES);
        const params = {};
        if (words) params.q = words;
        if (center && center !== "all") params.center = center;
        if (decade && decade !== "any") {
          params.year_start = Number(decade);
          params.year_end = Number(decade) + 9;
        }
        if (Object.keys(params).length === 0) params.q = "nasa";
        return search(params, page);
      },
      filters: async () => filters,
      async details({ id }) {
        const item = await lookup(id);
        const data = item.data[0];
        const year = Number.parseInt((data.date_created ?? "").slice(0, 4), 10);
        const center = CENTERS.find((option) => option.value === data.center)?.label;
        const credit = data.photographer ?? data.secondary_creator;
        return {
          id,
          title: tidyTitle(data.title ?? id),
          coverUrl: coverOf(item),
          description: [
            data.description ? plainText(data.description) : "",
            [center && `NASA ${center}`, credit && credit !== "NASA" && `By ${credit}`].filter(Boolean).join(" · ")
          ].filter(Boolean).join("\n\n") || null,
          genres: (data.keywords ?? []).slice(0, 8),
          directors: credit && credit !== "NASA" ? [credit] : [],
          type: "other",
          year: Number.isFinite(year) ? year : null,
          status: "completed",
          url: `https://images.nasa.gov/details/${encodeURIComponent(id)}`
        };
      },
      episodes: async ({ id }) => ({ episodes: [{ id, name: "Video" }] }),
      // One server, resolved at once: the library serves every encode itself.
      servers: async ({ episodeId }) => ({
        servers: [{ id: episodeId, name: "NASA", videos: await videosOf(episodeId) }]
      }),
      videos: async ({ serverId }) => ({ videos: await videosOf(serverId) })
    })
  );
})();
