// Nuclear providers as Raygun providers: metadata → music.metadata, streaming → music.stream,
// lyrics → lyrics, dashboard → music.charts and discovery → music.recommend (M46.23). Method shapes
// follow Nuclear's plugin-sdk types (AGPL-3.0).
import type { LyricsResult, Provider, StreamCandidate, Track } from '@raygun/sdk';

import type {
  ArtworkSet,
  NuclearLyrics,
  NuclearProvider,
  NuclearStream,
  NuclearStreamCandidate,
  NuclearTrack,
} from './nuclear-types';

export const KIND_MAP: Record<string, Provider['kind']> = {
  metadata: 'music.metadata',
  streaming: 'music.stream',
  lyrics: 'lyrics',
  dashboard: 'music.charts',
  discovery: 'music.recommend',
};

/** Raygun provider ids are `[a-z0-9][a-z0-9_-]*`. */
export const providerId = (id: string) =>
  id
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, '-')
    .replace(/^[^a-z0-9]+/, '')
    .slice(0, 64) || 'provider';

const call = <T>(provider: NuclearProvider, method: string, ...args: unknown[]): Promise<T> => {
  const fn = provider[method];
  if (typeof fn !== 'function') {
    throw new Error(`${provider.name} has no ${method}`);
  }
  return Promise.resolve(fn.apply(provider, args) as T);
};

const has = (provider: NuclearProvider, method: string) => typeof provider[method] === 'function';

/** The largest artwork, preferring covers. */
export const pickArtwork = (artwork?: ArtworkSet): string | null => {
  const items = (artwork?.items ?? []).filter((item) => /^https?:\/\//.test(item.url));
  const covers = items.filter((item) => !item.purpose || item.purpose === 'cover');
  const pool = covers.length > 0 ? covers : items;
  const best = [...pool].sort((a, b) => (b.width ?? 0) - (a.width ?? 0))[0];
  return best?.url ?? null;
};

export const toRaygunTrack = (track: NuclearTrack): Track => ({
  id: track.source?.id,
  title: track.title,
  artist: track.artists?.map((artist) => artist.name).join(', ') || null,
  album: track.album?.title ?? null,
  durationMs: track.durationMs ?? null,
  artworkUrl: pickArtwork(track.artwork),
});

/** What Nuclear plugins expect to receive for the track being looked up. */
export const toNuclearTrack = (query: {
  title: string;
  artist: string | null;
  album: string | null;
  durationMs: number | null;
}): NuclearTrack => ({
  title: query.title,
  artists: query.artist ? [{ name: query.artist, roles: ['main'] }] : [],
  album: query.album ? { title: query.album, source: { provider: 'raygun', id: '' } } : undefined,
  durationMs: query.durationMs ?? undefined,
  source: { provider: 'raygun', id: '' },
});

const playable = (stream?: NuclearStream) =>
  stream &&
  (stream.protocol === 'http' || stream.protocol === 'https') &&
  /^https?:\/\//.test(stream.url);

/** Flattens Nuclear's sectioned lyrics into Raygun's lines or text. */
export const toRaygunLyrics = (lyrics: NuclearLyrics | undefined | null): LyricsResult | null => {
  if (!lyrics || lyrics.type === 'instrumental') return null;
  const lineText = (line: { segments: { text: string }[] }) =>
    line.segments
      .map((segment) => segment.text)
      .join('')
      .trim();
  if (lyrics.type === 'plain') {
    const plain = lyrics.sections
      .map((section) => section.lines.map(lineText).join('\n'))
      .filter(Boolean)
      .join('\n\n');
    return plain ? { plain } : null;
  }
  const synced = lyrics.sections.flatMap((section) =>
    section.lines
      .filter((line) => typeof line.startMs === 'number')
      .map((line) => ({ timeMs: line.startMs as number, text: lineText(line) })),
  );
  return synced.length > 0 ? { synced } : null;
};

/** Candidates are kept by id for `getStreamUrlV2`, which wants the whole candidate back. */
const candidates = new Map<string, NuclearStreamCandidate>();

/** The Raygun provider for a Nuclear one, or null when Raygun has no matching kind. */
export const wrapProvider = (provider: NuclearProvider): Provider | null => {
  const kind = KIND_MAP[provider.kind];
  const id = providerId(provider.id);
  const name = provider.name;

  if (kind === 'music.metadata') {
    return {
      kind,
      id,
      name,
      async search({ query, limit }) {
        let tracks: NuclearTrack[] = [];
        if (has(provider, 'searchTracks')) {
          tracks = await call<NuclearTrack[]>(provider, 'searchTracks', { query, limit });
        } else if (has(provider, 'search')) {
          const results = await call<{ tracks?: NuclearTrack[] }>(provider, 'search', {
            query,
            types: ['tracks'],
            limit,
          });
          tracks = results?.tracks ?? [];
        }
        return { tracks: (tracks ?? []).slice(0, limit).map(toRaygunTrack) };
      },
    };
  }

  if (kind === 'music.stream') {
    return {
      kind,
      id,
      name,
      async candidates(query) {
        const found = has(provider, 'searchForTrackV2')
          ? await call<NuclearStreamCandidate[]>(
              provider,
              'searchForTrackV2',
              toNuclearTrack(query),
            )
          : await call<NuclearStreamCandidate[]>(
              provider,
              'searchForTrack',
              query.artist ?? '',
              query.title,
              query.album ?? undefined,
            );
        const list = (found ?? []).filter((candidate) => !candidate.failed).slice(0, query.limit);
        for (const candidate of list) candidates.set(candidate.id, candidate);
        return {
          candidates: list.map((candidate): StreamCandidate => ({
            id: candidate.id,
            title: candidate.title,
            durationMs: candidate.durationMs ?? null,
            quality: candidate.stream?.qualityLabel ?? null,
            source: playable(candidate.stream)
              ? { kind: 'url', url: (candidate.stream as NuclearStream).url, headers: {} }
              : null,
          })),
        };
      },
      async resolve({ id: candidateId }) {
        const known = candidates.get(candidateId);
        const stream =
          has(provider, 'getStreamUrlV2') && known
            ? await call<NuclearStream>(provider, 'getStreamUrlV2', known)
            : await call<NuclearStream>(provider, 'getStreamUrl', candidateId);
        if (!playable(stream)) {
          throw new Error(
            `${name} returned a ${stream?.protocol ?? 'missing'} stream Raygun can't play`,
          );
        }
        return { source: { kind: 'url', url: stream.url, headers: {} } };
      },
    };
  }

  if (kind === 'lyrics') {
    return {
      kind,
      id,
      name,
      async lyrics(query) {
        const lyrics = await call<NuclearLyrics | undefined>(
          provider,
          'getLyrics',
          toNuclearTrack(query),
          {},
        );
        return toRaygunLyrics(lyrics);
      },
    };
  }

  // A dashboard's top tracks become a chart (its albums, artists and playlists have no place in
  // Raygun's charts yet).
  if (kind === 'music.charts') {
    return {
      kind,
      id,
      name,
      async charts() {
        if (!has(provider, 'fetchTopTracks')) return { charts: [] };
        return {
          charts: [
            {
              id: 'topTracks',
              name: `Top songs on ${name}`,
              description: `${name}'s most played songs`,
              group: 'top',
            },
          ],
        };
      },
      async chart({ id: chart, limit }) {
        if (chart !== 'topTracks') throw new Error(`Unknown chart ${chart}`);
        const tracks = await call<NuclearTrack[]>(provider, 'fetchTopTracks');
        return { tracks: (tracks ?? []).slice(0, limit).map(toRaygunTrack) };
      },
    };
  }

  // Discovery's recommendations become radios: songs like the artist's (and the song's).
  if (kind === 'music.recommend') {
    return {
      kind,
      id,
      name,
      async radio({ artist, title, limit }) {
        const context = [
          toNuclearTrack({ title: title ?? '', artist, album: null, durationMs: null }),
        ];
        const tracks = await call<NuclearTrack[]>(provider, 'getRecommendations', context, {
          variety: 0.5,
          limit,
        });
        return { tracks: (tracks ?? []).slice(0, limit).map(toRaygunTrack) };
      },
    };
  }

  return null;
};
