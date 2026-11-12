// dev.raygun.eso 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
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

  // src/pages.ts
  var ID = /^[A-Za-z0-9_-]+$/;
  var decode = (text) => text.replace(
    /&#x([0-9a-f]+);/gi,
    (_, hex) => String.fromCodePoint(Number.parseInt(hex, 16))
  ).replace(/&#(\d+);/g, (_, code) => String.fromCodePoint(Number(code))).replace(/&nbsp;/g, " ").replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&");
  var lines = (html) => decode(
    html.replace(/<br\s*\/?>/gi, "\n").replace(/<\/(?:p|div|li|h\d)>/gi, "\n").replace(/<[^>]*>/g, "")
  ).split("\n").map((line) => line.replace(/\s+/g, " ").trim()).filter(Boolean);
  var videoId = (address) => /^\/public\/videos\/([^/?#]+)\/$/.exec(address ?? "")?.[1] ?? null;
  var thumbnails = (html, base) => {
    const seen = /* @__PURE__ */ new Set();
    return raygun.html.select(html, "a[data-url]").flatMap((link) => {
      const id = videoId(link.attr("data-url"));
      const title = decode(link.attr("data-title") ?? "").replace(/\s+/g, " ").trim();
      if (!id || !ID.test(id) || !title || seen.has(id)) return [];
      seen.add(id);
      const picture = link.select("img")[0]?.attr("src") ?? "";
      return [{ id, title, coverUrl: picture ? resolveUrl(base, picture) : null }];
    });
  };
  var tableRows = (html, base) => {
    const seen = /* @__PURE__ */ new Set();
    return raygun.html.select(html, "td.imagerow a[href]").flatMap((link) => {
      const id = videoId(link.attr("href"));
      const picture = link.select("img")[0];
      const title = decode(picture?.attr("alt") ?? "").replace(/\s+/g, " ").trim();
      if (!id || !ID.test(id) || !title || seen.has(id)) return [];
      seen.add(id);
      const src = picture?.attr("src") ?? "";
      return [{ id, title, coverUrl: src ? resolveUrl(base, src) : null }];
    });
  };
  var hasPageAfter = (html, page) => new RegExp(`/(?:list|page)/${page + 1}/`).test(html);
  var videoPage = (html, base, id) => {
    const title = decode(raygun.html.select(html, "h1")[0]?.text ?? "").replace(/\s+/g, " ").trim();
    if (!title || /^not found$/i.test(title)) return null;
    const start = html.indexOf("</h1>");
    const creditAt = html.indexOf('<div class="credit">');
    const between = creditAt > start ? html.slice(start, creditAt) : "";
    const description = raygun.html.select(between, "p").map((paragraph) => decode(paragraph.text).replace(/\s+/g, " ").trim()).filter(
      (text, index, all) => text && !/^(?:learn more about\b|members of the media\b|more information\b)/i.test(text) && all.indexOf(text) === index
    ).join("\n\n");
    const credit = lines(raygun.html.select(html, "div.credit")[0]?.inner ?? "");
    const facts = {};
    for (const row of raygun.html.select(html, "tr")) {
      const cells = /<t[hd][^>]*>([\s\S]*?)<\/t[hd]>\s*<td[^>]*>([\s\S]*?)<\/td>/i.exec(row.inner);
      if (!cells) continue;
      const name = lines(cells[1] ?? "").join(" ").replace(/:$/, "");
      if (name) facts[name] = lines(cells[2] ?? "").join(" ");
    }
    const files = raygun.html.select(html, "span.archive_dl_text a[href]").flatMap((link) => {
      const url = resolveUrl(base, link.attr("href") ?? "");
      const folder = /\/videos\/([a-z0-9_]+)\/[^/]+$/i.exec(url)?.[1];
      return folder ? [{ url, folder, name: decode(link.text).replace(/\s+/g, " ").trim() }] : [];
    });
    const subtitles = files.flatMap((file) => {
      const name = file.url.split("/").pop() ?? "";
      if (file.folder !== "srt" || !name.startsWith(id) || !/\.srt$/i.test(name)) return [];
      const lang = name.slice(id.length, -".srt".length).replace(/^[-_.]/, "");
      return lang ? [{ url: file.url, lang }] : [];
    });
    const og = /<meta\s+property="og:image(?::secure_url)?"\s+content="(https?:\/\/[^"]+)"/i.exec(
      html
    )?.[1];
    const picture = og ? og.replace(/^http:\/\//i, "https://") : null;
    return { title, description, credit, facts, files, subtitles, picture };
  };

  // src/index.ts
  var AGENT = "Raygun/1.0 ( https://github.com/uvxszi/raygun )";
  var categories = (options) => ({
    kind: "select",
    key: "category",
    label: "Category",
    options: [
      { value: "all", label: "All videos" },
      ...options.map(([value, label]) => ({ value, label }))
    ],
    default: "all"
  });
  var ESO = {
    id: "eso",
    name: "ESO",
    base: "https://www.eso.org",
    paging: "list",
    viewAll: tableRows,
    categories: categories([
      ["esocast", "ESOcast"],
      ["chasingstarlight", "Chasing Starlight"],
      ["vnr", "Video News Releases"],
      ["featuremovies", "Feature Movies"],
      ["fulldome", "Fulldome"],
      ["galaxies", "Galaxies"],
      ["galaxyclusters", "Galaxy Clusters"],
      ["nebulae", "Nebulae"],
      ["stars", "Stars"],
      ["starclusters", "Star Clusters"],
      ["exoplanets", "Exoplanets"],
      ["solarsystem", "Solar System"],
      ["blackholes", "Quasars and Black Holes"],
      ["cosmology", "Cosmology"],
      ["paranal", "Paranal"],
      ["lasilla", "La Silla"],
      ["alma", "ALMA"],
      ["apex", "APEX"],
      ["elt", "ELT"],
      ["abouteso", "About ESO"],
      ["events", "Events"],
      ["premises", "Premises"]
    ])
  };
  var NOIRLAB = {
    id: "noirlab",
    name: "NOIRLab",
    base: "https://noirlab.edu",
    paging: "page",
    viewAll: thumbnails,
    categories: categories([
      ["features", "Feature Films"],
      ["video-news-release", "Video News Release"],
      ["cosmoviews", "Cosmoviews"],
      ["fulldome", "Fulldome"],
      ["galaxies", "Galaxies"],
      ["galaxyclusters", "Galaxy Clusters"],
      ["nebulae", "Nebulae"],
      ["stars", "Stars"],
      ["starclusters", "Star Clusters"],
      ["exoplanets", "Exoplanets"],
      ["solarsystem", "Solar System"],
      ["blackholes", "Quasars and Black Holes"],
      ["cosmology", "Cosmology"],
      ["illustrations", "Illustrations"],
      ["gemini", "Gemini Observatory"],
      ["kpno", "Kitt Peak National Observatory"],
      ["ctio", "Cerro Tololo"],
      ["rubin", "Vera C. Rubin Observatory"],
      ["sos", "Science on a Sphere"],
      ["virtual-reality", "Virtual Reality"],
      ["peopleandevents", "People and Events"]
    ])
  };
  var FORMATS = [
    { folder: "ultra_hd", height: 2160, label: "4K" },
    { folder: "qhd_1440p25_screen", height: 1440, label: "1440p" },
    { folder: "hd_1080p25_screen", height: 1080, label: "1080p" },
    { folder: "hd_1080_screen", height: 1080, label: "1080p" },
    { folder: "dome_preview", height: 1024, label: "Fulldome preview" },
    { folder: "hd_and_apple", height: 720, label: "720p" },
    { folder: "large_qt", height: 405, label: "405p", quickTime: true },
    { folder: "medium_podcast", height: 360, label: "360p" },
    { folder: "small_qt", height: 180, label: "180p", quickTime: true }
  ];
  var OTHER_LICENCE = /©|\(c\)\s*(?:19|20)\d\d|all rights reserved|\bcopyright(?:ed)?\b|\bpermission\b|non-?commercial|no-?derivatives?|\bCC[\s-]*BY[\s-]*(?:NC|ND)\b|creativecommons\.org\/licenses\/by-n[cd]|not (?:be )?(?:used|reproduced|released under)/i;
  var LANGUAGES = {
    ar: "Arabic",
    cs: "Czech",
    da: "Danish",
    de: "German",
    el: "Greek",
    en: "English",
    es: "Spanish",
    fi: "Finnish",
    fr: "French",
    it: "Italian",
    ja: "Japanese",
    ko: "Korean",
    nl: "Dutch",
    pl: "Polish",
    pt: "Portuguese",
    ru: "Russian",
    sv: "Swedish",
    tr: "Turkish",
    zh: "Chinese"
  };
  var remember = (map, key, value, most) => {
    map.delete(key);
    map.set(key, value);
    if (map.size > most) map.delete(map.keys().next().value);
  };
  var lists = /* @__PURE__ */ new Map();
  var videos = /* @__PURE__ */ new Map();
  var refusals = /* @__PURE__ */ new Map();
  var refusedOn = (site) => {
    let refused = refusals.get(site.id);
    if (!refused) {
      let saved = null;
      try {
        saved = raygun.storage.get(`refused.${site.id}`);
      } catch {
      }
      refused = new Set(Array.isArray(saved) ? saved : []);
      refusals.set(site.id, refused);
    }
    return refused;
  };
  var refuse = (site, id) => {
    const refused = refusedOn(site);
    if (refused.has(id)) return;
    refused.add(id);
    try {
      raygun.storage.set(`refused.${site.id}`, [...refused].slice(-500));
    } catch {
    }
  };
  var fetchPage = (url) => fetchText(url, { headers: { "User-Agent": AGENT, Accept: "text/html" } });
  var listUrl = (site, path, page, search = "") => `${site.base}/public/videos/${path}${page > 1 ? `${site.paging}/${page}/` : ""}${search}`;
  var listPage = async (site, url, page, read) => {
    let listed = lists.get(url);
    if (!listed) {
      const html = await fetchPage(url);
      const found = read(html, site.base);
      listed = {
        items: found.map((video) => ({ id: video.id, title: video.title, coverUrl: video.coverUrl })),
        hasNextPage: found.length > 0 && hasPageAfter(html, page)
      };
      remember(lists, url, listed, 40);
    }
    const refused = refusedOn(site);
    return { ...listed, items: listed.items.filter((item) => !refused.has(item.id)) };
  };
  var videoOf = async (site, id) => {
    if (!/^[A-Za-z0-9_-]+$/.test(id)) throw new Error(`${site.name} has no video ${id}`);
    const key = `${site.id}:${id}`;
    let video = videos.get(key);
    if (!video) {
      const url = `${site.base}/public/videos/${id}/`;
      const response = await raygun.http.fetch(url, {
        headers: { "User-Agent": AGENT, Accept: "text/html" }
      });
      if (response.status === 404) throw new Error(`This video is no longer on ${site.name}'s site`);
      if (!response.ok) throw new Error(`${site.name}'s site answered HTTP ${response.status}`);
      const read = videoPage(await response.text(), site.base, id);
      if (!read) throw new Error(`${site.name}'s page for this video couldn't be read`);
      video = read;
    }
    remember(videos, key, video, 60);
    if (OTHER_LICENCE.test([video.description, ...video.credit].join("\n"))) {
      refuse(site, id);
      throw new Error(
        `${site.name} names a licence other than CC BY for part of this video, so Raygun doesn't offer it`
      );
    }
    return video;
  };
  var playable = (video) => {
    const known = video.files.flatMap((file) => {
      const format = FORMATS.find((candidate) => candidate.folder === file.folder);
      if (!format || !/\.(?:mp4|m4v|mov)$/i.test(file.url)) return [];
      return [{ ...file, height: format.height, label: format.label, quickTime: format.quickTime }];
    });
    const modern = known.filter((file) => !file.quickTime);
    return (modern.length > 0 ? modern : known).sort((a, b) => b.height - a.height);
  };
  var clockOf = (duration) => {
    const part = (unit) => Number(new RegExp(`(\\d+)\\s*${unit}\\b`).exec(duration ?? "")?.[1] ?? 0);
    const [hours, minutes, seconds] = [part("h"), part("m"), part("s")];
    if (hours + minutes + seconds === 0) return "";
    const rest = String(seconds).padStart(2, "0");
    return hours > 0 ? `${hours}:${String(minutes).padStart(2, "0")}:${rest}` : `${minutes}:${rest}`;
  };
  var MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
  var releasedAt = (date) => {
    const text = (date ?? "").toLowerCase();
    const dayFirst = /(\d{1,2})\s+([a-z]+)\.?\s+(\d{4})/.exec(text);
    const monthFirst = /([a-z]+)\.?\s+(\d{1,2}),\s*(\d{4})/.exec(text);
    const [day, month, year] = dayFirst ? [dayFirst[1], dayFirst[2], dayFirst[3]] : monthFirst ? [monthFirst[2], monthFirst[1], monthFirst[3]] : [];
    const index = MONTHS.indexOf((month ?? "").slice(0, 3));
    if (!day || !year || index < 0) return null;
    const clock = /(\d{1,2})(?::(\d{2}))?\s*(a\.m\.|p\.m\.)?\s*$/.exec(text.split(",").pop() ?? "");
    let hours = Number(clock?.[1] ?? 0);
    if (clock?.[3] === "p.m." && hours < 12) hours += 12;
    if (clock?.[3] === "a.m." && hours === 12) hours = 0;
    const minutes = Number(clock?.[2] ?? 0);
    return Date.UTC(Number(year), index, Number(day), clock ? hours : 0, minutes);
  };
  var descriptionOf = (site, video) => {
    const facts = [
      video.facts["Release date"] ? `Released ${video.facts["Release date"]}` : "",
      clockOf(video.facts.Duration)
    ].filter(Boolean);
    const music = video.credit.some((line) => /\bmusic\s*:/i.test(line));
    return [
      video.description,
      facts.join(" · "),
      `Credit: ${video.credit.join("; ") || site.name}`,
      `Licence: CC BY 4.0 (${site.base}/public/copyright/).${music ? ` The music isn't part of that licence: ${site.name} lets it be used only in its own videos, which play here as it published them.` : ""}`
    ].filter(Boolean).join("\n\n");
  };
  var videosOf = async (site, id) => {
    const video = await videoOf(site, id);
    const subtitles = video.subtitles.map(({ url, lang }) => ({
      url,
      lang,
      label: LANGUAGES[lang.toLowerCase()] ?? lang
    }));
    const files = playable(video);
    if (files.length === 0) throw new Error(`${site.name} has no file of this video Raygun can play`);
    return files.map((file) => ({
      url: file.url,
      label: file.label,
      quality: file.height,
      subtitles
    }));
  };
  var providerOf = (site) => defineProvider({
    kind: "video.source",
    id: site.id,
    name: site.name,
    // The site's own order: its newest and featured videos first.
    popular: ({ page }) => listPage(site, listUrl(site, "", page), page, thumbnails),
    latest: ({ page }) => listPage(site, listUrl(site, "viewall/", page, "?sort=-release_date"), page, site.viewAll),
    search({ query, page, filters }) {
      const category = selectValue(filters, site.categories);
      const words = query.trim();
      const path = category === "all" ? "" : `archive/category/${category}/`;
      const search = words ? `?search=${encodeURIComponent(words)}` : "";
      return listPage(site, listUrl(site, path, page, search), page, thumbnails);
    },
    filters: async () => [site.categories],
    async details({ id }) {
      const video = await videoOf(site, id);
      const released = releasedAt(video.facts["Release date"]);
      return {
        id,
        title: video.title,
        coverUrl: video.picture,
        description: descriptionOf(site, video),
        type: "other",
        year: released === null ? null : new Date(released).getUTCFullYear(),
        status: "completed",
        url: `${site.base}/public/videos/${encodeURIComponent(id)}/`
      };
    },
    async episodes({ id }) {
      const video = await videoOf(site, id);
      if (playable(video).length === 0) {
        throw new Error(`${site.name} has no file of this video Raygun can play`);
      }
      return {
        episodes: [{ id, name: "Video", dateUpload: releasedAt(video.facts["Release date"]) }]
      };
    },
    // One server, resolved at once: the site serves every file itself.
    servers: async ({ episodeId }) => ({
      servers: [{ id: episodeId, name: site.name, videos: await videosOf(site, episodeId) }]
    }),
    videos: async ({ serverId }) => ({ videos: await videosOf(site, serverId) })
  });
  registerProvider(providerOf(ESO));
  registerProvider(providerOf(NOIRLAB));
})();
