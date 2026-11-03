// dev.raygun.kugou 1.0.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/index.ts
  var LYRICS = "https://krcs.kugou.com";
  var SONGS = "https://mobileservice.kugou.com/api/v3/search/song";
  var TOLERANCE_S = 5;
  var BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  var fromBase64 = (text) => {
    const bytes = [];
    let buffer = 0;
    let bits = 0;
    for (const char of text) {
      const value = BASE64.indexOf(char);
      if (value < 0) continue;
      buffer = buffer << 6 | value;
      bits += 6;
      if (bits >= 8) {
        bits -= 8;
        bytes.push(buffer >> bits & 255);
      }
    }
    return bytes;
  };
  var utf8 = (bytes) => {
    let out = "";
    for (let index = 0; index < bytes.length; ) {
      const first = bytes[index++] ?? 0;
      let code = first;
      let extra = 0;
      if (first >= 240) [code, extra] = [first & 7, 3];
      else if (first >= 224) [code, extra] = [first & 15, 2];
      else if (first >= 192) [code, extra] = [first & 31, 1];
      for (let step = 0; step < extra; step += 1) code = code << 6 | (bytes[index++] ?? 0) & 63;
      out += String.fromCodePoint(code);
    }
    return out;
  };
  var get = async (url) => {
    const response = await raygun.http.fetch(url);
    if (!response.ok) throw new Error(`KuGou answered HTTP ${response.status}`);
    return response.json();
  };
  var keyword = (artist, title) => {
    const feat = /\s*\((?:feat\.|ft\.)\s*([^)]*)\)/i.exec(title);
    const plainTitle = feat ? title.replace(feat[0], "") : title;
    const artists = (feat ? `${artist}, ${feat[1]}` : artist).replaceAll(", ", "、").replaceAll(" & ", "、").replaceAll(".", "");
    return `${artists} - ${plainTitle}`.trim();
  };
  var TIMESTAMP = /\[(\d+):(\d{1,2})(?:[.:](\d{1,3}))?\]/g;
  var CREDIT = /^(作词|作曲|编曲|制作人|written by|lyrics by|composed by|producer)\s*[:：]/i;
  var parseLrc = (lrc, song) => {
    const lines = [];
    for (const row of lrc.split(/\r?\n/)) {
      const stamps = [...row.matchAll(TIMESTAMP)];
      if (stamps.length === 0) continue;
      const text = row.replace(TIMESTAMP, "").trim();
      if (CREDIT.test(text)) continue;
      for (const [, minutes, seconds, fraction = "0"] of stamps) {
        const fractionMs = Number(fraction.padEnd(3, "0").slice(0, 3));
        lines.push({ timeMs: (Number(minutes) * 60 + Number(seconds)) * 1e3 + fractionMs, text });
      }
    }
    lines.sort((a, b) => a.timeMs - b.timeMs);
    const simple = (text) => text.toLowerCase().replace(/[\s\-–、,]+/g, "");
    if (lines[0] && simple(lines[0].text) === simple(song)) lines.shift();
    return lines.some((line) => line.text) ? { synced: lines, plain: null } : null;
  };
  var download = async (candidate, song) => {
    const params = `ver=1&man=yes&client=pc&fmt=lrc&id=${candidate.id}&accesskey=${candidate.accesskey}`;
    const body = await get(`${LYRICS}/download?${params}`);
    return body.content ? parseLrc(utf8(fromBase64(body.content)), song) : null;
  };
  var lyrics = defineProvider({
    kind: "lyrics",
    id: "lyrics",
    name: "KuGou",
    async lyrics(query) {
      if (!query.artist) return null;
      const words = keyword(query.artist, query.title);
      const encoded = encodeURIComponent(words);
      if (query.durationMs) {
        const seconds = Math.round(query.durationMs / 1e3);
        const songs = await get(
          `${SONGS}?version=9108&plat=0&pagesize=8&showtype=0&keyword=${encoded}`
        );
        const close = (songs.data?.info ?? []).map((song) => ({ song, gap: Math.abs(song.duration - seconds) })).filter(({ gap }) => gap <= TOLERANCE_S).sort((a, b) => a.gap - b.gap);
        for (const { song } of close) {
          const found2 = await get(
            `${LYRICS}/search?ver=1&man=yes&client=mobi&hash=${song.hash}`
          );
          const candidate2 = found2.candidates?.[0];
          if (candidate2) {
            const result = await download(candidate2, words);
            if (result) return result;
          }
        }
      }
      const found = await get(
        `${LYRICS}/search?ver=1&man=yes&client=mobi&keyword=${encoded}`
      );
      const candidate = found.candidates?.[0];
      return candidate ? download(candidate, words) : null;
    }
  });
  registerProvider(lyrics);
})();
