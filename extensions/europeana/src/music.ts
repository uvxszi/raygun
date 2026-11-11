// Recordings from Europeana's music collections under an open licence, most of them from
// Romanian Radio's archive and other European sound archives. Only recordings Europeana links as
// an audio file are offered (an MP3 by Europeana's own reading, and a file by its address); ones
// it links as a web page, or whose institution refuses outside players, are left out. A recording
// without a named performer is credited to the institution that holds it.
import { defineProvider } from '@raygun/sdk';
import type { Track } from '@raygun/sdk';

import { europeana, isOpen, isWeb, people, pick, plainId, searchWords } from './api';
import type { LangMap } from './api';

type SearchItem = {
  id?: string;
  title?: string[];
  dcTitleLangAware?: LangMap;
  dcCreator?: string[];
  dcCreatorLangAware?: LangMap;
  dataProvider?: string[];
  rights?: string[];
  edmIsShownBy?: string[];
  edmPreview?: string[];
  language?: string[];
};
type SearchAnswer = { items?: SearchItem[] };

const AUDIO = /\.(mp3|m4a|ogg|oga|opus|flac|wav)$/i;

const trackOf = (item: SearchItem): Track[] => {
  const id = item.id ? plainId(item.id) : '';
  const file = item.edmIsShownBy?.find(isWeb);
  if (!id || !file || !AUDIO.test(file.split(/[?#]/)[0] ?? '')) return [];
  if (!isOpen(item.rights?.[0])) return [];
  // The recording's own language first: titles in other languages are often translations.
  const languages = [...(item.language ?? []), 'def'];
  const title = pick(item.dcTitleLangAware, languages) ?? pick(item.title, []);
  if (!title) return [];
  const creators = people(item.dcCreatorLangAware ?? item.dcCreator, languages);
  return [
    {
      id,
      title,
      artist: creators.slice(0, 3).join(', ') || item.dataProvider?.[0]?.trim() || null,
      artworkUrl: item.edmPreview?.find(isWeb) ?? null,
      source: { kind: 'url', url: file, headers: {} },
    },
  ];
};

export const recordings = defineProvider({
  kind: 'music.metadata',
  id: 'music',
  name: 'Europeana',

  async search({ query, limit }) {
    const words = searchWords(query);
    if (!words) return { tracks: [] };
    const answer = await europeana<SearchAnswer>('search.json', {
      query: words,
      theme: 'music',
      qf: ['TYPE:SOUND', 'MIME_TYPE:audio/mpeg'],
      reusability: 'open',
      media: true,
      rows: Math.min(100, Math.max(10, limit * 2)),
      profile: 'minimal',
    });
    const seen = new Set<string>();
    const tracks = (answer.items ?? []).flatMap(trackOf).filter((track) => {
      const key = String(track.id);
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
    return { tracks: tracks.slice(0, Math.max(1, limit)) };
  },
});
