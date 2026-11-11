// dev.raygun.podcast-index 1.1.0. Built by raygun-ext from src/index.ts; do not edit.
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

  // src/sha1.ts
  var rotate = (value, by) => value << by | value >>> 32 - by;
  var utf8 = (text) => {
    const bytes = [];
    for (const char of text) {
      const code = char.codePointAt(0) ?? 0;
      if (code < 128) {
        bytes.push(code);
      } else if (code < 2048) {
        bytes.push(192 | code >> 6, 128 | code & 63);
      } else if (code < 65536) {
        bytes.push(224 | code >> 12, 128 | code >> 6 & 63, 128 | code & 63);
      } else {
        bytes.push(
          240 | code >> 18,
          128 | code >> 12 & 63,
          128 | code >> 6 & 63,
          128 | code & 63
        );
      }
    }
    return bytes;
  };
  var sha1Hex = (text) => {
    const bytes = utf8(text);
    const blocks = (bytes.length + 8 >> 6) + 1;
    const words = new Array(blocks * 16).fill(0);
    bytes.forEach((byte, index) => {
      words[index >> 2] = (words[index >> 2] ?? 0) | byte << 24 - index % 4 * 8;
    });
    words[bytes.length >> 2] = (words[bytes.length >> 2] ?? 0) | 128 << 24 - bytes.length % 4 * 8;
    const bits = bytes.length * 8;
    words[blocks * 16 - 2] = Math.floor(bits / 4294967296);
    words[blocks * 16 - 1] = bits >>> 0;
    const hash = [1732584193, 4023233417, 2562383102, 271733878, 3285377520];
    const schedule = new Array(80).fill(0);
    for (let block = 0; block < words.length; block += 16) {
      for (let t = 0; t < 80; t += 1) {
        schedule[t] = t < 16 ? words[block + t] ?? 0 : rotate(
          (schedule[t - 3] ?? 0) ^ (schedule[t - 8] ?? 0) ^ (schedule[t - 14] ?? 0) ^ (schedule[t - 16] ?? 0),
          1
        );
      }
      let [a, b, c, d, e] = hash;
      for (let t = 0; t < 80; t += 1) {
        const mix = t < 20 ? b & c | ~b & d : t < 40 ? b ^ c ^ d : t < 60 ? b & c | b & d | c & d : b ^ c ^ d;
        const constant = t < 20 ? 1518500249 : t < 40 ? 1859775393 : t < 60 ? 2400959708 : 3395469782;
        const next = rotate(a, 5) + mix + e + constant + (schedule[t] ?? 0) | 0;
        e = d;
        d = c;
        c = rotate(b, 30);
        b = a;
        a = next;
      }
      hash[0] = (hash[0] ?? 0) + a | 0;
      hash[1] = (hash[1] ?? 0) + b | 0;
      hash[2] = (hash[2] ?? 0) + c | 0;
      hash[3] = (hash[3] ?? 0) + d | 0;
      hash[4] = (hash[4] ?? 0) + e | 0;
    }
    return hash.map((word) => (word >>> 0).toString(16).padStart(8, "0")).join("");
  };

  // src/index.ts
  var API = "https://api.podcastindex.org/api/1.0";
  var MOST = 100;
  var LANGUAGES = {
    Arabic: "ar",
    Chinese: "zh",
    Dutch: "nl",
    English: "en",
    French: "fr",
    German: "de",
    Hindi: "hi",
    Italian: "it",
    Japanese: "ja",
    Korean: "ko",
    Polish: "pl",
    Portuguese: "pt",
    Russian: "ru",
    Spanish: "es",
    Swedish: "sv",
    Turkish: "tr"
  };
  var plain = (text) => (text ?? "").replace(/<[^>]+>/g, " ").replace(/&nbsp;/g, " ").replace(/&amp;/g, "&").replace(/&quot;/g, '"').replace(/&#0?39;|&rsquo;/g, "'").replace(/\s+/g, " ").trim();
  var isWeb = (address) => typeof address === "string" && /^https?:\/\//i.test(address);
  var toResult = (feed) => {
    const title = plain(feed.title);
    if (!title || !isWeb(feed.url) || feed.dead === 1) return null;
    if (feed.explicit === true && !raygun.content.showNsfw()) return null;
    const description = plain(feed.description);
    return {
      title,
      author: plain(feed.author) || plain(feed.ownerName) || null,
      feedUrl: feed.url,
      artworkUrl: [feed.artwork, feed.image].find(isWeb) ?? null,
      description: description ? description.slice(0, 600) : null
    };
  };
  var credentials = () => {
    const key = raygun.settings.get("apiKey")?.trim();
    const secret = raygun.settings.get("apiSecret")?.trim();
    if (!key || !secret) {
      return needsSetup(
        "Podcast Index needs your free API key and secret: get them at api.podcastindex.org and add them in this source's settings"
      );
    }
    return { key, secret };
  };
  var ask = async (path, params) => {
    const { key, secret } = credentials();
    const time = String(Math.floor(Date.now() / 1e3));
    const address = `${API}/${path}?${query(params)}`;
    const response = await raygun.http.fetch(address, {
      headers: {
        Accept: "application/json",
        "X-Auth-Key": key,
        "X-Auth-Date": time,
        Authorization: sha1Hex(`${key}${secret}${time}`)
      }
    });
    if (response.status === 401 || response.status === 403) {
      needsSetup(
        "Podcast Index didn't accept the API key and secret: check them in this source's settings"
      );
    }
    if (!response.ok) throw new Error(`HTTP ${response.status} from ${address}`);
    const answer = await response.json();
    const seen = /* @__PURE__ */ new Set();
    return (answer.feeds ?? []).map(toResult).filter((podcast) => {
      if (!podcast || seen.has(podcast.feedUrl)) return false;
      seen.add(podcast.feedUrl);
      return true;
    });
  };
  var count = (limit) => Math.max(1, Math.min(limit || 20, MOST));
  var clean = () => raygun.content.showNsfw() ? {} : { clean: "" };
  registerProvider(
    defineProvider({
      kind: "podcast.directory",
      id: "podcasts",
      name: "Podcast Index",
      async search({ query: text, limit }) {
        const term = text.trim();
        if (!term) return [];
        const found = await ask("search/byterm", { q: term, max: count(limit), ...clean() });
        return found.slice(0, count(limit));
      },
      async top({ limit }) {
        const language = LANGUAGES[raygun.settings.get("language") ?? "English"];
        const params = { max: count(limit) };
        if (language) params.lang = language;
        const trending = await ask("podcasts/trending", params);
        return trending.slice(0, count(limit));
      }
    })
  );
})();
