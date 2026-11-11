// dev.raygun.peertube 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
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
    const [, scheme = "", host = "", path = "", search2 = ""] = parts;
    if (link.startsWith("//")) return `${scheme}${link}`;
    const origin = `${scheme}//${host}`;
    if (link === "") return `${origin}${path || "/"}${search2}`;
    if (link.startsWith("#")) return `${origin}${path || "/"}${search2}${link}`;
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
  var SEARCH = "https://sepiasearch.org/api/v1/search/videos";
  var COUNT = 24;
  var COMMENTS = 20;
  var LICENCES = {
    1: "Creative Commons Attribution (CC BY)",
    2: "Creative Commons Attribution-ShareAlike (CC BY-SA)",
    3: "Creative Commons Attribution-NoDerivatives (CC BY-ND)",
    4: "Creative Commons Attribution-NonCommercial (CC BY-NC)",
    5: "Creative Commons Attribution-NonCommercial-ShareAlike (CC BY-NC-SA)",
    6: "Creative Commons Attribution-NonCommercial-NoDerivatives (CC BY-NC-ND)",
    7: "Public domain dedication (CC0)"
  };
  var LICENCE = {
    kind: "select",
    key: "licence",
    label: "Licence",
    default: "open",
    options: [
      { value: "open", label: "Any open licence" },
      { value: "commercial", label: "Allows commercial use" },
      { value: "changes", label: "Allows changes" },
      { value: "public", label: "Public domain" }
    ]
  };
  var LICENCE_IDS = {
    open: [1, 2, 3, 4, 5, 6, 7],
    commercial: [1, 2, 3, 7],
    changes: [1, 2, 4, 5, 7],
    public: [7]
  };
  var SORT = {
    kind: "select",
    key: "sort",
    label: "Sort by",
    default: "-match",
    options: [
      { value: "-match", label: "Best match" },
      { value: "-hot", label: "Trending" },
      { value: "-publishedAt", label: "Newest" },
      { value: "-views", label: "Most viewed" },
      { value: "-likes", label: "Most liked" }
    ]
  };
  var CATEGORY = {
    kind: "select",
    key: "category",
    label: "Category",
    default: "any",
    options: [
      { value: "any", label: "Any" },
      { value: "1", label: "Music" },
      { value: "2", label: "Films" },
      { value: "3", label: "Vehicles" },
      { value: "4", label: "Art" },
      { value: "5", label: "Sports" },
      { value: "6", label: "Travels" },
      { value: "7", label: "Gaming" },
      { value: "8", label: "People" },
      { value: "9", label: "Comedy" },
      { value: "10", label: "Entertainment" },
      { value: "11", label: "News & Politics" },
      { value: "12", label: "How To" },
      { value: "13", label: "Education" },
      { value: "14", label: "Activism" },
      { value: "15", label: "Science & Technology" },
      { value: "16", label: "Animals" },
      { value: "17", label: "Kids" },
      { value: "18", label: "Food" }
    ]
  };
  var LANGUAGE = {
    kind: "select",
    key: "language",
    label: "Language",
    default: "any",
    options: [
      { value: "any", label: "Any" },
      { value: "en", label: "English" },
      { value: "fr", label: "French" },
      { value: "de", label: "German" },
      { value: "es", label: "Spanish" },
      { value: "it", label: "Italian" },
      { value: "pt", label: "Portuguese" },
      { value: "nl", label: "Dutch" },
      { value: "ca", label: "Catalan" },
      { value: "eo", label: "Esperanto" },
      { value: "pl", label: "Polish" },
      { value: "cs", label: "Czech" },
      { value: "sv", label: "Swedish" },
      { value: "fi", label: "Finnish" },
      { value: "ru", label: "Russian" },
      { value: "ja", label: "Japanese" },
      { value: "zh", label: "Chinese" },
      { value: "ar", label: "Arabic" }
    ]
  };
  var DURATION = {
    kind: "select",
    key: "duration",
    label: "Length",
    default: "any",
    options: [
      { value: "any", label: "Any" },
      { value: "short", label: "Under 4 minutes" },
      { value: "medium", label: "4 to 20 minutes" },
      { value: "long", label: "Over 20 minutes" }
    ]
  };
  var DURATIONS = {
    short: { durationMax: 239 },
    medium: { durationMin: 240, durationMax: 1200 },
    long: { durationMin: 1201 }
  };
  var MONTHS = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December"
  ];
  var WATCH = /^(https?:\/\/[^/?#]+)\/(?:videos\/watch|w)\/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\/?$/i;
  var parseId = (id) => {
    const parts = WATCH.exec(id.trim());
    if (!parts) throw new Error("PeerTube has no such video");
    return { origin: parts[1], uuid: parts[2].toLowerCase() };
  };
  var idOf = (origin, uuid) => `${origin}/videos/watch/${uuid}`;
  var isOpen = (licence) => typeof licence?.id === "number" && licence.id in LICENCES;
  var decodeEntities = (text) => text.replace(/&#(\d+);/g, (_match, code) => String.fromCodePoint(Number(code))).replace(
    /&#x([0-9a-f]+);/gi,
    (_match, code) => String.fromCodePoint(Number.parseInt(code, 16))
  ).replace(/&nbsp;/g, " ").replace(/&quot;/g, '"').replace(/&#39;|&apos;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&");
  var plainText = (html) => decodeEntities(
    html.replace(/<br\s*\/?>/gi, "\n").replace(/<\/p>\s*<p[^>]*>/gi, "\n\n").replace(/<[^>]*>/g, "")
  ).replace(/[ \t]+/g, " ").replace(/\n{3,}/g, "\n\n").trim();
  var fromMarkdown = (text) => text.replace(/\r\n?/g, "\n").replace(/!\[[^\]]*\]\([^)]*\)/g, "").replace(/\[([^\]]+)\]\([^)]*\)/g, "$1").replace(/(\*\*|__)(.+?)\1/g, "$2").replace(/(^|[\s(])[*_]([^*_\n]+)[*_](?=$|[\s).,;:!?])/gm, "$1$2").replace(/^#{1,6}\s+/gm, "").replace(/^>\s?/gm, "").replace(/[ \t]+\n/g, "\n").replace(/\n{3,}/g, "\n\n").trim();
  var clock = (seconds) => {
    const whole = Math.max(0, Math.round(seconds));
    const hours = Math.floor(whole / 3600);
    const minutes = Math.floor(whole % 3600 / 60);
    const rest = String(whole % 60).padStart(2, "0");
    return hours > 0 ? `${hours}:${String(minutes).padStart(2, "0")}:${rest}` : `${minutes}:${rest}`;
  };
  var dayOf = (iso) => {
    const time = iso ? Date.parse(iso) : NaN;
    if (!Number.isFinite(time)) return null;
    const date = new Date(time);
    return {
      time,
      text: `${date.getUTCDate()} ${MONTHS[date.getUTCMonth()]} ${date.getUTCFullYear()}`,
      iso: date.toISOString().slice(0, 10),
      year: date.getUTCFullYear()
    };
  };
  var byline = (item) => {
    const channel = item.channel?.displayName || item.channel?.name;
    const account = item.account?.displayName || item.account?.name;
    const handle = item.channel?.name && item.channel.host ? `${item.channel.name}@${item.channel.host}` : null;
    const named = [channel, handle ? `(${handle})` : null].filter(Boolean).join(" ");
    return [named, account && account !== channel ? `by ${account}` : null].filter(Boolean).join(", ");
  };
  var absolute = (origin, path) => path ? resolveUrl(`${origin}/`, path) : null;
  var searchPage = async (params, page) => {
    const at = Math.max(page, 1);
    const showNsfw = raygun.content.showNsfw();
    const answer = await fetchJson(
      `${SEARCH}?${query({
        ...params,
        start: (at - 1) * COUNT,
        count: COUNT,
        nsfw: showNsfw ? "both" : "false",
        isLive: "false"
      })}`
    );
    const items2 = (answer.data ?? []).flatMap((video2) => {
      const origin = /^(https?:\/\/[^/?#]+)\//.exec(video2.url ?? "")?.[1];
      const uuid = video2.uuid?.toLowerCase();
      if (!origin || !uuid || !video2.name || video2.isLive || !isOpen(video2.licence)) return [];
      if (video2.nsfw && !showNsfw) return [];
      const channel = video2.channel?.displayName || video2.channel?.name;
      return [
        {
          id: idOf(origin, uuid),
          title: video2.name,
          coverUrl: video2.previewUrl ?? video2.thumbnailUrl ?? null,
          authors: channel ? [channel] : [],
          nsfw: video2.nsfw === true
        }
      ];
    });
    return { items: items2, hasNextPage: at * COUNT < (answer.total ?? 0) };
  };
  var openLicences = LICENCE_IDS.open;
  var search = (words, page, chosen) => {
    const category = selectValue(chosen, CATEGORY);
    const language = selectValue(chosen, LANGUAGE);
    const trimmed = words.trim();
    return searchPage(
      {
        search: trimmed || void 0,
        sort: selectValue(chosen, SORT),
        licenceOneOf: LICENCE_IDS[selectValue(chosen, LICENCE)] ?? openLicences,
        categoryOneOf: category === "any" ? void 0 : category,
        languageOneOf: language === "any" ? void 0 : language,
        ...DURATIONS[selectValue(chosen, DURATION)] ?? {}
      },
      page
    );
  };
  var items = /* @__PURE__ */ new Map();
  var fetchItem = (id) => {
    let item = items.get(id);
    if (!item) {
      const { origin, uuid } = parseId(id);
      item = (async () => {
        const address = `${origin}/api/v1/videos/${uuid}`;
        const response = await raygun.http.fetch(address, {
          headers: { Accept: "application/json" }
        });
        if (response.status === 404) throw new Error("This video is no longer on its instance");
        if (!response.ok) throw new Error(`HTTP ${response.status} from ${address}`);
        return response.json();
      })();
      item.catch(() => items.delete(id));
      items.set(id, item);
      if (items.size > 20) items.delete(items.keys().next().value);
    }
    return item;
  };
  var video = async (id) => {
    const item = await fetchItem(id);
    if (item.privacy && item.privacy.id !== 1) throw new Error("This video is no longer public");
    if (!isOpen(item.licence)) {
      throw new Error(
        "This video's uploader doesn't publish it under an open licence, so Raygun doesn't play it"
      );
    }
    if (item.isLive)
      throw new Error("This is a live stream, and PeerTube's lives aren't listed here");
    if (item.nsfw && !raygun.content.showNsfw()) {
      throw new Error("This video is marked sensitive; it shows while NSFW content is on");
    }
    return item;
  };
  var details = async (id) => {
    const item = await video(id);
    const { origin } = parseId(id);
    const published = dayOf(item.originallyPublishedAt) ?? dayOf(item.publishedAt);
    const facts = [
      published ? `Published ${published.text}` : null,
      item.duration ? clock(item.duration) : null,
      item.category?.label ?? null,
      item.language?.id ? item.language.label : null
    ].filter(Boolean);
    const host = origin.replace(/^https?:\/\//, "");
    const text = fromMarkdown(item.description ?? item.truncatedDescription ?? "");
    const who = byline(item);
    return {
      id,
      title: item.name ?? "PeerTube video",
      coverUrl: absolute(origin, item.previewPath ?? item.thumbnailPath),
      description: [
        text,
        facts.join(" · "),
        who ? `Published on ${host} by ${who}.` : `Published on ${host}.`,
        `Licence: ${LICENCES[item.licence.id]}, as its uploader set it.`
      ].filter(Boolean).join("\n\n"),
      genres: [...new Set([item.category?.label ?? "", ...item.tags ?? []].filter(Boolean))].slice(
        0,
        8
      ),
      type: item.category?.id === 2 ? "movie" : "other",
      year: published && published.year >= 1800 && published.year <= 2200 ? published.year : null,
      status: "completed",
      url: id
    };
  };
  var captions = async (origin, uuid) => {
    try {
      const answer = await fetchJson(
        `${origin}/api/v1/videos/${uuid}/captions`
      );
      return (answer.data ?? []).flatMap((caption) => {
        const url = caption.fileUrl ?? absolute(origin, caption.captionPath);
        if (!url || !/^https?:\/\//.test(url)) return [];
        const name = caption.language?.label ?? caption.language?.id ?? "Captions";
        return [
          {
            url,
            lang: caption.language?.id,
            label: caption.automaticallyGenerated ? `${name} (automatic)` : name
          }
        ];
      });
    } catch {
      return [];
    }
  };
  var links = async (id) => {
    const item = await video(id);
    const { origin, uuid } = parseId(id);
    const subtitles = (await captions(origin, uuid)).slice(0, 50);
    const heights = (files2) => (files2 ?? []).map((file) => file.resolution?.id ?? 0).filter((height) => Number.isInteger(height) && height > 0);
    const found = [];
    for (const playlist of item.streamingPlaylists ?? []) {
      if (playlist.type !== 1 || !playlist.playlistUrl?.startsWith("http")) continue;
      const tallest = Math.max(0, ...heights(playlist.files));
      found.push({
        url: playlist.playlistUrl,
        format: "hls",
        label: tallest > 0 ? `${tallest}p (HLS)` : "HLS",
        quality: tallest > 0 ? tallest : void 0,
        subtitles
      });
    }
    const files = (item.files ?? []).filter((file) => file.fileUrl?.startsWith("http")).sort((a, b) => (b.resolution?.id ?? 0) - (a.resolution?.id ?? 0));
    for (const file of files) {
      const height = file.resolution?.id ?? 0;
      found.push({
        url: file.fileUrl,
        label: height > 0 ? `${height}p` : "Audio",
        quality: height > 0 ? height : void 0,
        subtitles
      });
    }
    if (found.length === 0) throw new Error("Its instance has no file of this video to play");
    return found.slice(0, 50);
  };
  var server = async (id) => ({
    id,
    name: parseId(id).origin.replace(/^https?:\/\//, ""),
    videos: await links(id)
  });
  var comments = async (id, page) => {
    await video(id);
    const { origin, uuid } = parseId(id);
    const start = Math.max(0, Number.parseInt(page ?? "0", 10) || 0);
    const answer = await fetchJson(
      `${origin}/api/v1/videos/${uuid}/comment-threads?${query({
        start,
        count: COMMENTS,
        sort: "-createdAt"
      })}`
    );
    const threads = answer.data ?? [];
    const list = threads.flatMap((thread) => {
      const author = thread.account?.displayName || thread.account?.name;
      const text = plainText(thread.text ?? "");
      if (thread.isDeleted || !author || !text) return [];
      const avatar = [...thread.account?.avatars ?? []].sort(
        (a, b) => (a.width ?? 0) - (b.width ?? 0)
      )[0];
      return [
        {
          author,
          text,
          avatarUrl: avatar?.fileUrl ?? absolute(origin, avatar?.path),
          published: dayOf(thread.createdAt)?.iso ?? null,
          replies: thread.totalReplies ?? null
        }
      ];
    });
    const next = start + threads.length;
    return {
      comments: list,
      next: threads.length > 0 && next < (answer.total ?? 0) ? String(next) : null
    };
  };
  registerProvider(
    defineProvider({
      kind: "video.source",
      id: "videos",
      name: "PeerTube",
      searchPill: "PeerTube",
      popular: ({ page }) => searchPage({ sort: "-hot", licenceOneOf: openLicences }, page),
      latest: ({ page }) => searchPage({ sort: "-publishedAt", licenceOneOf: openLicences }, page),
      filters: async () => [SORT, LICENCE, CATEGORY, LANGUAGE, DURATION],
      search: ({ query: words, page, filters: chosen }) => search(words, page, chosen),
      details: ({ id }) => details(id),
      episodes: async ({ id }) => {
        const item = await video(id);
        const published = dayOf(item.originallyPublishedAt) ?? dayOf(item.publishedAt);
        return {
          episodes: [
            {
              id,
              name: item.category?.id === 2 ? "Film" : "Video",
              number: 1,
              dateUpload: published?.time ?? null
            }
          ]
        };
      },
      servers: async ({ episodeId }) => ({ servers: [await server(episodeId)] }),
      videos: async ({ serverId }) => ({ videos: await links(serverId) }),
      comments: ({ id, page }) => comments(id, page)
    })
  );
})();
