// dev.raygun.lastfm 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
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

  // src/api.ts
  var API = "https://ws.audioscrobbler.com/2.0";
  var HEADERS = {
    "User-Agent": "Raygun/1.0 ( https://github.com/uvxszi/raygun )",
    Accept: "application/json"
  };
  var SPACING_MS = 250;
  var KEEP_MS = 60 * 60 * 1e3;
  var KEEP_AT_MOST_MS = 24 * 60 * 60 * 1e3;
  var KEEP_CHARS = 8 * 1024 * 1024;
  var REST_MS = 60 * 1e3;
  var RATE_LIMITED = "Last.fm is limiting requests for a moment; try again in a minute";
  var apiKey = () => {
    const key = raygun.settings.get("apiKey");
    const typed = typeof key === "string" ? key.trim() : "";
    if (!typed) {
      return needsSetup(
        "Last.fm needs your own free API key: create one at last.fm/api/account/create and enter it in this source's settings"
      );
    }
    return typed;
  };
  var nextStart = 0;
  var restUntil = 0;
  var turn = async () => {
    const start = Math.max(Date.now(), nextStart);
    nextStart = start + SPACING_MS;
    for (let wait = start - Date.now(); wait > 0; wait = start - Date.now()) {
      await raygun.time.sleep(Math.min(wait, 1e4));
    }
  };
  var answers = /* @__PURE__ */ new Map();
  var keptChars = 0;
  var forget = (key) => {
    const known = answers.get(key);
    if (!known) return;
    keptChars -= known.size;
    answers.delete(key);
  };
  var keep = (key, value, size, ms) => {
    forget(key);
    if (ms <= 0 || size > KEEP_CHARS / 8) return;
    answers.set(key, { until: Date.now() + ms, size, value });
    keptChars += size;
    while (keptChars > KEEP_CHARS && answers.size > 0) forget(answers.keys().next().value ?? key);
  };
  var keepFor = (headers) => {
    const control = (headers["cache-control"] ?? "").toLowerCase();
    if (/no-store|no-cache/.test(control)) return 0;
    const maxAge = /max-age=(\d+)/.exec(control);
    return maxAge ? Math.min(Number(maxAge[1]) * 1e3, KEEP_AT_MOST_MS) : KEEP_MS;
  };
  var lastfm = async (method, params) => {
    const key = apiKey();
    const asked = query({ method, ...params, format: "json" });
    const known = answers.get(asked);
    if (known && known.until > Date.now()) return known.value;
    if (Date.now() < restUntil) throw new Error(RATE_LIMITED);
    await turn();
    if (Date.now() < restUntil) throw new Error(RATE_LIMITED);
    const response = await raygun.http.fetch(`${API}/?${asked}&${query({ api_key: key })}`, {
      headers: HEADERS
    });
    const text = await response.text();
    let answer = null;
    try {
      const parsed = JSON.parse(text);
      if (parsed && typeof parsed === "object") answer = parsed;
    } catch {
      answer = null;
    }
    const code = answer?.error === void 0 ? null : Number(answer.error);
    const said = typeof answer?.message === "string" ? answer.message.trim() : "";
    if (code === 26) {
      needsSetup(
        "Last.fm has suspended this API key: create a new one at last.fm/api/account/create and enter it in this source's settings"
      );
    }
    if (code === 10 || response.status === 401 || response.status === 403) {
      needsSetup("Last.fm didn't accept the API key: check it in this source's settings");
    }
    if (code === 29 || response.status === 429) {
      restUntil = Date.now() + REST_MS;
      throw new Error(RATE_LIMITED);
    }
    if (code === 6) {
      keep(asked, null, 16, keepFor(response.headers));
      return null;
    }
    if (code === 8 || code === 11 || code === 16) {
      throw new Error("Last.fm is having trouble answering right now; try again in a few minutes");
    }
    if (code !== null) throw new Error(`Last.fm couldn't answer${said ? `: ${said}` : ""}`);
    if (!response.ok) throw new Error(`Last.fm answered with an error (HTTP ${response.status})`);
    if (!answer) throw new Error("Last.fm's answer couldn't be read");
    keep(asked, answer, text.length, keepFor(response.headers));
    return answer;
  };
  var listOf = (value) => Array.isArray(value) ? value : value && typeof value === "object" ? [value] : [];
  var textOf = (value) => typeof value === "string" ? value.replace(/\s+/g, " ").trim() : "";
  var numberOf = (value) => {
    const number = typeof value === "number" ? value : typeof value === "string" ? Number(value.trim() || NaN) : NaN;
    return Number.isFinite(number) ? number : null;
  };
  var clamp = (value, low, high) => Math.max(low, Math.min(Number.isFinite(value) ? Math.floor(value) : low, high));
  var words = (text) => text.normalize("NFKD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  var artistName = (artist) => typeof artist === "string" ? textOf(artist) : textOf(artist?.name) || textOf(artist?.["#text"]);
  var lengthOf = (duration) => {
    const number = numberOf(duration);
    if (!number || number <= 0) return null;
    return Math.round(number >= 3e4 ? number : number * 1e3);
  };
  var toTrack = (track) => {
    const title = textOf(track.name);
    if (!title) return [];
    const artist = artistName(track.artist);
    return [
      {
        id: textOf(track.mbid) || textOf(track.url) || `${artist} – ${title}`,
        title,
        artist: artist || null,
        durationMs: lengthOf(track.duration)
      }
    ];
  };
  var distinct = (tracks) => {
    const seen = /* @__PURE__ */ new Set();
    return tracks.filter((track) => {
      const key = `${words(track.artist ?? "")}|${words(track.title)}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  };
  var tracksOf = (list) => distinct(listOf(list?.track).flatMap(toTrack));
  var songsIn = (answer) => tracksOf(answer?.tracks ?? answer?.toptracks ?? answer?.similartracks);

  // src/charts.ts
  var MOST = 100;
  var COUNTRIES = [
    ["us", "the United States", "United States"],
    ["gb", "the United Kingdom", "United Kingdom"],
    ["ca", "Canada", "Canada"],
    ["au", "Australia", "Australia"],
    ["nz", "New Zealand", "New Zealand"],
    ["ie", "Ireland", "Ireland"],
    ["de", "Germany", "Germany"],
    ["fr", "France", "France"],
    ["es", "Spain", "Spain"],
    ["it", "Italy", "Italy"],
    ["pt", "Portugal", "Portugal"],
    ["nl", "the Netherlands", "Netherlands"],
    ["be", "Belgium", "Belgium"],
    ["ch", "Switzerland", "Switzerland"],
    ["at", "Austria", "Austria"],
    ["dk", "Denmark", "Denmark"],
    ["no", "Norway", "Norway"],
    ["se", "Sweden", "Sweden"],
    ["fi", "Finland", "Finland"],
    ["is", "Iceland", "Iceland"],
    ["pl", "Poland", "Poland"],
    ["cz", "Czechia", "Czech Republic", "Czechia"],
    ["sk", "Slovakia", "Slovakia"],
    ["hu", "Hungary", "Hungary"],
    ["ro", "Romania", "Romania"],
    ["bg", "Bulgaria", "Bulgaria"],
    ["gr", "Greece", "Greece"],
    ["hr", "Croatia", "Croatia"],
    ["si", "Slovenia", "Slovenia"],
    ["rs", "Serbia", "Serbia"],
    ["ua", "Ukraine", "Ukraine"],
    ["by", "Belarus", "Belarus"],
    ["ru", "Russia", "Russian Federation", "Russia"],
    ["ee", "Estonia", "Estonia"],
    ["lv", "Latvia", "Latvia"],
    ["lt", "Lithuania", "Lithuania"],
    ["tr", "Turkey", "Turkey", "Türkiye"],
    ["il", "Israel", "Israel"],
    ["mx", "Mexico", "Mexico"],
    ["br", "Brazil", "Brazil"],
    ["ar", "Argentina", "Argentina"],
    ["cl", "Chile", "Chile"],
    ["co", "Colombia", "Colombia"],
    ["pe", "Peru", "Peru"],
    ["ve", "Venezuela", "Venezuela", "Venezuela, Bolivarian Republic of"],
    ["uy", "Uruguay", "Uruguay"],
    ["ec", "Ecuador", "Ecuador"],
    ["cr", "Costa Rica", "Costa Rica"],
    ["za", "South Africa", "South Africa"],
    ["ng", "Nigeria", "Nigeria"],
    ["eg", "Egypt", "Egypt"],
    ["ma", "Morocco", "Morocco"],
    ["ke", "Kenya", "Kenya"],
    ["in", "India", "India"],
    ["id", "Indonesia", "Indonesia"],
    ["my", "Malaysia", "Malaysia"],
    ["sg", "Singapore", "Singapore"],
    ["ph", "the Philippines", "Philippines"],
    ["th", "Thailand", "Thailand"],
    ["vn", "Vietnam", "Viet Nam", "Vietnam"],
    ["kr", "South Korea", "Korea, Republic of", "South Korea"],
    ["jp", "Japan", "Japan"],
    ["tw", "Taiwan", "Taiwan", "Taiwan, Province of China"],
    ["hk", "Hong Kong", "Hong Kong"],
    ["cn", "China", "China"],
    ["sa", "Saudi Arabia", "Saudi Arabia"],
    ["ae", "the United Arab Emirates", "United Arab Emirates"]
  ];
  var BY_CODE = new Map(COUNTRIES.map(([code, shown, ...names]) => [code, { shown, names }]));
  var countryOf = (region) => {
    const typed = (region ?? "").trim().toLowerCase();
    const code = typed === "uk" ? "gb" : typed;
    return BY_CODE.has(code) ? code : null;
  };
  var GENRES = [
    ["rock", "Rock"],
    ["pop", "Pop"],
    ["hip-hop", "Hip-Hop"],
    ["electronic", "Electronic"],
    ["indie", "Indie"],
    ["alternative", "Alternative"],
    ["metal", "Metal"],
    ["punk", "Punk"],
    ["rnb", "R&B"],
    ["soul", "Soul"],
    ["funk", "Funk"],
    ["jazz", "Jazz"],
    ["blues", "Blues"],
    ["folk", "Folk"],
    ["country", "Country"],
    ["singer-songwriter", "Singer-Songwriter"],
    ["classical", "Classical"],
    ["soundtrack", "Soundtrack"],
    ["reggae", "Reggae"],
    ["latin", "Latin"],
    ["dance", "Dance"],
    ["house", "House"],
    ["techno", "Techno"],
    ["ambient", "Ambient"],
    ["k-pop", "K-Pop"],
    ["j-pop", "J-Pop"]
  ];
  var tagTracks = async (tag, count) => songsIn(await lastfm("tag.getTopTracks", { tag, limit: count })).slice(0, count);
  var countryTracks = async (code, count) => {
    const country = BY_CODE.get(code);
    if (!country) throw new Error("Last.fm has no chart for this country");
    for (const name of country.names) {
      const answer = await lastfm("geo.getTopTracks", { country: name, limit: count });
      if (answer) return songsIn(answer).slice(0, count);
    }
    throw new Error(`Last.fm has no chart for ${country.shown}`);
  };
  var charts = defineProvider({
    kind: "music.charts",
    id: "charts",
    name: "Last.fm",
    async charts({ region }) {
      const country = countryOf(region);
      const shown = country ? BY_CODE.get(country)?.shown : null;
      const list = [
        {
          id: "top",
          name: "Top songs on Last.fm",
          description: "The songs most played on Last.fm",
          group: "top"
        },
        ...country && shown ? [
          {
            id: `country:${country}`,
            name: `Top songs in ${shown}`,
            description: `The songs most played in ${shown} on Last.fm`,
            group: "top"
          }
        ] : [],
        ...GENRES.map(([tag, name]) => ({
          id: `tag:${tag}`,
          name: `${name} on Last.fm`,
          description: `The songs Last.fm's listeners tag ${tag} most`,
          tag: name,
          group: "genre"
        }))
      ];
      return { charts: list };
    },
    async chart({ id, limit }) {
      const count = clamp(limit || 50, 1, MOST);
      if (id === "top") {
        const answer = await lastfm("chart.getTopTracks", { limit: count });
        return { tracks: songsIn(answer).slice(0, count) };
      }
      if (id.startsWith("country:")) return { tracks: await countryTracks(id.slice(8), count) };
      const tag = id.startsWith("tag:") ? id.slice(4).trim() : "";
      if (!tag || tag.length > 60) throw new Error("Last.fm has no such chart");
      return { tracks: await tagTracks(tag, count) };
    },
    // A genre's or mood's songs: its tag's top songs, else those of the first of its other
    // spellings that has some.
    async tagSongs({ tag, aliases, limit }) {
      const count = clamp(limit || 25, 1, MOST);
      const tags2 = [...new Set([tag, ...aliases].map((name) => name.trim().toLowerCase()))].filter((name) => words(name) !== "" && name.length <= 60).slice(0, 5);
      for (const name of tags2) {
        const tracks = await tagTracks(name, count);
        if (tracks.length > 0) return { tracks };
      }
      return { tracks: [] };
    }
  });

  // src/index.ts
  var SITE = "https://www.last.fm";
  var NOT_GENRES = /* @__PURE__ */ new Set([
    "seen live",
    "favorite",
    "favourite",
    "favorites",
    "favourites",
    "my favorite",
    "my favourite",
    "my favorites",
    "my favourites",
    "favorite songs",
    "favourite songs",
    "loved",
    "loved tracks",
    "albums i own",
    "awesome",
    "spotify",
    "under 2000 listeners"
  ]);
  var MIN_COUNT = 10;
  var MOST_TAGS = 10;
  var genreTags = (tags2, scale, artist) => {
    const own = words(artist);
    const seen = /* @__PURE__ */ new Set();
    return listOf(tags2).flatMap((tag) => {
      const name = textOf(tag.name);
      const key = words(name);
      const count = Math.min(numberOf(tag.count) ?? 0, 100);
      const kept = key !== "" && name.length <= 60 && count >= MIN_COUNT && !NOT_GENRES.has(key) && !/^\d{4}$/.test(key) && key !== own && !seen.has(key);
      if (!kept) return [];
      seen.add(key);
      return [{ name, weight: count / 100 * scale }];
    }).slice(0, MOST_TAGS);
  };
  var tags = defineProvider({
    kind: "music.tags",
    id: "tags",
    name: "Last.fm",
    async tags({ title, artist }) {
      const name = artist?.trim() ?? "";
      const song = title.trim();
      if (!name || !song) return { tags: [] };
      const own = await lastfm("track.getTopTags", {
        artist: name,
        track: song,
        autocorrect: 1
      });
      const found = genreTags(own?.toptags?.tag, 1, name);
      if (found.length > 0) return { tags: found };
      const byArtist = await lastfm("artist.getTopTags", { artist: name, autocorrect: 1 });
      return { tags: genreTags(byArtist?.toptags?.tag, 0.5, name) };
    }
  });
  var similarTo = async (artist, count) => {
    const answer = await lastfm("artist.getSimilar", {
      artist,
      autocorrect: 1,
      limit: count
    });
    const seen = /* @__PURE__ */ new Set([words(artist)]);
    return listOf(answer?.similarartists?.artist).flatMap((found) => {
      const name = textOf(found.name);
      if (!name || seen.has(words(name))) return [];
      seen.add(words(name));
      return [{ id: textOf(found.mbid) || textOf(found.url) || name, name }];
    }).slice(0, count);
  };
  var topTracksOf = async (artist, count) => songsIn(
    await lastfm("artist.getTopTracks", { artist, autocorrect: 1, limit: count })
  ).slice(0, count);
  var interleave = (groups) => {
    const order = [];
    const longest = Math.max(0, ...groups.map((group) => group.length));
    for (let index = 0; index < longest; index += 1) {
      for (const group of groups) {
        const track = group[index];
        if (track) order.push(track);
      }
    }
    return order;
  };
  var MIX_ARTISTS = 8;
  var recommend = defineProvider({
    kind: "music.recommend",
    id: "recommend",
    name: "Last.fm",
    async similarArtists({ artist, limit }) {
      const name = artist.trim();
      return { artists: name ? await similarTo(name, clamp(limit || 10, 1, 100)) : [] };
    },
    async topTracks({ artist, limit }) {
      const name = artist.trim();
      return { tracks: name ? await topTracksOf(name, clamp(limit || 10, 1, 100)) : [] };
    },
    // Songs like the song, when Last.fm knows it; else the most played songs of the artist's
    // closest artists, a few each, taking turns.
    async radio({ artist, title, limit }) {
      const name = artist.trim();
      if (!name) return { tracks: [] };
      const count = clamp(limit || 25, 1, 100);
      const song = title?.trim();
      if (song) {
        const similar = await lastfm("track.getSimilar", {
          artist: name,
          track: song,
          autocorrect: 1,
          limit: Math.min(count * 2, 200)
        });
        const tracks = songsIn(similar).slice(0, count);
        if (tracks.length > 0) return { tracks };
      }
      const others = await similarTo(name, MIX_ARTISTS);
      const each = Math.max(2, Math.ceil(count * 3 / 2 / Math.max(1, others.length)));
      const groups = [];
      for (const other of others) {
        try {
          groups.push(await topTracksOf(other.name, each));
        } catch (error) {
          if (String(error).includes("needs-setup")) throw error;
          raygun.log.info(`Last.fm had no songs of ${other.name}: ${String(error)}`);
        }
      }
      return { tracks: distinct(interleave(groups)).slice(0, count) };
    }
  });
  var decode = (text) => text.replace(/&nbsp;|&#160;/g, " ").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#0?39;|&apos;/g, "'").replace(
    /&#x([0-9a-f]+);/gi,
    (_match, code) => String.fromCodePoint(parseInt(code, 16))
  ).replace(/&#(\d+);/g, (_match, code) => String.fromCodePoint(Number(code))).replace(/&amp;/g, "&");
  var biography = (html) => decode(
    (typeof html === "string" ? html : "").replace(/<a\b[^>]*>\s*Read more on Last\.fm\s*<\/a>\.?/gi, "").replace(
      /User-contributed text is available under the Creative Commons By-SA License;?\s*(?:additional terms may apply\.?)?/gi,
      ""
    ).replace(/<br\s*\/?>|<\/p>/gi, "\n").replace(/<[^>]+>/g, "")
  ).split(/\n+/).map((paragraph) => paragraph.replace(/\s+/g, " ").trim()).filter(Boolean).join("\n\n");
  var cut = (text, most) => {
    if (text.length <= most) return text;
    const part = text.slice(0, most);
    const end = Math.max(part.lastIndexOf(". "), part.lastIndexOf(".\n"));
    return end > most / 2 ? part.slice(0, end + 1) : `${part.trimEnd()}…`;
  };
  var info = defineProvider({
    kind: "music.info",
    id: "info",
    name: "Last.fm",
    async artist({ name }) {
      const wanted = name.trim();
      if (!wanted) return { artist: null };
      const answer = await lastfm("artist.getInfo", {
        artist: wanted,
        autocorrect: 1
      });
      const found = answer?.artist;
      const shown = textOf(found?.name);
      const bio = cut(biography(found?.bio?.summary) || biography(found?.bio?.content), 1500);
      if (!shown || !bio) return { artist: null };
      const page = textOf(found?.url);
      return {
        artist: {
          name: shown,
          bio,
          url: /^https?:\/\//i.test(page) ? page : `${SITE}/music/${encodeURIComponent(shown)}`
        }
      };
    }
  });
  registerProvider(charts);
  registerProvider(tags);
  registerProvider(recommend);
  registerProvider(info);
})();
