// Last.fm's charts: the songs most played on Last.fm, in a country, and the top songs of a tag.
import { defineProvider } from '@raygun/sdk';
import type { Chart, Track } from '@raygun/sdk';

import { clamp, lastfm, songsIn, words } from './api';
import type { TrackAnswer } from './api';

/** Songs a chart gives at most. */
const MOST = 100;

/**
 * Countries by their ISO 3166 codes (the app's regions): how a chart names them, then the names
 * Last.fm may know them by (ISO 3166-1's English short names, older spellings first).
 */
const COUNTRIES: [code: string, shown: string, ...names: string[]][] = [
  ['us', 'the United States', 'United States'],
  ['gb', 'the United Kingdom', 'United Kingdom'],
  ['ca', 'Canada', 'Canada'],
  ['au', 'Australia', 'Australia'],
  ['nz', 'New Zealand', 'New Zealand'],
  ['ie', 'Ireland', 'Ireland'],
  ['de', 'Germany', 'Germany'],
  ['fr', 'France', 'France'],
  ['es', 'Spain', 'Spain'],
  ['it', 'Italy', 'Italy'],
  ['pt', 'Portugal', 'Portugal'],
  ['nl', 'the Netherlands', 'Netherlands'],
  ['be', 'Belgium', 'Belgium'],
  ['ch', 'Switzerland', 'Switzerland'],
  ['at', 'Austria', 'Austria'],
  ['dk', 'Denmark', 'Denmark'],
  ['no', 'Norway', 'Norway'],
  ['se', 'Sweden', 'Sweden'],
  ['fi', 'Finland', 'Finland'],
  ['is', 'Iceland', 'Iceland'],
  ['pl', 'Poland', 'Poland'],
  ['cz', 'Czechia', 'Czech Republic', 'Czechia'],
  ['sk', 'Slovakia', 'Slovakia'],
  ['hu', 'Hungary', 'Hungary'],
  ['ro', 'Romania', 'Romania'],
  ['bg', 'Bulgaria', 'Bulgaria'],
  ['gr', 'Greece', 'Greece'],
  ['hr', 'Croatia', 'Croatia'],
  ['si', 'Slovenia', 'Slovenia'],
  ['rs', 'Serbia', 'Serbia'],
  ['ua', 'Ukraine', 'Ukraine'],
  ['by', 'Belarus', 'Belarus'],
  ['ru', 'Russia', 'Russian Federation', 'Russia'],
  ['ee', 'Estonia', 'Estonia'],
  ['lv', 'Latvia', 'Latvia'],
  ['lt', 'Lithuania', 'Lithuania'],
  ['tr', 'Turkey', 'Turkey', 'Türkiye'],
  ['il', 'Israel', 'Israel'],
  ['mx', 'Mexico', 'Mexico'],
  ['br', 'Brazil', 'Brazil'],
  ['ar', 'Argentina', 'Argentina'],
  ['cl', 'Chile', 'Chile'],
  ['co', 'Colombia', 'Colombia'],
  ['pe', 'Peru', 'Peru'],
  ['ve', 'Venezuela', 'Venezuela', 'Venezuela, Bolivarian Republic of'],
  ['uy', 'Uruguay', 'Uruguay'],
  ['ec', 'Ecuador', 'Ecuador'],
  ['cr', 'Costa Rica', 'Costa Rica'],
  ['za', 'South Africa', 'South Africa'],
  ['ng', 'Nigeria', 'Nigeria'],
  ['eg', 'Egypt', 'Egypt'],
  ['ma', 'Morocco', 'Morocco'],
  ['ke', 'Kenya', 'Kenya'],
  ['in', 'India', 'India'],
  ['id', 'Indonesia', 'Indonesia'],
  ['my', 'Malaysia', 'Malaysia'],
  ['sg', 'Singapore', 'Singapore'],
  ['ph', 'the Philippines', 'Philippines'],
  ['th', 'Thailand', 'Thailand'],
  ['vn', 'Vietnam', 'Viet Nam', 'Vietnam'],
  ['kr', 'South Korea', 'Korea, Republic of', 'South Korea'],
  ['jp', 'Japan', 'Japan'],
  ['tw', 'Taiwan', 'Taiwan', 'Taiwan, Province of China'],
  ['hk', 'Hong Kong', 'Hong Kong'],
  ['cn', 'China', 'China'],
  ['sa', 'Saudi Arabia', 'Saudi Arabia'],
  ['ae', 'the United Arab Emirates', 'United Arab Emirates'],
];
const BY_CODE = new Map(COUNTRIES.map(([code, shown, ...names]) => [code, { shown, names }]));

/** The country of a region the app passes ("us", "GB", "uk"), when Last.fm has its chart. */
const countryOf = (region: string | null | undefined) => {
  const typed = (region ?? '').trim().toLowerCase();
  const code = typed === 'uk' ? 'gb' : typed;
  return BY_CODE.has(code) ? code : null;
};

/** Genres with a chart of their own: Last.fm's tag for each, and its name in the app. */
const GENRES: [tag: string, name: string][] = [
  ['rock', 'Rock'],
  ['pop', 'Pop'],
  ['hip-hop', 'Hip-Hop'],
  ['electronic', 'Electronic'],
  ['indie', 'Indie'],
  ['alternative', 'Alternative'],
  ['metal', 'Metal'],
  ['punk', 'Punk'],
  ['rnb', 'R&B'],
  ['soul', 'Soul'],
  ['funk', 'Funk'],
  ['jazz', 'Jazz'],
  ['blues', 'Blues'],
  ['folk', 'Folk'],
  ['country', 'Country'],
  ['singer-songwriter', 'Singer-Songwriter'],
  ['classical', 'Classical'],
  ['soundtrack', 'Soundtrack'],
  ['reggae', 'Reggae'],
  ['latin', 'Latin'],
  ['dance', 'Dance'],
  ['house', 'House'],
  ['techno', 'Techno'],
  ['ambient', 'Ambient'],
  ['k-pop', 'K-Pop'],
  ['j-pop', 'J-Pop'],
];

const tagTracks = async (tag: string, count: number) =>
  songsIn(await lastfm<TrackAnswer>('tag.getTopTracks', { tag, limit: count })).slice(0, count);

/** A country's chart, asked by each name Last.fm may know it by until one is known. */
const countryTracks = async (code: string, count: number): Promise<Track[]> => {
  const country = BY_CODE.get(code);
  if (!country) throw new Error('Last.fm has no chart for this country');
  for (const name of country.names) {
    const answer = await lastfm<TrackAnswer>('geo.getTopTracks', { country: name, limit: count });
    if (answer) return songsIn(answer).slice(0, count);
  }
  throw new Error(`Last.fm has no chart for ${country.shown}`);
};

export const charts = defineProvider({
  kind: 'music.charts',
  id: 'charts',
  name: 'Last.fm',

  async charts({ region }) {
    const country = countryOf(region);
    const shown = country ? BY_CODE.get(country)?.shown : null;
    const list: Chart[] = [
      {
        id: 'top',
        name: 'Top songs on Last.fm',
        description: 'The songs most played on Last.fm',
        group: 'top',
      },
      ...(country && shown
        ? [
            {
              id: `country:${country}`,
              name: `Top songs in ${shown}`,
              description: `The songs most played in ${shown} on Last.fm`,
              group: 'top' as const,
            },
          ]
        : []),
      ...GENRES.map(([tag, name]) => ({
        id: `tag:${tag}`,
        name: `${name} on Last.fm`,
        description: `The songs Last.fm's listeners tag ${tag} most`,
        tag: name,
        group: 'genre' as const,
      })),
    ];
    return { charts: list };
  },

  async chart({ id, limit }) {
    const count = clamp(limit || 50, 1, MOST);
    if (id === 'top') {
      const answer = await lastfm<TrackAnswer>('chart.getTopTracks', { limit: count });
      return { tracks: songsIn(answer).slice(0, count) };
    }
    if (id.startsWith('country:')) return { tracks: await countryTracks(id.slice(8), count) };
    const tag = id.startsWith('tag:') ? id.slice(4).trim() : '';
    if (!tag || tag.length > 60) throw new Error('Last.fm has no such chart');
    return { tracks: await tagTracks(tag, count) };
  },

  // A genre's or mood's songs: its tag's top songs, else those of the first of its other
  // spellings that has some.
  async tagSongs({ tag, aliases, limit }) {
    const count = clamp(limit || 25, 1, MOST);
    const tags = [...new Set([tag, ...aliases].map((name) => name.trim().toLowerCase()))]
      .filter((name) => words(name) !== '' && name.length <= 60)
      .slice(0, 5);
    for (const name of tags) {
      const tracks = await tagTracks(name, count);
      if (tracks.length > 0) return { tracks };
    }
    return { tracks: [] };
  },
});
