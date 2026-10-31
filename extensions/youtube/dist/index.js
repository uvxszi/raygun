// dev.raygun.youtube 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var API = "https://www.googleapis.com/youtube/v3";
  var MUSIC_CATEGORY = "10";
  var apiKey = () => {
    const key = (raygun.settings.get("apiKey") ?? "").trim();
    if (!key) {
      throw new Error("Add your YouTube Data API key in Settings → Extensions → YouTube");
    }
    return key;
  };
  var call = async (path, params) => {
    const query = Object.entries({ ...params, key: apiKey() }).map(([name, value]) => `${name}=${encodeURIComponent(String(value))}`).join("&");
    const response = await raygun.http.fetch(`${API}/${path}?${query}`);
    if (response.ok) return response.json();
    const body = await response.json().catch(() => ({}));
    const reason = body.error?.errors?.[0]?.reason;
    if (reason === "quotaExceeded" || reason === "dailyLimitExceeded") {
      throw new Error("Today's YouTube API quota is used up; it resets at midnight Pacific time");
    }
    if (reason === "keyInvalid" || response.status === 400) {
      throw new Error("YouTube rejected the API key; check it in the extension settings");
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
  var channelArtist = (channel) => channel.replace(/\s*-\s*Topic$/, "");
  var searchVideos = async (q, limit) => {
    const region = (raygun.settings.get("region") ?? "").trim().toUpperCase();
    const search2 = await call("search", {
      part: "snippet",
      type: "video",
      videoCategoryId: MUSIC_CATEGORY,
      maxResults: Math.min(Math.max(limit, 1), 25),
      q,
      ...region ? { regionCode: region } : {}
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
        durationMs: lengths.get(videoId) ?? null,
        artworkUrl: (thumbnails.high ?? thumbnails.medium ?? thumbnails.default)?.url ?? null
      };
    });
  };
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
