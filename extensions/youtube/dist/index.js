// dev.raygun.youtube 2.2.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var channelArtist = (channel) => channel.replace(/\s*-\s*Topic$/, "");
  var MUSIC = "https://music.youtube.com";
  var FALLBACK_CLIENT_VERSION = "1.20261027.10.00";
  var USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";
  var FILTER = {
    songs: "EgWKAQIIAWoKEAkQBRAKEAMQBA==",
    videos: "EgWKAQIQAWoKEAkQChAFEAMQBA=="
  };
  var clientVersion = FALLBACK_CLIENT_VERSION;
  var refreshClientVersion = async () => {
    const page = await raygun.http.fetch(`${MUSIC}/`, { headers: { "User-Agent": USER_AGENT } });
    const version = /"INNERTUBE_CLIENT_VERSION":"([\d.]+)"/.exec(await page.text())?.[1];
    if (!version || version === clientVersion) return false;
    clientVersion = version;
    return true;
  };
  var region = () => (raygun.settings.get("region") ?? "").trim().toUpperCase();
  var innertube = async (endpoint, body) => {
    const send = () => raygun.http.fetch(`${MUSIC}/youtubei/v1/${endpoint}?prettyPrint=false`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "User-Agent": USER_AGENT,
        Origin: MUSIC,
        Referer: `${MUSIC}/`,
        "X-YouTube-Client-Name": "67",
        "X-YouTube-Client-Version": clientVersion
      },
      body: JSON.stringify({
        context: {
          client: {
            clientName: "WEB_REMIX",
            clientVersion,
            hl: "en",
            ...region() ? { gl: region() } : {}
          }
        },
        ...body
      })
    });
    let response = await send();
    if (response.status === 400 && await refreshClientVersion().catch(() => false)) {
      response = await send();
    }
    if (response.status === 429) {
      throw new Error("YouTube is limiting requests from this network; try again in a while");
    }
    if (!response.ok) throw new Error(`YouTube Music answered HTTP ${response.status}`);
    return response.json();
  };
  var pageType = (run) => run.navigationEndpoint?.browseEndpoint?.browseEndpointContextSupportedConfigs?.browseEndpointContextMusicConfig?.pageType;
  var TIME = /^(?:(\d+):)?(\d{1,2}):(\d{2})$/;
  var clockDuration = (text) => {
    const match = TIME.exec(text.trim());
    if (!match) return null;
    const [, hours = "0", minutes = "0", seconds = "0"] = match;
    return ((Number(hours) * 60 + Number(minutes)) * 60 + Number(seconds)) * 1e3;
  };
  var largeThumbnail = (url) => url.replace(/=w\d+-h\d+/, "=w544-h544");
  var parseItem = (item) => {
    const columns = (item.flexColumns ?? []).map(
      (column) => column.musicResponsiveListItemFlexColumnRenderer?.text?.runs ?? []
    );
    const titleRun = columns[0]?.[0];
    const videoId = item.playlistItemData?.videoId ?? titleRun?.navigationEndpoint?.watchEndpoint?.videoId;
    if (!titleRun || !videoId) return null;
    const details = columns[1] ?? [];
    const artists = details.filter((run) => pageType(run) === "MUSIC_PAGE_TYPE_ARTIST");
    const uploader = details[0]?.text;
    const artist = artists.length > 0 ? artists.map((run) => run.text).join(", ") : uploader && !TIME.test(uploader) ? uploader : "";
    const album = details.find((run) => pageType(run) === "MUSIC_PAGE_TYPE_ALBUM")?.text ?? null;
    const duration = details.map((run) => clockDuration(run.text)).find((ms) => ms !== null);
    const thumbnails = item.thumbnail?.musicThumbnailRenderer?.thumbnail?.thumbnails ?? [];
    const thumbnail = thumbnails.at(-1)?.url;
    return {
      videoId,
      title: titleRun.text,
      artist: channelArtist(artist),
      album,
      durationMs: duration ?? null,
      artworkUrl: thumbnail ? largeThumbnail(thumbnail) : null,
      explicit: (item.badges ?? []).some(
        (badge) => badge.musicInlineBadgeRenderer?.icon?.iconType === "MUSIC_EXPLICIT_BADGE"
      )
    };
  };
  var searchMusic = async (query, filter) => {
    const response = await innertube("search", { query, params: FILTER[filter] });
    const sections = response.contents?.tabbedSearchResultsRenderer?.tabs?.[0]?.tabRenderer?.content?.sectionListRenderer?.contents ?? [];
    return sections.flatMap((section) => section.musicShelfRenderer?.contents ?? []).map(
      (entry) => entry.musicResponsiveListItemRenderer ? parseItem(entry.musicResponsiveListItemRenderer) : null
    ).filter((video) => video !== null);
  };
  var searchKeyless = async (query, limit) => {
    const songs = await searchMusic(query, "songs");
    if (songs.length >= limit) return songs.slice(0, limit);
    const seen = new Set(songs.map((song) => song.videoId));
    const videos2 = await searchMusic(query, "videos").catch(() => []);
    return [...songs, ...videos2.filter((video) => !seen.has(video.videoId))].slice(0, limit);
  };
  var API = "https://www.googleapis.com/youtube/v3";
  var MUSIC_CATEGORY = "10";
  var apiKey = () => (raygun.settings.get("apiKey") ?? "").trim();
  var call = async (path, params) => {
    const query = Object.entries({ ...params, key: apiKey() }).map(([name, value]) => `${name}=${encodeURIComponent(String(value))}`).join("&");
    const response = await raygun.http.fetch(`${API}/${path}?${query}`);
    if (response.ok) return response.json();
    const body = await response.json().catch(() => ({}));
    const reason = body.error?.errors?.[0]?.reason;
    if (reason === "quotaExceeded" || reason === "dailyLimitExceeded") {
      throw new Error(
        "Today's YouTube API quota is used up; it resets at midnight Pacific time. Clear the key to search without it"
      );
    }
    if (reason === "keyInvalid" || response.status === 400) {
      throw new Error("YouTube rejected the API key; check it in the extension settings or clear it");
    }
    throw new Error(body.error?.message ?? `YouTube answered HTTP ${response.status}`);
  };
  var ENTITIES = {
    "&amp;": "&",
    "&quot;": '"',
    "&#39;": "'",
    "&lt;": "<",
    "&gt;": ">"
  };
  var unescape = (text) => text.replace(/&(amp|quot|#39|lt|gt);/g, (entity) => ENTITIES[entity] ?? entity);
  var isoDuration = (value) => {
    const match = value?.match(/^PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?$/);
    if (!match) return null;
    const [, hours = "0", minutes = "0", seconds = "0"] = match;
    return ((Number(hours) * 60 + Number(minutes)) * 60 + Number(seconds)) * 1e3 || null;
  };
  var searchWithKey = async (q, limit) => {
    const search2 = await call("search", {
      part: "snippet",
      type: "video",
      videoCategoryId: MUSIC_CATEGORY,
      maxResults: Math.min(Math.max(limit, 1), 25),
      q,
      ...region() ? { regionCode: region() } : {}
    });
    const items = (search2.items ?? []).filter((item) => item.id.videoId);
    if (items.length === 0) return [];
    const videos2 = await call("videos", {
      part: "contentDetails",
      id: items.map((item) => item.id.videoId).join(",")
    });
    const lengths = new Map(
      (videos2.items ?? []).map((video) => [video.id, isoDuration(video.contentDetails?.duration)])
    );
    return items.map((item) => {
      const videoId = item.id.videoId;
      const thumbnails = item.snippet.thumbnails ?? {};
      return {
        videoId,
        title: unescape(item.snippet.title),
        artist: channelArtist(unescape(item.snippet.channelTitle)),
        album: null,
        durationMs: lengths.get(videoId) ?? null,
        artworkUrl: (thumbnails.high ?? thumbnails.medium ?? thumbnails.default)?.url ?? null
      };
    });
  };
  var searchVideos = (query, limit) => apiKey() ? searchWithKey(query, limit) : searchKeyless(query, Math.max(limit, 1));
  var embed = (videoId) => ({ kind: "youTube", videoId });
  var PLAYLIST_FILTER = {
    community: "EgeKAQQoAEABagoQAxAEEAoQCRAF",
    featured: "EgeKAQQoADgBagwQDhAKEAMQBRAJEAQ="
  };
  var listItems = (value, found = [], depth = 0) => {
    if (depth > 14 || value === null || typeof value !== "object") return found;
    if (Array.isArray(value)) {
      for (const item of value) listItems(item, found, depth + 1);
      return found;
    }
    for (const [key, child] of Object.entries(value)) {
      if (key === "musicResponsiveListItemRenderer") found.push(child);
      else listItems(child, found, depth + 1);
    }
    return found;
  };
  var findKey = (value, key, depth = 0) => {
    if (depth > 14 || value === null || typeof value !== "object") return null;
    if (Array.isArray(value)) {
      for (const item of value) {
        const found = findKey(item, key, depth + 1);
        if (found) return found;
      }
      return null;
    }
    const record = value;
    if (key in record && record[key] && typeof record[key] === "object") {
      return record[key];
    }
    for (const child of Object.values(record)) {
      const found = findKey(child, key, depth + 1);
      if (found) return found;
    }
    return null;
  };
  var runsText = (value) => (value?.runs ?? []).map((run) => run.text).join("");
  var thumbnailOf = (value) => {
    const thumbs = findKey(value, "musicThumbnailRenderer")?.thumbnail?.thumbnails ?? [];
    const url = thumbs.at(-1)?.url;
    return url ? largeThumbnail(url) : null;
  };
  var parsePlaylist = (item, featured) => {
    const browseId = item.navigationEndpoint?.browseEndpoint?.browseId;
    const columns = (item.flexColumns ?? []).map(
      (column) => column.musicResponsiveListItemFlexColumnRenderer?.text?.runs ?? []
    );
    const title = columns[0]?.[0]?.text;
    if (!browseId || !title) return null;
    const details = (columns[1] ?? []).map((run) => run.text.trim()).filter(Boolean);
    const parts = details.filter((part) => part !== "•" && part !== "Playlist");
    const count = parts.map((part) => /^([\d,.]+)\s+(songs?|tracks?)$/i.exec(part)).find(Boolean);
    return {
      id: browseId,
      title,
      author: parts.find((part) => !/views?$|songs?$|tracks?$/i.test(part)) ?? null,
      artworkUrl: thumbnailOf(item),
      trackCount: count ? Number(count[1]?.replace(/[,.]/g, "")) : null,
      featured
    };
  };
  var searchPlaylists = async (query, featured, limit) => {
    const response = await innertube("search", {
      query,
      params: featured ? PLAYLIST_FILTER.featured : PLAYLIST_FILTER.community
    });
    return listItems(response).map((item) => parsePlaylist(item, featured)).filter((playlist) => playlist !== null).slice(0, limit);
  };
  var playlistPage = async (id) => {
    const browseId = id.startsWith("VL") || id.startsWith("MPRE") ? id : `VL${id}`;
    const response = await innertube("browse", { browseId });
    const header = findKey(response, "musicResponsiveHeaderRenderer") ?? findKey(response, "musicDetailHeaderRenderer") ?? findKey(response, "musicEditablePlaylistDetailHeaderRenderer");
    const songs = listItems(response).map(parseItem).filter((video) => video !== null);
    const playlist = {
      id: browseId,
      title: runsText(header?.title) || "Playlist",
      author: runsText(header?.straplineTextOne) || runsText(header?.subtitle) || null,
      artworkUrl: thumbnailOf(header) ?? songs[0]?.artworkUrl ?? null,
      trackCount: songs.length,
      featured: false
    };
    return { playlist, songs };
  };
  var toTrack = (video) => ({
    id: video.videoId,
    title: video.title,
    artist: video.artist,
    album: video.album,
    durationMs: video.durationMs,
    artworkUrl: video.artworkUrl,
    source: embed(video.videoId),
    explicit: video.explicit ?? null
  });
  var simpleWords = (text) => text.toLowerCase().replace(/\(.*?\)|\[.*?\]/g, " ").replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  var youTubeLyrics = async (query) => {
    const words = [query.artist, query.title].filter(Boolean).join(" ");
    const songs = await searchMusic(words, "songs");
    const wanted = simpleWords(query.title);
    const song = songs.find((candidate) => simpleWords(candidate.title).includes(wanted));
    if (!song) return null;
    const next = await innertube("next", { videoId: song.videoId, isAudioOnly: true });
    const tabs = findKey(next, "watchNextTabbedResultsRenderer")?.tabs ?? [];
    const browseId = tabs.map((tab) => tab.tabRenderer?.endpoint?.browseEndpoint?.browseId).find((id) => id?.startsWith("MPLY"));
    if (!browseId) return null;
    const page = await innertube("browse", { browseId });
    const shelf = findKey(page, "musicDescriptionShelfRenderer");
    const text = runsText(shelf?.description).trim();
    return text ? { synced: [], plain: text } : null;
  };
  var search = defineProvider({
    kind: "music.metadata",
    id: "search",
    name: "YouTube",
    async searchPlaylists({ query, limit, featured }) {
      if (!query.trim()) return { playlists: [] };
      return { playlists: await searchPlaylists(query, featured, Math.max(limit, 1)) };
    },
    async playlist({ id }) {
      const { playlist, songs } = await playlistPage(id);
      return { playlist, tracks: songs.map(toTrack) };
    },
    async search({ query, limit }) {
      if (!query.trim()) return { tracks: [] };
      const videos2 = await searchVideos(query, limit);
      const tracks = videos2.map((video) => ({
        id: video.videoId,
        title: video.title,
        artist: video.artist,
        album: video.album,
        durationMs: video.durationMs,
        artworkUrl: video.artworkUrl,
        source: embed(video.videoId),
        explicit: video.explicit ?? null
      }));
      return { tracks };
    }
  });
  var streams = defineProvider({
    kind: "music.stream",
    id: "stream",
    name: "YouTube",
    async candidates({ title, artist, limit }) {
      if (!title.trim()) return { candidates: [] };
      const videos2 = await searchVideos(
        [artist, title].filter(Boolean).join(" "),
        Math.min(limit, 5)
      );
      return {
        candidates: videos2.map((video) => ({
          id: video.videoId,
          title: video.title,
          artist: video.artist,
          durationMs: video.durationMs,
          quality: "YouTube",
          source: embed(video.videoId)
        }))
      };
    }
  });
  var WEB = "https://www.youtube.com";
  var webClientVersion = "2.20261028.01.00";
  var textOf = (text) => text?.simpleText ?? text?.runs?.map((run) => run.text).join("") ?? "";
  var webInnertube = async (endpoint, body) => {
    const send = () => raygun.http.fetch(`${WEB}/youtubei/v1/${endpoint}?prettyPrint=false`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "User-Agent": USER_AGENT,
        Origin: WEB,
        Referer: `${WEB}/`,
        "X-YouTube-Client-Name": "1",
        "X-YouTube-Client-Version": webClientVersion
      },
      body: JSON.stringify({
        context: {
          client: {
            clientName: "WEB",
            clientVersion: webClientVersion,
            hl: "en",
            ...region() ? { gl: region() } : {}
          }
        },
        ...body
      })
    });
    let response = await send();
    if (response.status === 400) {
      const page = await raygun.http.fetch(`${WEB}/`, { headers: { "User-Agent": USER_AGENT } });
      const version = /"INNERTUBE_CLIENT_VERSION":"([\d.]+)"/.exec(await page.text())?.[1];
      if (version && version !== webClientVersion) {
        webClientVersion = version;
        response = await send();
      }
    }
    if (!response.ok) throw new Error(`YouTube answered ${response.status}`);
    return await response.json();
  };
  var videosIn = (items, out = []) => {
    for (const item of items ?? []) {
      if (item.videoRenderer?.videoId) out.push(item.videoRenderer);
      if (item.itemSectionRenderer) videosIn(item.itemSectionRenderer.contents, out);
      if (item.shelfRenderer)
        videosIn(item.shelfRenderer.content?.expandedShelfContentsRenderer?.items, out);
      if (item.richItemRenderer?.content) videosIn([item.richItemRenderer.content], out);
    }
    return out;
  };
  var continuationIn = (items) => {
    for (const item of items ?? []) {
      const token = item.continuationItemRenderer?.continuationEndpoint?.continuationCommand?.token;
      if (token) return token;
      const nested = continuationIn(item.itemSectionRenderer?.contents);
      if (nested) return nested;
    }
    return null;
  };
  var toPage = (found, next) => ({
    items: found.map((video) => ({
      id: video.videoId,
      title: textOf(video.title),
      coverUrl: video.thumbnail?.thumbnails?.at(-1)?.url?.split("?")[0] ?? null
    })),
    hasNextPage: next !== null
  });
  var pages = /* @__PURE__ */ new Map();
  var searchWeb = async (query, page) => {
    const key = (n) => `${query}|${n}`;
    let items;
    if (page > 1) {
      const token = pages.get(key(page));
      if (!token) return { items: [], hasNextPage: false };
      const response = await webInnertube("search", { continuation: token });
      items = response.onResponseReceivedCommands?.flatMap(
        (command) => command.appendContinuationItemsAction?.continuationItems ?? []
      );
    } else {
      const response = await webInnertube("search", { query });
      items = response.contents?.twoColumnSearchResultsRenderer?.primaryContents?.sectionListRenderer?.contents;
    }
    const next = continuationIn(items);
    if (next) pages.set(key(page + 1), next);
    if (pages.size > 60) pages.delete(pages.keys().next().value);
    return toPage(videosIn(items), next);
  };
  var videos = defineProvider({
    kind: "video.source",
    id: "videos",
    name: "YouTube",
    searchPill: "YouTube",
    // With nothing typed: what's trending.
    popular: ({ page }) => searchWeb("trending", page),
    latest: ({ page }) => searchWeb("live now", page),
    search: ({ query, page }) => searchWeb(query.trim() || "trending", page),
    async details({ id }) {
      const watch = `${WEB}/watch?v=${encodeURIComponent(id)}`;
      const response = await raygun.http.fetch(
        `${WEB}/oembed?format=json&url=${encodeURIComponent(watch)}`,
        { headers: { "User-Agent": USER_AGENT } }
      );
      const meta = response.ok ? await response.json() : {};
      return {
        id,
        title: meta.title ?? id,
        coverUrl: meta.thumbnail_url ?? `https://i.ytimg.com/vi/${id}/hqdefault.jpg`,
        description: meta.author_name ? `By ${meta.author_name}` : null,
        genres: [],
        type: "movie",
        url: watch
      };
    },
    episodes: async ({ id }) => ({ episodes: [{ id, name: "Video" }] }),
    // Played in YouTube's embed: one link, no stream extraction.
    servers: async ({ episodeId }) => ({
      servers: [
        {
          id: episodeId,
          name: "YouTube",
          videos: [{ url: `youtube:${episodeId}`, label: "YouTube" }]
        }
      ]
    }),
    videos: async ({ serverId }) => ({
      videos: [{ url: `youtube:${serverId}`, label: "YouTube" }]
    })
  });
  registerProvider(search);
  registerProvider(streams);
  registerProvider(videos);
  registerProvider(
    defineProvider({
      kind: "lyrics",
      id: "lyrics",
      name: "YouTube Music",
      lyrics: youTubeLyrics
    })
  );
})();
