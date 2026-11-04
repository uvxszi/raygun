// KuGou lyrics (M46.39): synced lyrics from KuGou's lyric service, found the way Kreate and
// RiMusic find them. Adapted from RiMusic (github.com/fast4x/RiMusic,
// extensions/kugou/src/main/kotlin/it/fast4x/kugou/KuGou.kt), GPL-3.0: the song search by
// "artist - title", candidates by the song's hash when the length agrees within five seconds,
// else by keyword, and the lyrics with their credit lines dropped.
//
// M46.48: lyrics come as KRC, KuGou's own format, which carries the translation (Simplified
// Chinese) and romanization its editors wrote; LRC is the fallback.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { LyricsQuery, LyricsResult, LyricsTranslationResult } from '@raygun/sdk';

import { inflate } from './inflate';

const LYRICS = 'https://krcs.kugou.com';
const SONGS = 'https://mobileservice.kugou.com/api/v3/search/song';
/** Seconds a song's length may differ from KuGou's and still be the same recording. */
const TOLERANCE_S = 5;

type SongSearch = { data?: { info?: { hash: string; duration: number }[] } };
type Candidate = { id: number | string; accesskey: string };
type LyricSearch = { candidates?: Candidate[] };
type Download = { content?: string };

const BASE64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

/** Base64 to bytes (the runtime has no `atob`). */
const fromBase64 = (text: string): number[] => {
  const bytes: number[] = [];
  let buffer = 0;
  let bits = 0;
  for (const char of text) {
    const value = BASE64.indexOf(char);
    if (value < 0) continue;
    buffer = ((buffer << 6) | value) & 0xffffff;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      bytes.push((buffer >> bits) & 0xff);
    }
  }
  return bytes;
};

/** UTF-8 bytes to text (KuGou's lyrics are often Chinese, Japanese or Korean). */
const utf8 = (bytes: ArrayLike<number>): string => {
  let out = '';
  for (let index = 0; index < bytes.length;) {
    const first = bytes[index++] ?? 0;
    let code = first;
    let extra = 0;
    if (first >= 0xf0) [code, extra] = [first & 0x07, 3];
    else if (first >= 0xe0) [code, extra] = [first & 0x0f, 2];
    else if (first >= 0xc0) [code, extra] = [first & 0x1f, 1];
    for (let step = 0; step < extra; step += 1) code = (code << 6) | ((bytes[index++] ?? 0) & 0x3f);
    out += String.fromCodePoint(code);
  }
  // A byte order mark first is not text.
  return out.charCodeAt(0) === 0xfeff ? out.slice(1) : out;
};

const get = async <T>(url: string): Promise<T> => {
  const response = await raygun.http.fetch(url);
  if (!response.ok) throw new Error(`KuGou answered HTTP ${response.status}`);
  return response.json<T>();
};

/** KuGou's search words: "artist、featured - title", as its catalogue writes them. */
const keyword = (artist: string, title: string) => {
  const feat = /\s*\((?:feat\.|ft\.)\s*([^)]*)\)/i.exec(title);
  const plainTitle = feat ? title.replace(feat[0], '') : title;
  const artists = (feat ? `${artist}, ${feat[1]}` : artist)
    .replaceAll(', ', '、')
    .replaceAll(' & ', '、')
    .replaceAll('.', '');
  return `${artists} - ${plainTitle}`.trim();
};

/** A lyrics line with what KuGou's editors added to it. */
type Line = { timeMs: number; text: string; translation?: string; romanization?: string };

/** Credit lines KuGou puts first ("作词 : …", "作詞：…", "Written by：…"), not lyrics. */
const CREDIT =
  /^(作词|作詞|作曲|编曲|編曲|制作人|製作人|监制|監製|词|詞|曲|작사|작곡|편곡|written by|lyrics by|composed by|produced by|producer|arranged by)\s*[:：]/i;

const simple = (text: string) => text.toLowerCase().replace(/[\s\-–、,·()（）[\]【】]+/g, '');

/** A line that only names the song: "Artist - Title", or KRC's "Title - Artist (艾德·希兰)". */
const namesSong = (text: string, query: LyricsQuery) => {
  const line = simple(text);
  const title = simple(query.title);
  const artist = simple((query.artist ?? '').split(/,|&| feat/i)[0] ?? '');
  return Boolean(title) && line.includes(title) && (!artist || line.includes(artist));
};

/** The lines without credits or a first line that only names the song. */
const tidy = (lines: Line[], query: LyricsQuery): Line[] => {
  const kept = lines.filter((line) => !CREDIT.test(line.text));
  kept.sort((a, b) => a.timeMs - b.timeMs);
  const first = kept[0];
  if (first && namesSong(first.text, query)) kept.shift();
  return kept;
};

/** The script a romanization spells out, from the original's letters. */
const romanizedLanguage = (lines: Line[]): string => {
  const text = lines.map((line) => line.text).join('');
  if (/[぀-ヿ]/.test(text)) return 'ja-Latn';
  if (/[가-힯ᄀ-ᇿ]/.test(text)) return 'ko-Latn';
  if (/[一-鿿]/.test(text)) return 'zh-Latn';
  return 'und-Latn';
};

/** The lines as Raygun's lyrics, with KuGou's translation and romanization when it has them. */
const toResult = (lines: Line[]): LyricsResult | null => {
  if (!lines.some((line) => line.text)) return null;
  const translations: LyricsTranslationResult[] = [];
  // KuGou's translations are its editors' Simplified Chinese.
  const translated = lines.map((line) => line.translation?.trim() ?? '');
  const original = lines.map((line) => line.text);
  if (translated.some((line, index) => line && line !== original[index])) {
    translations.push({ language: 'zh-Hans', lines: translated });
  }
  const romanized = lines.map((line) => line.romanization?.replace(/\s+/g, ' ').trim() ?? '');
  // A romanization of Latin-letter lyrics only repeats them.
  if (romanized.some((line, index) => line && simple(line) !== simple(original[index] ?? ''))) {
    translations.push({
      language: romanizedLanguage(lines),
      kind: 'romanization',
      lines: romanized,
    });
  }
  return {
    synced: lines.map(({ timeMs, text }) => ({ timeMs, text })),
    plain: null,
    ...(translations.length > 0 ? { translations } : {}),
  };
};

const TIMESTAMP = /\[(\d+):(\d{1,2})(?:[.:](\d{1,3}))?\]/g;

/** LRC as timed lines. */
const parseLrc = (lrc: string): Line[] => {
  const lines: Line[] = [];
  for (const row of lrc.split(/\r?\n/)) {
    const stamps = [...row.matchAll(TIMESTAMP)];
    if (stamps.length === 0) continue;
    const text = row.replace(TIMESTAMP, '').trim();
    for (const [, minutes, seconds, fraction = '0'] of stamps) {
      const fractionMs = Number(fraction.padEnd(3, '0').slice(0, 3));
      lines.push({ timeMs: (Number(minutes) * 60 + Number(seconds)) * 1000 + fractionMs, text });
    }
  }
  return lines;
};

/** KRC's key: each byte of the file after its `krc1` mark is XORed with it. */
const KRC_KEY = [64, 71, 97, 119, 94, 50, 116, 71, 81, 54, 49, 45, 206, 210, 110, 105];

/** A downloaded KRC file's text. */
const decodeKrc = (bytes: number[]): string => {
  if (String.fromCharCode(...bytes.slice(0, 4)) !== 'krc1') throw new Error('not a KRC file');
  const body = new Uint8Array(bytes.length - 4);
  for (let index = 4; index < bytes.length; index += 1) {
    body[index - 4] = (bytes[index] ?? 0) ^ (KRC_KEY[(index - 4) % KRC_KEY.length] ?? 0);
  }
  return utf8(inflate(body));
};

type KrcLanguage = {
  content?: { type?: number; lyricContent?: (string | null)[][] }[];
};

/** KRC's `[language:…]` entry: base64 JSON with a list per kind, one entry per timed line. */
const parseLanguage = (encoded: string): { translation?: string[]; romanization?: string[] } => {
  try {
    const parsed = JSON.parse(utf8(fromBase64(encoded))) as KrcLanguage;
    const lists: { translation?: string[]; romanization?: string[] } = {};
    for (const entry of parsed.content ?? []) {
      const lines = (entry.lyricContent ?? []).map((parts) =>
        Array.isArray(parts) ? parts.map((part) => part ?? '').join('') : '',
      );
      // Type 1 is the translation, type 0 the romanization (one part per sung word).
      if (entry.type === 1) lists.translation = lines;
      else if (entry.type === 0) lists.romanization = lines;
    }
    return lists;
  } catch {
    return {};
  }
};

/** `[start,length]<offset,length,0>word…` lines, with the translations that go with them. */
const parseKrc = (krc: string): Line[] => {
  const lines: Line[] = [];
  let language = '';
  for (const row of krc.split(/\r?\n/)) {
    const tag = /^\[language:([^\]]*)\]/.exec(row);
    if (tag) {
      language = tag[1] ?? '';
      continue;
    }
    const timed = /^\[(\d+),(\d+)\](.*)$/.exec(row.trim());
    if (!timed) continue;
    const text = (timed[3] ?? '').replace(/<\d+,\d+,\d+>/g, '').trim();
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

const fetchLines = async (candidate: Candidate, format: 'krc' | 'lrc'): Promise<Line[]> => {
  const params = `ver=1&man=yes&client=pc&fmt=${format}&id=${candidate.id}&accesskey=${candidate.accesskey}`;
  const body = await get<Download>(`${LYRICS}/download?${params}`);
  if (!body.content) return [];
  const bytes = fromBase64(body.content);
  return format === 'krc' ? parseKrc(decodeKrc(bytes)) : parseLrc(utf8(bytes));
};

/** A candidate's lyrics: KRC with its translations, else LRC. */
const download = async (candidate: Candidate, query: LyricsQuery): Promise<LyricsResult | null> => {
  try {
    const krc = toResult(tidy(await fetchLines(candidate, 'krc'), query));
    if (krc) return krc;
  } catch (error) {
    raygun.log.info('KuGou KRC unreadable, using LRC', String(error));
  }
  return toResult(tidy(await fetchLines(candidate, 'lrc'), query));
};

const lyrics = defineProvider({
  kind: 'lyrics',
  id: 'lyrics',
  name: 'KuGou',
  async lyrics(query: LyricsQuery) {
    if (!query.artist) return null;
    const words = keyword(query.artist, query.title);
    const encoded = encodeURIComponent(words);
    // The recording with the same length first, by its hash; otherwise by the words.
    if (query.durationMs) {
      const seconds = Math.round(query.durationMs / 1000);
      const songs = await get<SongSearch>(
        `${SONGS}?version=9108&plat=0&pagesize=8&showtype=0&keyword=${encoded}`,
      );
      const close = (songs.data?.info ?? [])
        .map((song) => ({ song, gap: Math.abs(song.duration - seconds) }))
        .filter(({ gap }) => gap <= TOLERANCE_S)
        .sort((a, b) => a.gap - b.gap);
      for (const { song } of close) {
        const found = await get<LyricSearch>(
          `${LYRICS}/search?ver=1&man=yes&client=mobi&hash=${song.hash}`,
        );
        const candidate = found.candidates?.[0];
        if (candidate) {
          const result = await download(candidate, query);
          if (result) return result;
        }
      }
    }
    const found = await get<LyricSearch>(
      `${LYRICS}/search?ver=1&man=yes&client=mobi&keyword=${encoded}`,
    );
    const candidate = found.candidates?.[0];
    return candidate ? download(candidate, query) : null;
  },
});

registerProvider(lyrics);
