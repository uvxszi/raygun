// dev.raygun.youtube 2.0.0. Built by raygun-ext from src/index.ts; do not edit.
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
      artworkUrl: thumbnail ? largeThumbnail(thumbnail) : null
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
    const videos = await searchMusic(query, "videos").catch(() => []);
    return [...songs, ...videos.filter((video) => !seen.has(video.videoId))].slice(0, limit);
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
    const videos = await call("videos", {
      part: "contentDetails",
      id: items.map((item) => item.id.videoId).join(",")
    });
    const lengths = new Map(
      (videos.items ?? []).map((video) => [video.id, isoDuration(video.contentDetails?.duration)])
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
  var search = defineProvider({
    kind: "music.metadata",
    id: "search",
    name: "YouTube",
    async search({ query, limit }) {
      if (!query.trim()) return { tracks: [] };
      const videos = await searchVideos(query, limit);
      const tracks = videos.map((video) => ({
        id: video.videoId,
        title: video.title,
        artist: video.artist,
        album: video.album,
        durationMs: video.durationMs,
        artworkUrl: video.artworkUrl,
        source: embed(video.videoId)
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
      const videos = await searchVideos(
        [artist, title].filter(Boolean).join(" "),
        Math.min(limit, 5)
      );
      return {
        candidates: videos.map((video) => ({
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
  registerProvider(search);
  registerProvider(streams);
})();
