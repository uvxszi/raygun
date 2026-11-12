// The SDK's models must stay compatible with what the app reads (`@raygun/bindings`, generated
// from Rust), and the CLI's table of calls (`lib/kinds.json`, which `raygun-ext validate` and the
// host's review read) with the provider types. These checks fail `tsc` (or the test) when either
// drifts.
import type { MediaSource as AppMediaSource, OnlineTrack } from '@raygun/bindings';

import kinds from '../lib/kinds.json';
import { defineProvider } from '../src';
import type { MediaSource, Provider, ProviderKind, Track } from '../src';

type Assignable<From, To> = [From] extends [To] ? true : false;
const assertTrue = <T extends true>(): T => true as T;

/** A kind's calls: everything on its provider type but what names it. */
type Of<K extends ProviderKind> = Extract<Provider, { kind: K }>;
type Methods<K extends ProviderKind> = Exclude<keyof Of<K>, 'kind' | 'id' | 'name' | 'searchPill'>;
type RequiredMethods<K extends ProviderKind> = {
  [M in Methods<K>]-?: object extends Pick<Of<K>, M> ? never : M;
}[Methods<K>];
type OptionalMethods<K extends ProviderKind> = Exclude<Methods<K>, RequiredMethods<K>>;

/** The calls of every kind, checked against the types: each one listed is the kind's. */
const CALLS = {
  'music.metadata': {
    required: ['search'],
    optional: [
      'searchArtists',
      'searchAlbums',
      'album',
      'artistReleases',
      'artistEvents',
      'searchPlaylists',
      'playlist',
    ],
  },
  'music.stream': { required: ['candidates'], optional: ['resolve'] },
  lyrics: { required: ['lyrics'], optional: [] },
  'music.charts': { required: ['charts', 'chart'], optional: ['tagSongs'] },
  'music.tags': { required: ['tags'], optional: [] },
  'music.info': { required: [], optional: ['artist', 'credits'] },
  'music.recommend': {
    required: [],
    optional: ['similarArtists', 'topTracks', 'newReleases', 'radio'],
  },
  dictionary: { required: ['define'], optional: [] },
  answers: { required: ['answer'], optional: [] },
  'manga.source': {
    required: ['popular', 'search', 'details', 'chapters', 'pages'],
    optional: ['latest', 'filters'],
  },
  'novel.source': {
    required: ['popular', 'search', 'details', 'chapters', 'passage'],
    optional: ['latest', 'filters'],
  },
  'video.source': {
    required: ['popular', 'search', 'details', 'episodes', 'servers', 'videos'],
    optional: ['latest', 'filters', 'comments'],
  },
  'podcast.directory': { required: ['search'], optional: ['top'] },
  'audiobook.directory': {
    required: ['search', 'popular'],
    optional: ['latest', 'subject', 'chapters'],
  },
} as const satisfies {
  [K in ProviderKind]: {
    required: readonly RequiredMethods<K>[];
    optional: readonly OptionalMethods<K>[];
  };
};

/** And none is missing: every call of every type is listed. */
type Complete = {
  [K in ProviderKind]: [RequiredMethods<K>] extends [(typeof CALLS)[K]['required'][number]]
    ? [OptionalMethods<K>] extends [(typeof CALLS)[K]['optional'][number]]
      ? true
      : false
    : false;
}[ProviderKind];

describe('SDK types', () => {
  it('match the app models', () => {
    assertTrue<Assignable<MediaSource, AppMediaSource>>();
    assertTrue<Assignable<Track['title'], OnlineTrack['title']>>();
    assertTrue<Assignable<NonNullable<Track['artist']>, NonNullable<OnlineTrack['artist']>>>();
    assertTrue<Assignable<NonNullable<Track['album']>, NonNullable<OnlineTrack['album']>>>();
    assertTrue<
      Assignable<NonNullable<Track['durationMs']>, NonNullable<OnlineTrack['durationMs']>>
    >();
    assertTrue<
      Assignable<NonNullable<Track['artworkUrl']>, NonNullable<OnlineTrack['artworkUrl']>>
    >();
    assertTrue<Assignable<NonNullable<Track['source']>, NonNullable<OnlineTrack['source']>>>();
  });

  it('list every call of every kind as the CLI does', () => {
    assertTrue<Complete>();
    // Site scripts register no providers: they run in the pages of Raygun's browser.
    const table = Object.fromEntries(
      Object.entries(kinds)
        .filter(([kind]) => kind !== 'site.scripts')
        .map(([kind, info]) => [kind, { required: info.required, optional: info.optional }]),
    );
    expect(table).toEqual(CALLS);
    // Every call has the arguments `raygun-ext test` fills in.
    for (const [kind, info] of Object.entries(kinds)) {
      expect(Object.keys(info.args).sort(), kind).toEqual(
        [...info.required, ...info.optional].sort(),
      );
    }
  });

  it('defineProvider returns the provider unchanged', () => {
    const provider = defineProvider({
      kind: 'music.metadata',
      id: 'p',
      search: async () => ({ tracks: [] }),
    });
    expect(provider.id).toBe('p');
  });
});
