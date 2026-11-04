// dev.raygun.kugou 1.1.0. Built by raygun-ext from src/index.ts; do not edit.
"use strict";
(() => {
  // ../../packages/sdk/src/index.ts
  var defineProvider = (provider) => provider;
  var registerProvider = (provider) => {
    raygun.providers.register(provider);
  };

  // src/inflate.ts
  var Bits = class {
    constructor(data) {
      this.data = data;
    }
    data;
    at = 0;
    bit = 0;
    read(count) {
      let value = 0;
      for (let index = 0; index < count; index += 1) {
        if (this.at >= this.data.length) throw new Error("compressed data ends early");
        value |= ((this.data[this.at] ?? 0) >> this.bit & 1) << index;
        this.bit += 1;
        if (this.bit === 8) {
          this.bit = 0;
          this.at += 1;
        }
      }
      return value;
    }
    /** Skips to the next whole byte and reads `count` bytes from there. */
    bytes(count) {
      if (this.bit > 0) {
        this.bit = 0;
        this.at += 1;
      }
      if (this.at + count > this.data.length) throw new Error("compressed data ends early");
      const slice = this.data.subarray(this.at, this.at + count);
      this.at += count;
      return slice;
    }
  };
  var MAX_BITS = 15;
  var huffman = (lengths) => {
    const counts = new Array(MAX_BITS + 1).fill(0);
    for (const length of lengths) counts[length] = (counts[length] ?? 0) + 1;
    counts[0] = 0;
    const offsets = new Array(MAX_BITS + 2).fill(0);
    for (let length = 1; length <= MAX_BITS; length += 1) {
      offsets[length + 1] = (offsets[length] ?? 0) + (counts[length] ?? 0);
    }
    const symbols = new Array(lengths.length).fill(0);
    lengths.forEach((length, symbol) => {
      if (length > 0) {
        symbols[offsets[length] ?? 0] = symbol;
        offsets[length] = (offsets[length] ?? 0) + 1;
      }
    });
    return { counts, symbols };
  };
  var decode = (bits, table) => {
    let code = 0;
    let first = 0;
    let index = 0;
    for (let length = 1; length <= MAX_BITS; length += 1) {
      code |= bits.read(1);
      const count = table.counts[length] ?? 0;
      if (code - first < count) return table.symbols[index + code - first] ?? 0;
      index += count;
      first = first + count << 1;
      code <<= 1;
    }
    throw new Error("compressed data is damaged");
  };
  var LENGTH_BASE = [
    3,
    4,
    5,
    6,
    7,
    8,
    9,
    10,
    11,
    13,
    15,
    17,
    19,
    23,
    27,
    31,
    35,
    43,
    51,
    59,
    67,
    83,
    99,
    115,
    131,
    163,
    195,
    227,
    258
  ];
  var LENGTH_EXTRA = [
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    1,
    1,
    1,
    1,
    2,
    2,
    2,
    2,
    3,
    3,
    3,
    3,
    4,
    4,
    4,
    4,
    5,
    5,
    5,
    5,
    0
  ];
  var DISTANCE_BASE = [
    1,
    2,
    3,
    4,
    5,
    7,
    9,
    13,
    17,
    25,
    33,
    49,
    65,
    97,
    129,
    193,
    257,
    385,
    513,
    769,
    1025,
    1537,
    2049,
    3073,
    4097,
    6145,
    8193,
    12289,
    16385,
    24577
  ];
  var DISTANCE_EXTRA = [
    0,
    0,
    0,
    0,
    1,
    1,
    2,
    2,
    3,
    3,
    4,
    4,
    5,
    5,
    6,
    6,
    7,
    7,
    8,
    8,
    9,
    9,
    10,
    10,
    11,
    11,
    12,
    12,
    13,
    13
  ];
  var CODE_LENGTH_ORDER = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
  var fixedTables = () => {
    const lengths = [];
    for (let symbol = 0; symbol < 288; symbol += 1) {
      lengths.push(symbol < 144 ? 8 : symbol < 256 ? 9 : symbol < 280 ? 7 : 8);
    }
    return [huffman(lengths), huffman(new Array(30).fill(5))];
  };
  var dynamicTables = (bits) => {
    const literals = bits.read(5) + 257;
    const distances = bits.read(5) + 1;
    const codes = bits.read(4) + 4;
    const codeLengths = new Array(19).fill(0);
    for (let index = 0; index < codes; index += 1) {
      codeLengths[CODE_LENGTH_ORDER[index] ?? 0] = bits.read(3);
    }
    const lengthCode = huffman(codeLengths);
    const lengths = [];
    while (lengths.length < literals + distances) {
      const symbol = decode(bits, lengthCode);
      if (symbol < 16) {
        lengths.push(symbol);
        continue;
      }
      let repeat;
      let value = 0;
      if (symbol === 16) {
        if (lengths.length === 0) throw new Error("compressed data is damaged");
        value = lengths[lengths.length - 1] ?? 0;
        repeat = 3 + bits.read(2);
      } else if (symbol === 17) {
        repeat = 3 + bits.read(3);
      } else {
        repeat = 11 + bits.read(7);
      }
      for (let step = 0; step < repeat; step += 1) lengths.push(value);
    }
    if (lengths.length > literals + distances) throw new Error("compressed data is damaged");
    return [huffman(lengths.slice(0, literals)), huffman(lengths.slice(literals))];
  };
  var inflate = (data) => {
    const method = data[0] ?? 0;
    const flags = data[1] ?? 0;
    if ((method & 15) !== 8 || (method * 256 + flags) % 31 !== 0 || flags & 32) {
      throw new Error("not zlib data");
    }
    const bits = new Bits(data.subarray(2));
    let out = new Uint8Array(Math.max(1024, data.length * 4));
    let size = 0;
    const push = (byte) => {
      if (size === out.length) {
        const grown = new Uint8Array(out.length * 2);
        grown.set(out);
        out = grown;
      }
      out[size] = byte;
      size += 1;
    };
    let fixed = null;
    let last = 0;
    while (!last) {
      last = bits.read(1);
      const type = bits.read(2);
      if (type === 0) {
        const header = bits.bytes(4);
        const length = (header[0] ?? 0) | (header[1] ?? 0) << 8;
        for (const byte of bits.bytes(length)) push(byte);
        continue;
      }
      if (type === 3) throw new Error("compressed data is damaged");
      const [literal, distance] = type === 1 ? fixed ??= fixedTables() : dynamicTables(bits);
      for (; ; ) {
        const symbol = decode(bits, literal);
        if (symbol < 256) {
          push(symbol);
          continue;
        }
        if (symbol === 256) break;
        const lengthIndex = symbol - 257;
        if (lengthIndex >= LENGTH_BASE.length) throw new Error("compressed data is damaged");
        const length = (LENGTH_BASE[lengthIndex] ?? 0) + bits.read(LENGTH_EXTRA[lengthIndex] ?? 0);
        const distanceIndex = decode(bits, distance);
        if (distanceIndex >= DISTANCE_BASE.length) throw new Error("compressed data is damaged");
        const back = (DISTANCE_BASE[distanceIndex] ?? 0) + bits.read(DISTANCE_EXTRA[distanceIndex] ?? 0);
        if (back > size) throw new Error("compressed data is damaged");
        for (let step = 0; step < length; step += 1) push(out[size - back] ?? 0);
      }
    }
    return out.subarray(0, size);
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
      buffer = (buffer << 6 | value) & 16777215;
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
    return out.charCodeAt(0) === 65279 ? out.slice(1) : out;
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
  var CREDIT = /^(作词|作詞|作曲|编曲|編曲|制作人|製作人|监制|監製|词|詞|曲|작사|작곡|편곡|written by|lyrics by|composed by|produced by|producer|arranged by)\s*[:：]/i;
  var simple = (text) => text.toLowerCase().replace(/[\s\-–、,·()（）[\]【】]+/g, "");
  var namesSong = (text, query) => {
    const line = simple(text);
    const title = simple(query.title);
    const artist = simple((query.artist ?? "").split(/,|&| feat/i)[0] ?? "");
    return Boolean(title) && line.includes(title) && (!artist || line.includes(artist));
  };
  var tidy = (lines, query) => {
    const kept = lines.filter((line) => !CREDIT.test(line.text));
    kept.sort((a, b) => a.timeMs - b.timeMs);
    const first = kept[0];
    if (first && namesSong(first.text, query)) kept.shift();
    return kept;
  };
  var romanizedLanguage = (lines) => {
    const text = lines.map((line) => line.text).join("");
    if (/[぀-ヿ]/.test(text)) return "ja-Latn";
    if (/[가-힯ᄀ-ᇿ]/.test(text)) return "ko-Latn";
    if (/[一-鿿]/.test(text)) return "zh-Latn";
    return "und-Latn";
  };
  var toResult = (lines) => {
    if (!lines.some((line) => line.text)) return null;
    const translations = [];
    const translated = lines.map((line) => line.translation?.trim() ?? "");
    const original = lines.map((line) => line.text);
    if (translated.some((line, index) => line && line !== original[index])) {
      translations.push({ language: "zh-Hans", lines: translated });
    }
    const romanized = lines.map((line) => line.romanization?.replace(/\s+/g, " ").trim() ?? "");
    if (romanized.some((line, index) => line && simple(line) !== simple(original[index] ?? ""))) {
      translations.push({
        language: romanizedLanguage(lines),
        kind: "romanization",
        lines: romanized
      });
    }
    return {
      synced: lines.map(({ timeMs, text }) => ({ timeMs, text })),
      plain: null,
      ...translations.length > 0 ? { translations } : {}
    };
  };
  var TIMESTAMP = /\[(\d+):(\d{1,2})(?:[.:](\d{1,3}))?\]/g;
  var parseLrc = (lrc) => {
    const lines = [];
    for (const row of lrc.split(/\r?\n/)) {
      const stamps = [...row.matchAll(TIMESTAMP)];
      if (stamps.length === 0) continue;
      const text = row.replace(TIMESTAMP, "").trim();
      for (const [, minutes, seconds, fraction = "0"] of stamps) {
        const fractionMs = Number(fraction.padEnd(3, "0").slice(0, 3));
        lines.push({ timeMs: (Number(minutes) * 60 + Number(seconds)) * 1e3 + fractionMs, text });
      }
    }
    return lines;
  };
  var KRC_KEY = [64, 71, 97, 119, 94, 50, 116, 71, 81, 54, 49, 45, 206, 210, 110, 105];
  var decodeKrc = (bytes) => {
    if (String.fromCharCode(...bytes.slice(0, 4)) !== "krc1") throw new Error("not a KRC file");
    const body = new Uint8Array(bytes.length - 4);
    for (let index = 4; index < bytes.length; index += 1) {
      body[index - 4] = (bytes[index] ?? 0) ^ (KRC_KEY[(index - 4) % KRC_KEY.length] ?? 0);
    }
    return utf8(inflate(body));
  };
  var parseLanguage = (encoded) => {
    try {
      const parsed = JSON.parse(utf8(fromBase64(encoded)));
      const lists = {};
      for (const entry of parsed.content ?? []) {
        const lines = (entry.lyricContent ?? []).map(
          (parts) => Array.isArray(parts) ? parts.map((part) => part ?? "").join("") : ""
        );
        if (entry.type === 1) lists.translation = lines;
        else if (entry.type === 0) lists.romanization = lines;
      }
      return lists;
    } catch {
      return {};
    }
  };
  var parseKrc = (krc) => {
    const lines = [];
    let language = "";
    for (const row of krc.split(/\r?\n/)) {
      const tag = /^\[language:([^\]]*)\]/.exec(row);
      if (tag) {
        language = tag[1] ?? "";
        continue;
      }
      const timed = /^\[(\d+),(\d+)\](.*)$/.exec(row.trim());
      if (!timed) continue;
      const text = (timed[3] ?? "").replace(/<\d+,\d+,\d+>/g, "").trim();
      lines.push({ timeMs: Number(timed[1]), text });
    }
    if (language) {
      const { translation, romanization } = parseLanguage(language);
      lines.forEach((line, index) => {
        if (translation?.[index]) line.translation = translation[index];
        if (romanization?.[index]) line.romanization = romanization[index];
      });
    }
    return lines;
  };
  var fetchLines = async (candidate, format) => {
    const params = `ver=1&man=yes&client=pc&fmt=${format}&id=${candidate.id}&accesskey=${candidate.accesskey}`;
    const body = await get(`${LYRICS}/download?${params}`);
    if (!body.content) return [];
    const bytes = fromBase64(body.content);
    return format === "krc" ? parseKrc(decodeKrc(bytes)) : parseLrc(utf8(bytes));
  };
  var download = async (candidate, query) => {
    try {
      const krc = toResult(tidy(await fetchLines(candidate, "krc"), query));
      if (krc) return krc;
    } catch (error) {
      raygun.log.info("KuGou KRC unreadable, using LRC", String(error));
    }
    return toResult(tidy(await fetchLines(candidate, "lrc"), query));
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
            const result = await download(candidate2, query);
            if (result) return result;
          }
        }
      }
      const found = await get(
        `${LYRICS}/search?ver=1&man=yes&client=mobi&keyword=${encoded}`
      );
      const candidate = found.candidates?.[0];
      return candidate ? download(candidate, query) : null;
    }
  });
  registerProvider(lyrics);
})();
