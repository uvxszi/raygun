import type { OnlineTrack, ProviderSearchResult } from '@raygun/bindings';

import { sourceAnswers } from './sourceAnswers';

const song = (title: string, artist: string, id: string): OnlineTrack => ({
  externalId: id,
  title,
  artist,
  album: null,
  durationMs: 200_000,
  artworkUrl: null,
  source: { kind: 'url', url: `https://example.com/${id}.mp3`, headers: {} },
  explicit: false,
});

const from = (name: string, tracks: OnlineTrack[], error: string | null = null) =>
  ({
    provider: {
      id: name.toLowerCase(),
      extensionId: name.toLowerCase(),
      kind: 'music.search',
      name,
    },
    tracks,
    error,
  }) as ProviderSearchResult;

describe('Choose a source', () => {
  it("lists the same song once per source, best match first, each source's own few", () => {
    const answers = sourceAnswers('nina simone sinnerman', [
      from('YouTube', [
        song('Sinnerman (Live)', 'Nina Simone', 'y1'),
        song('Sinnerman', 'Nina Simone', 'y2'),
        song('Sinnerman', 'Nina Simone', 'y3'),
        ...Array.from({ length: 8 }, (_, index) => song(`Other ${index}`, 'Someone', `yo${index}`)),
      ]),
      from('Indie Tracks', [song('Sinnerman', 'Nina Simone', 'a1')]),
      from('Broken', [song('Sinnerman', 'Nina Simone', 'b1')], 'timed out'),
    ]);
    // Indie Tracks's copy shows beside YouTube's; the same song twice from one source shows once.
    expect(answers.filter((answer) => answer.track.title === 'Sinnerman')).toEqual([
      expect.objectContaining({
        source: 'YouTube',
        track: expect.objectContaining({ externalId: 'y2' }),
      }),
      expect.objectContaining({
        source: 'Indie Tracks',
        track: expect.objectContaining({ externalId: 'a1' }),
      }),
    ]);
    // A source that failed shows nothing; one with many shows its best few.
    expect(answers.some((answer) => answer.source === 'Broken')).toBe(false);
    expect(answers.filter((answer) => answer.source === 'YouTube')).toHaveLength(5);
  });

  it('keeps only what can be used', () => {
    const youtube = { ...song('Swim', 'BTS', 'y'), source: { kind: 'youTube', videoId: 'v' } };
    const answers = sourceAnswers(
      'bts swim',
      [from('YouTube', [youtube as OnlineTrack]), from('Indie Tracks', [song('Swim', 'BTS', 'a')])],
      (track) => track.source?.kind !== 'youTube',
    );
    expect(answers.map((answer) => answer.source)).toEqual(['Indie Tracks']);
  });
});
