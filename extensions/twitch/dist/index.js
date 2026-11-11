// dev.raygun.twitch 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };
  var needsSetup = (reason) => {
    const host = raygun;
    if (typeof host.needsSetup === "function") host.needsSetup(reason);
    throw new Error(reason);
  };
  var query = (params) => Object.entries(params).flatMap(([name, value]) => {
    if (value === void 0 || value === null) return [];
    const values = Array.isArray(value) ? value : [value];
    return values.map((item) => `${encodeURIComponent(name)}=${encodeURIComponent(item)}`);
  }).join("&");
  var toggleValue = (values, filter) => {
    const chosen2 = values?.[filter.key];
    return typeof chosen2 === "boolean" ? chosen2 : filter.default ?? false;
  };

  // src/helix.ts
  var API = "https://api.twitch.tv/helix";
  var TOKEN = "https://id.twitch.tv/oauth2/token";
  var SETUP = "Add the Client ID and Client Secret of an application you registered at dev.twitch.tv/console in this source's settings";
  var REFUSED = "Twitch didn't accept the Client ID and Client Secret: check them in this source's settings";
  var KEPT = "appToken";
  var token = null;
  var setting = (key) => {
    const value = raygun.settings.get(key);
    return typeof value === "string" ? value.trim() : "";
  };
  var credentials = () => {
    const clientId = setting("clientId");
    const clientSecret = setting("clientSecret");
    if (!clientId || !clientSecret) needsSetup(SETUP);
    return { clientId, clientSecret };
  };
  var usable = (kept, clientId) => kept !== null && kept.clientId === clientId && kept.expiresAt - Date.now() > 6e4;
  var appToken = async (fresh) => {
    const { clientId, clientSecret } = credentials();
    if (!fresh) {
      if (usable(token, clientId)) return token.value;
      const kept = raygun.storage.get(KEPT);
      if (usable(kept, clientId)) {
        token = kept;
        return kept.value;
      }
    }
    const response = await raygun.http.fetch(TOKEN, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: query({
        client_id: clientId,
        client_secret: clientSecret,
        grant_type: "client_credentials"
      })
    });
    if ([400, 401, 403].includes(response.status)) needsSetup(REFUSED);
    if (!response.ok) throw new Error(`Twitch's sign-in answered HTTP ${response.status}`);
    const answer = await response.json();
    if (!answer.access_token) throw new Error("Twitch's sign-in gave no token");
    token = {
      clientId,
      value: answer.access_token,
      expiresAt: Date.now() + (answer.expires_in ?? 3600) * 1e3
    };
    try {
      raygun.storage.set(KEPT, token);
    } catch {
    }
    return token.value;
  };
  var helix = async (path, params) => {
    const { clientId } = credentials();
    const url = `${API}/${path}?${query(params)}`;
    const send = async (fresh) => raygun.http.fetch(url, {
      headers: { "Client-Id": clientId, Authorization: `Bearer ${await appToken(fresh)}` }
    });
    let response = await send(false);
    if (response.status === 401) response = await send(true);
    if (response.status === 401 || response.status === 403) needsSetup(REFUSED);
    if (response.status === 429)
      throw new Error("Twitch is limiting requests: try again in a minute");
    if (!response.ok) throw new Error(`Twitch answered HTTP ${response.status} for ${path}`);
    return response.json();
  };

  // src/parse.ts
  var WEBSITE = "https://www.twitch.tv";
  var timeOf = (iso) => {
    if (!iso) return null;
    const ms = Date.parse(iso);
    return Number.isFinite(ms) && ms > 0 ? ms : null;
  };
  var sized = (template, width, height) => {
    if (!template || !/^https:\/\//.test(template)) return null;
    return template.replace(/%?\{width\}/g, String(width)).replace(/%?\{height\}/g, String(height));
  };
  var preview = (template, now) => {
    const url = sized(template, 640, 360);
    return url && `${url}?t=${Math.floor(now / 6e5)}`;
  };
  var isLogin = (text) => /^[a-z0-9_]{1,25}$/i.test(text);
  var line = (...parts) => parts.map((part) => (part ?? "").trim()).filter(Boolean).join(" · ") || null;
  var streamItem = (stream, now) => ({
    id: stream.user_login.toLowerCase(),
    title: stream.title?.trim() || stream.user_name,
    coverUrl: preview(stream.thumbnail_url, now),
    subtitle: line(stream.user_name, stream.game_name),
    live: true,
    viewers: typeof stream.viewer_count === "number" ? stream.viewer_count : null,
    startedAt: timeOf(stream.started_at)
  });
  var channelItem = (channel) => {
    const live = channel.is_live === true;
    return {
      id: channel.broadcaster_login.toLowerCase(),
      title: channel.display_name || channel.broadcaster_login,
      coverUrl: sized(channel.thumbnail_url, 300, 300),
      subtitle: live ? line(channel.title, channel.game_name) : line(channel.game_name),
      live,
      startedAt: live ? timeOf(channel.started_at) : null
    };
  };
  var viewerCount = (count) => {
    if (count >= 1e6) return `${(count / 1e6).toFixed(1).replace(/\.0$/, "")}M`;
    if (count >= 1e3) return `${(count / 1e3).toFixed(1).replace(/\.0$/, "")}K`;
    return String(count);
  };
  var channelDetails = (user, channel, stream, now) => {
    const live = stream && line(
      `Live now: ${stream.title?.trim() || "untitled stream"}`,
      stream.game_name,
      typeof stream.viewer_count === "number" ? `${viewerCount(stream.viewer_count)} watching` : null
    );
    return {
      id: user.login,
      title: user.display_name || user.login,
      coverUrl: stream && preview(stream.thumbnail_url, now) || sized(user.offline_image_url, 1920, 1080) || sized(user.profile_image_url, 300, 300),
      description: [live, user.description?.trim()].filter(Boolean).join("\n\n") || null,
      genres: [channel?.game_name, ...channel?.tags ?? []].filter((genre) => Boolean(genre && genre.trim())).slice(0, 10),
      type: "other",
      status: stream ? "ongoing" : "unknown",
      url: `${WEBSITE}/${user.login}`
    };
  };
  var LIVE = "live:";
  var VIDEO = "video:";
  var CLIP = "clip:";
  var liveEpisode = (login, stream) => ({
    id: `${LIVE}${login}`,
    name: `Live now: ${stream.title?.trim() || "untitled stream"}`,
    dateUpload: timeOf(stream.started_at)
  });
  var VIDEO_KINDS = { highlight: "Highlight", upload: "Upload" };
  var videoEpisode = (video) => {
    const kind = VIDEO_KINDS[video.type ?? ""];
    const title = video.title?.trim() || "Past broadcast";
    return {
      id: `${VIDEO}${video.id}`,
      name: kind ? `${kind}: ${title}` : title,
      dateUpload: timeOf(video.published_at) ?? timeOf(video.created_at)
    };
  };
  var clipEpisode = (clip) => ({
    id: `${CLIP}${clip.id}`,
    name: `Clip: ${clip.title?.trim() || clip.id}`,
    dateUpload: timeOf(clip.created_at)
  });
  var playerPages = (episodeId) => {
    const page = (url, label, live = false) => ({
      url,
      label,
      format: "page",
      ...live ? { live: true } : {}
    });
    if (episodeId.startsWith(LIVE)) {
      const channel = episodeId.slice(LIVE.length);
      if (!isLogin(channel)) return [];
      return [
        page(`${WEBSITE}/${channel}`, "On Twitch, with chat", true),
        page(
          `https://player.twitch.tv/?channel=${channel}&parent=twitch.tv&player=popout`,
          "Twitch's player alone",
          true
        )
      ];
    }
    if (episodeId.startsWith(VIDEO)) {
      const id = episodeId.slice(VIDEO.length);
      if (!/^\d+$/.test(id)) return [];
      return [
        page(`${WEBSITE}/videos/${id}`, "On Twitch, with chat"),
        page(
          `https://player.twitch.tv/?video=v${id}&parent=twitch.tv&player=popout`,
          "Twitch's player alone"
        )
      ];
    }
    if (episodeId.startsWith(CLIP)) {
      const slug = episodeId.slice(CLIP.length);
      if (!/^[\w-]{1,120}$/.test(slug)) return [];
      return [page(`https://clips.twitch.tv/${slug}`, "On Twitch")];
    }
    return [];
  };
  var categoryOptions = (games2) => games2.filter((game) => /^\d+$/.test(game.id) && game.name.trim()).map((game) => ({ value: game.id, label: game.name.trim() }));
  var LANGUAGES = [
    ["en", "English"],
    ["es", "Spanish"],
    ["pt", "Portuguese"],
    ["de", "German"],
    ["fr", "French"],
    ["ru", "Russian"],
    ["ja", "Japanese"],
    ["ko", "Korean"],
    ["zh", "Chinese"],
    ["it", "Italian"],
    ["pl", "Polish"],
    ["tr", "Turkish"],
    ["ar", "Arabic"],
    ["nl", "Dutch"],
    ["sv", "Swedish"],
    ["uk", "Ukrainian"],
    ["cs", "Czech"],
    ["hu", "Hungarian"],
    ["th", "Thai"],
    ["id", "Indonesian"],
    ["vi", "Vietnamese"],
    ["fi", "Finnish"],
    ["da", "Danish"],
    ["no", "Norwegian"],
    ["el", "Greek"],
    ["ro", "Romanian"]
  ];

  // src/index.ts
  var PAGE_SIZE = 30;
  var ALL = "all";
  var LIVE_ONLY = {
    kind: "toggle",
    key: "liveOnly",
    label: "Live channels only",
    default: false
  };
  var LANGUAGE = {
    kind: "select",
    key: "language",
    label: "Language",
    options: [
      { value: ALL, label: "All languages" },
      ...LANGUAGES.map(([value, label]) => ({ value, label }))
    ],
    default: ALL
  };
  var cursors = /* @__PURE__ */ new Map();
  var cursorFor = (list, page) => {
    if (page <= 1) return void 0;
    return cursors.get(`${list}|${page}`) ?? null;
  };
  var remember = (list, page, cursor) => {
    if (!cursor) return false;
    cursors.set(`${list}|${page + 1}`, cursor);
    if (cursors.size > 200) cursors.delete(cursors.keys().next().value);
    return true;
  };
  var EMPTY = { items: [], hasNextPage: false };
  var streams = async (filters, page) => {
    const list = `streams|${filters.game ?? ""}|${filters.language ?? ""}`;
    const after = cursorFor(list, page);
    if (after === null) return EMPTY;
    const answer = await helix("streams", {
      first: PAGE_SIZE,
      after,
      game_id: filters.game,
      language: filters.language
    });
    const now = Date.now();
    const items = (answer.data ?? []).filter((stream) => isLogin(stream.user_login)).map((stream) => streamItem(stream, now));
    return {
      items,
      hasNextPage: remember(list, page, answer.pagination?.cursor) && items.length > 0
    };
  };
  var channels = async (words, filters, page) => {
    const list = `channels|${words}|${filters.liveOnly}`;
    const after = cursorFor(list, page);
    if (after === null) return EMPTY;
    const answer = await helix("search/channels", {
      query: words,
      first: PAGE_SIZE,
      after,
      live_only: filters.liveOnly
    });
    const found = (answer.data ?? []).filter(
      (channel) => isLogin(channel.broadcaster_login) && (!filters.game || channel.game_id === filters.game) && (!filters.language || channel.broadcaster_language === filters.language)
    );
    const items = [
      ...found.filter((channel) => channel.is_live),
      ...found.filter((channel) => !channel.is_live)
    ].map(channelItem);
    return {
      items,
      hasNextPage: remember(list, page, answer.pagination?.cursor) && (answer.data ?? []).length > 0
    };
  };
  var topGames = null;
  var games = () => {
    if (!topGames) {
      topGames = helix("games/top", { first: 100 }).then((answer) => answer.data ?? []);
      topGames.catch(() => {
        topGames = null;
      });
    }
    return topGames;
  };
  var chosen = (filters) => {
    const game = filters?.category;
    const language = filters?.language;
    return {
      game: typeof game === "string" && /^\d+$/.test(game) ? game : void 0,
      language: typeof language === "string" && LANGUAGES.some(([code]) => code === language) ? language : void 0,
      liveOnly: toggleValue(filters, LIVE_ONLY)
    };
  };
  var users = /* @__PURE__ */ new Map();
  var userOf = (login) => {
    const name = login.toLowerCase();
    if (!isLogin(name)) return Promise.reject(new Error("Not a Twitch channel"));
    let found = users.get(name);
    if (!found) {
      found = helix("users", { login: name }).then((answer) => {
        const user = answer.data?.[0];
        if (!user) throw new Error(`Twitch has no channel called ${name}`);
        return user;
      });
      found.catch(() => users.delete(name));
      users.set(name, found);
      if (users.size > 100) users.delete(users.keys().next().value);
    }
    return found;
  };
  var streamOf = async (user) => (await helix("streams", { user_id: user.id })).data?.[0] ?? null;
  registerProvider(
    defineProvider({
      kind: "video.source",
      id: "twitch",
      name: "Twitch",
      searchPill: "Twitch",
      popular: ({ page }) => streams({}, page),
      filters: async () => {
        const options = categoryOptions(await games().catch(() => []));
        const category = {
          kind: "select",
          key: "category",
          label: "Category",
          options: [{ value: ALL, label: "All categories" }, ...options],
          default: ALL
        };
        return [category, LANGUAGE, LIVE_ONLY];
      },
      search: ({ query: typed, page, filters }) => {
        const words = typed.trim();
        const picked = chosen(filters);
        return words ? channels(words, picked, page) : streams(picked, page);
      },
      details: async ({ id }) => {
        const user = await userOf(id);
        const [channel, stream] = await Promise.all([
          helix("channels", { broadcaster_id: user.id }).then(
            (answer) => answer.data?.[0] ?? null
          ),
          streamOf(user)
        ]);
        return channelDetails(user, channel, stream, Date.now());
      },
      episodes: async ({ id }) => {
        const user = await userOf(id);
        const [stream, videos, clips] = await Promise.all([
          streamOf(user),
          helix("videos", { user_id: user.id, first: 50, type: "all" }).then(
            (answer) => answer.data ?? []
          ),
          helix("clips", { broadcaster_id: user.id, first: 20 }).then((answer) => answer.data ?? []).catch(() => [])
        ]);
        const episodes = [
          ...stream ? [liveEpisode(user.login, stream)] : [],
          ...videos.filter((video) => /^\d+$/.test(video.id)).map(videoEpisode),
          ...clips.filter((clip) => /^[\w-]{1,120}$/.test(clip.id)).map(clipEpisode)
        ];
        if (episodes.length === 0) {
          throw new Error(`${user.display_name} isn't live and has no videos or clips`);
        }
        return { episodes };
      },
      // Twitch's player needs nothing resolved: each episode's one server comes with its pages.
      servers: async ({ episodeId }) => {
        const videos = playerPages(episodeId);
        if (videos.length === 0) throw new Error("Not a Twitch stream, video or clip");
        return { servers: [{ id: episodeId, name: "Twitch", videos }] };
      },
      videos: async ({ serverId }) => ({ videos: playerPages(serverId) })
    })
  );
})();
