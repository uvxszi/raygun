// dev.raygun.youtube 2.4.0. Built by raygun-ext from src/index.ts; do not edit.
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
    const answer = await response.json();
    rememberVisitor(answer.responseContext?.visitorData);
    return answer;
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
  var PLAYER_CLIENTS = [
    {
      name: "VISIONOS",
      id: 101,
      version: "1.04",
      userAgent: "com.google.visionos.youtube/1.04 (RealityDevice17,1; U; CPU visionOS 26_6_0 like Mac OS X; US) gzip",
      context: {
        deviceMake: "Apple",
        deviceModel: "RealityDevice17,1",
        osName: "visionOS",
        osVersion: "26.6.0"
      }
    },
    {
      name: "ANDROID_VR",
      id: 28,
      version: "1.65.10",
      userAgent: "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; en_US; Quest 3 Build/SQ3A.220605.009.A1) gzip",
      context: {
        deviceMake: "Oculus",
        deviceModel: "Quest 3",
        osName: "Android",
        osVersion: "12L",
        androidSdkVersion: 32,
        platform: "MOBILE"
      }
    },
    {
      name: "IOS",
      id: 5,
      version: "20.10.4",
      userAgent: "com.google.ios.youtube/20.10.4 (iPhone16,2; U; CPU iOS 18_3_2 like Mac OS X;)",
      context: {
        deviceMake: "Apple",
        deviceModel: "iPhone16,2",
        osName: "iPhone",
        osVersion: "18.3.2.22D82"
      }
    },
    {
      name: "ANDROID",
      id: 3,
      version: "20.10.38",
      userAgent: "com.google.android.youtube/20.10.38 (Linux; U; Android 11) gzip",
      context: { osName: "Android", osVersion: "11", androidSdkVersion: 30, platform: "MOBILE" }
    }
  ];
  var AUDIO_PREFIX = "audio:";
  var VISITOR_TTL_MS = 12 * 60 * 60 * 1e3;
  var FAILURES_TO_REST = 3;
  var REST_MS = 10 * 60 * 1e3;
  var visitor = null;
  var preferredClient = 0;
  var failuresInARow = 0;
  var restingUntil = 0;
  var isVisitor = (value) => typeof value === "string" && /^Cg[\w%-]{12,}/.test(value);
  var rememberVisitor = (value) => {
    if (!isVisitor(value) || visitor?.value === value) return;
    visitor = { value, at: Date.now() };
    try {
      raygun.storage.set("visitor", visitor);
    } catch {
    }
  };
  var findVisitor = (value, depth = 0) => {
    if (isVisitor(value)) return value;
    if (depth > 8 || value === null || typeof value !== "object") return null;
    for (const child of Object.values(value)) {
      const found = findVisitor(child, depth + 1);
      if (found) return found;
    }
    return null;
  };
  var visitorData = async () => {
    if (!visitor) {
      try {
        const stored = raygun.storage.get("visitor");
        if (stored && isVisitor(stored.value)) visitor = stored;
      } catch {
      }
    }
    if (visitor && Date.now() - visitor.at < VISITOR_TTL_MS) return visitor.value;
    const response = await raygun.http.fetch(`${WEB}/sw.js_data`, {
      headers: { "User-Agent": USER_AGENT }
    });
    if (response.ok) {
      const text = await response.text();
      const start = text.indexOf("[");
      if (start >= 0) {
        const data = JSON.parse(text.slice(start));
        const known = data?.[0]?.[2]?.[0]?.[0]?.[13];
        rememberVisitor(isVisitor(known) ? known : findVisitor(data));
      }
    }
    return visitor?.value ?? null;
  };
  var askPlayer = async (client, videoId, visitorValue) => {
    const response = await raygun.http.fetch(`${WEB}/youtubei/v1/player?prettyPrint=false`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "User-Agent": client.userAgent,
        "X-Goog-Api-Format-Version": "2",
        "X-YouTube-Client-Name": String(client.id),
        "X-YouTube-Client-Version": client.version,
        ...visitorValue ? { "X-Goog-Visitor-Id": visitorValue } : {}
      },
      body: JSON.stringify({
        context: {
          client: {
            clientName: client.name,
            clientVersion: client.version,
            ...client.context,
            hl: "en",
            gl: region() || "US",
            utcOffsetMinutes: 0,
            ...visitorValue ? { visitorData: visitorValue } : {}
          },
          user: { lockedSafetyMode: false }
        },
        videoId,
        contentCheckOk: true,
        racyCheckOk: true
      })
    });
    if (response.status === 429) {
      throw new Error("YouTube is limiting requests from this network; try again in a while");
    }
    if (!response.ok) throw new Error(`YouTube answered HTTP ${response.status}`);
    return response.json();
  };
  var kbpsOf = (format) => Math.round((format.averageBitrate ?? format.bitrate ?? 0) / 1e3);
  var isAac = (format) => format.mimeType.startsWith("audio/mp4");
  var distance = (kbps, target) => Math.max(kbps, target, 1) * 100 / Math.max(Math.min(kbps, target), 1) - 100;
  var pickFormat = (formats, targetKbps, decodable) => {
    const audio = formats.filter((format) => format.mimeType.startsWith("audio/") && format.url);
    const regular = audio.filter(
      (format) => !format.isDrc && format.audioTrack?.audioIsDefault !== false
    );
    const usable = regular.length > 0 ? regular : audio;
    const aac = usable.filter(isAac);
    const choices = decodable && aac.length > 0 ? aac : usable;
    const score = (format) => (typeof targetKbps === "number" && targetKbps > 0 ? -distance(kbpsOf(format), targetKbps) * 1e3 : kbpsOf(format) * 1e3) + (isAac(format) === decodable ? 1 : 0);
    return choices.reduce(
      (best, format) => best === null || score(format) > score(best) ? format : best,
      null
    );
  };
  var expiryOf = (answer, url) => {
    const seconds = Number(answer.streamingData?.expiresInSeconds);
    if (Number.isFinite(seconds) && seconds > 0) return Date.now() + seconds * 1e3;
    const expire = Number(/[?&]expire=(\d+)/.exec(url)?.[1]);
    return Number.isFinite(expire) && expire > 0 ? expire * 1e3 : null;
  };
  var streams = async (url, userAgent) => {
    try {
      const response = await raygun.http.fetch(url, {
        headers: { Range: "bytes=0-1", "User-Agent": userAgent }
      });
      return response.ok;
    } catch {
      return false;
    }
  };
  var unplayable = (answer) => {
    const status = answer.playabilityStatus?.status;
    const reason = answer.playabilityStatus?.reason?.trim();
    if (status === "LOGIN_REQUIRED" && reason && /age/i.test(reason)) {
      return "YouTube wants a sign-in to play it (age-restricted)";
    }
    if (status === "UNPLAYABLE" || status === "ERROR") {
      return reason ? `YouTube can't play it: ${reason}` : "YouTube can't play it";
    }
    return null;
  };
  var audioStream = async (videoId, args) => {
    if (!/^[\w-]{11}$/.test(videoId)) throw new Error("Not a YouTube video");
    if (Date.now() < restingUntil) throw new Error("YouTube isn't giving streams right now");
    const visitorValue = await visitorData().catch(() => null);
    const order = [
      PLAYER_CLIENTS[preferredClient],
      ...PLAYER_CLIENTS.filter((_, index) => index !== preferredClient)
    ].filter((client) => client !== void 0);
    let refusal = null;
    for (const client of order) {
      const answer = await askPlayer(client, videoId, visitorValue).catch(() => null);
      if (!answer) continue;
      const why = unplayable(answer);
      if (why) {
        refusal ??= why;
        if (answer.playabilityStatus?.status === "ERROR") break;
        continue;
      }
      if (answer.playabilityStatus?.status !== "OK") continue;
      const format = pickFormat(
        answer.streamingData?.adaptiveFormats ?? [],
        args.targetKbps,
        args.decodable === true
      );
      if (!format?.url || !await streams(format.url, client.userAgent)) continue;
      preferredClient = PLAYER_CLIENTS.indexOf(client);
      failuresInARow = 0;
      return {
        source: { kind: "url", url: format.url, headers: { "User-Agent": client.userAgent } },
        expiresAt: expiryOf(answer, format.url)
      };
    }
    if (refusal) throw new Error(refusal);
    failuresInARow += 1;
    if (failuresInARow >= FAILURES_TO_REST) {
      failuresInARow = 0;
      restingUntil = Date.now() + REST_MS;
    }
    throw new Error("YouTube didn't give a stream for it");
  };
  var streamProvider = defineProvider({
    kind: "music.stream",
    id: "stream",
    name: "YouTube",
    // Each video twice: its audio file (resolved when it's tried), then its embed, the fallback.
    async candidates({ title, artist, limit }) {
      if (!title.trim()) return { candidates: [] };
      const videos2 = await searchVideos(
        [artist, title].filter(Boolean).join(" "),
        Math.min(limit, 5)
      );
      const named = (video) => ({
        title: video.title,
        artist: video.artist,
        durationMs: video.durationMs,
        quality: "YouTube"
      });
      return {
        candidates: videos2.flatMap((video) => [
          { id: `${AUDIO_PREFIX}${video.videoId}`, ...named(video) },
          { id: video.videoId, ...named(video), source: embed(video.videoId) }
        ])
      };
    },
    resolve: async (args) => {
      const videoId = args.id.startsWith(AUDIO_PREFIX) ? args.id.slice(AUDIO_PREFIX.length) : args.id;
      return audioStream(videoId, args);
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
  var PREFIX = { channel: "channel:", playlist: "playlist:" };
  var imageUrl = (url) => {
    if (!url) return null;
    const full = url.startsWith("//") ? `https:${url}` : url;
    return full.includes("ytimg.com") ? full.split("?")[0] ?? full : full;
  };
  var lockupImage = (lockup) => imageUrl(
    (lockup.contentImage?.thumbnailViewModel ?? lockup.contentImage?.collectionThumbnailViewModel?.primaryThumbnail?.thumbnailViewModel)?.image?.sources?.at(-1)?.url
  );
  var entriesIn = (items, videosOnly = false, out = []) => {
    for (const item of items ?? []) {
      const video = item.videoRenderer;
      if (video?.videoId) {
        out.push({
          id: video.videoId,
          title: textOf(video.title),
          coverUrl: imageUrl(video.thumbnail?.thumbnails?.at(-1)?.url)
        });
      }
      const channel = item.channelRenderer;
      if (channel?.channelId && !videosOnly) {
        out.push({
          id: `${PREFIX.channel}${channel.channelId}`,
          title: textOf(channel.title),
          coverUrl: imageUrl(channel.thumbnail?.thumbnails?.at(-1)?.url)
        });
      }
      const lockup = item.lockupViewModel;
      if (lockup?.contentId) {
        const title = lockup.metadata?.lockupMetadataViewModel?.title?.content ?? "";
        if (lockup.contentType === "LOCKUP_CONTENT_TYPE_VIDEO") {
          out.push({ id: lockup.contentId, title, coverUrl: lockupImage(lockup) });
        } else if (lockup.contentType === "LOCKUP_CONTENT_TYPE_PLAYLIST" && !videosOnly) {
          out.push({
            id: `${PREFIX.playlist}${lockup.contentId}`,
            title,
            coverUrl: lockupImage(lockup)
          });
        }
      }
      if (item.itemSectionRenderer) entriesIn(item.itemSectionRenderer.contents, videosOnly, out);
      if (item.shelfRenderer) {
        entriesIn(item.shelfRenderer.content?.expandedShelfContentsRenderer?.items, videosOnly, out);
      }
      if (item.richItemRenderer?.content) entriesIn([item.richItemRenderer.content], videosOnly, out);
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
    items: found,
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
    return toPage(entriesIn(items), next);
  };
  var CHANNEL_VIDEOS = "EgZ2aWRlb3PyBgQKAjoA";
  var browsePage = (id) => id.startsWith(PREFIX.channel) ? webInnertube("browse", {
    browseId: id.slice(PREFIX.channel.length),
    params: CHANNEL_VIDEOS
  }) : webInnertube("browse", {
    browseId: `VL${id.slice(PREFIX.playlist.length)}`
  });
  var browseVideos = (page) => {
    const tabs = page.contents?.twoColumnBrowseResultsRenderer?.tabs ?? [];
    const tab = tabs.find((each) => each.tabRenderer?.selected) ?? tabs[0];
    const content = tab?.tabRenderer?.content;
    return entriesIn(
      content?.richGridRenderer?.contents ?? content?.sectionListRenderer?.contents,
      true
    );
  };
  var isCollection = (id) => id.startsWith(PREFIX.channel) || id.startsWith(PREFIX.playlist);
  var tokenOf = (item) => item?.continuationItemRenderer?.continuationEndpoint?.continuationCommand?.token ?? null;
  var videoComments = async (id, page) => {
    if (isCollection(id)) return { comments: [], next: null };
    let token = page ?? null;
    if (!token) {
      const watch = await webInnertube("next", { videoId: id });
      const sections = watch.contents?.twoColumnWatchNextResults?.results?.results?.contents ?? [];
      const section = sections.find(
        (each) => each.itemSectionRenderer?.targetId === "comments-section"
      );
      token = tokenOf(section?.itemSectionRenderer?.contents?.[0]);
      if (!token) return { comments: [], next: null };
    }
    const answer = await webInnertube("next", { continuation: token });
    const items = (answer.onResponseReceivedEndpoints ?? []).flatMap(
      (endpoint) => (endpoint.reloadContinuationItemsCommand ?? endpoint.appendContinuationItemsAction)?.continuationItems ?? []
    );
    const payloads = /* @__PURE__ */ new Map();
    for (const mutation of answer.frameworkUpdates?.entityBatchUpdate?.mutations ?? []) {
      const payload = mutation.payload?.commentEntityPayload;
      if (mutation.entityKey && payload) payloads.set(mutation.entityKey, payload);
    }
    const comments = items.flatMap((item) => {
      const key = item.commentThreadRenderer?.commentViewModel?.commentViewModel?.commentKey;
      const comment = key ? payloads.get(key) : void 0;
      if (!comment) return [];
      const replies = Number.parseInt(comment.toolbar?.replyCount ?? "", 10);
      return [
        {
          author: comment.author?.displayName ?? "",
          avatarUrl: comment.author?.avatarThumbnailUrl ?? null,
          text: comment.properties?.content?.content ?? "",
          likes: comment.toolbar?.likeCountNotliked?.trim() || null,
          published: comment.properties?.publishedTime ?? null,
          replies: Number.isFinite(replies) ? replies : null
        }
      ];
    });
    return { comments, next: tokenOf(items.at(-1)) };
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
      if (isCollection(id)) {
        const page = await browsePage(id);
        const channel = page.metadata?.channelMetadataRenderer;
        const playlist = page.metadata?.playlistMetadataRenderer;
        const own = id.startsWith(PREFIX.channel) ? `${WEB}/channel/${id.slice(PREFIX.channel.length)}` : `${WEB}/playlist?list=${id.slice(PREFIX.playlist.length)}`;
        return {
          id,
          title: channel?.title ?? playlist?.title ?? id,
          coverUrl: imageUrl(channel?.avatar?.thumbnails?.at(-1)?.url) ?? browseVideos(page)[0]?.coverUrl ?? null,
          description: channel?.description ?? playlist?.description ?? null,
          genres: [],
          type: "series",
          url: own
        };
      }
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
    episodes: async ({ id }) => {
      if (!isCollection(id)) return { episodes: [{ id, name: "Video" }] };
      const videos2 = browseVideos(await browsePage(id));
      return {
        episodes: videos2.map((video, index) => ({
          id: video.id,
          name: video.title,
          number: index + 1
        }))
      };
    },
    comments: ({ id, page }) => videoComments(id, page),
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
  registerProvider(streamProvider);
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
