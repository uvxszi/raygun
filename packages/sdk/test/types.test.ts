// The SDK's models must stay compatible with what the app reads (`@raygun/bindings`, generated
// from Rust). These checks fail `tsc` when a Rust model change would break extensions.
import type { MediaSource as AppMediaSource, OnlineTrack } from '@raygun/bindings';

import { defineProvider } from '../src';
import type { MediaSource, Track } from '../src';

type Assignable<From, To> = [From] extends [To] ? true : false;
const assertTrue = <T extends true>(): T => true as T;

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

  it('defineProvider returns the provider unchanged', () => {
    const provider = defineProvider({
      kind: 'music.metadata',
      id: 'p',
      search: async () => ({ tracks: [] }),
    });
    expect(provider.id).toBe('p');
  });
});
