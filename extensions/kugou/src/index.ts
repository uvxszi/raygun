// KuGou lyrics (M46.39): synced lyrics from KuGou's lyric service, found the way Kreate and
// RiMusic find them. Adapted from RiMusic (github.com/fast4x/RiMusic,
// extensions/kugou/src/main/kotlin/it/fast4x/kugou/KuGou.kt), GPL-3.0: the song search by
// "artist - title", candidates by the song's hash when the length agrees within five seconds,
// else by keyword, and the downloaded LRC (base64) with its credit lines dropped.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { LyricsQuery, LyricsResult } from '@raygun/sdk';

const LYRICS = 'https://krcs.kugou.com';
const SONGS = 'https://mobileservice.kugou.com/api/v3/search/song';
/** Seconds a song's length may differ from KuGou's and still be the same recording. */
const TOLERANCE_S = 5;

type SongSearch = { data?: { info?: { hash: string; duration: number }[] } };
type LyricSearch = { candidates?: { id: number | string; accesskey: string }[] };
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
    buffer = (buffer << 6) | value;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      bytes.push((buffer >> bits) & 0xff);
    }
  }
  return bytes;
};

/** UTF-8 bytes to text (KuGou's lyrics are often Chinese, Japanese or Korean). */
const utf8 = (bytes: number[]): string => {
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
  return out;
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

const TIMESTAMP = /\[(\d+):(\d{1,2})(?:[.:](\d{1,3}))?\]/g;
/** Credit lines KuGou puts first ("作词 : …", "Written by：…"), not lyrics. */
const CREDIT = /^(作词|作曲|编曲|制作人|written by|lyrics by|composed by|producer)\s*[:：]/i;

/** The LRC as timed lines, without credits or a first line that only names the song. */
const parseLrc = (lrc: string, song: string): LyricsResult | null => {
  const lines: { timeMs: number; text: string }[] = [];
  for (const row of lrc.split(/\r?\n/)) {
    const stamps = [...row.matchAll(TIMESTAMP)];
    if (stamps.length === 0) continue;
    const text = row.replace(TIMESTAMP, '').trim();
    if (CREDIT.test(text)) continue;
    for (const [, minutes, seconds, fraction = '0'] of stamps) {
      const fractionMs = Number(fraction.padEnd(3, '0').slice(0, 3));
      lines.push({ timeMs: (Number(minutes) * 60 + Number(seconds)) * 1000 + fractionMs, text });
    }
  }
  lines.sort((a, b) => a.timeMs - b.timeMs);
  const simple = (text: string) => text.toLowerCase().replace(/[\s\-–、,]+/g, '');
  if (lines[0] && simple(lines[0].text) === simple(song)) lines.shift();
  return lines.some((line) => line.text) ? { synced: lines, plain: null } : null;
};

const download = async (candidate: { id: number | string; accesskey: string }, song: string) => {
  const params = `ver=1&man=yes&client=pc&fmt=lrc&id=${candidate.id}&accesskey=${candidate.accesskey}`;
  const body = await get<Download>(`${LYRICS}/download?${params}`);
  return body.content ? parseLrc(utf8(fromBase64(body.content)), song) : null;
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
          const result = await download(candidate, words);
          if (result) return result;
        }
      }
    }
    const found = await get<LyricSearch>(
      `${LYRICS}/search?ver=1&man=yes&client=mobi&keyword=${encoded}`,
    );
    const candidate = found.candidates?.[0];
    return candidate ? download(candidate, words) : null;
  },
});

registerProvider(lyrics);
