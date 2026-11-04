import { describe, expect, it } from 'vitest';

import { keysOf, plainTitle, sameSong, trackKey } from './trackKey';

const songKey = (title: string, artist: string | null) => keysOf(title, artist)[0] as string;

describe('trackKey', () => {
  it('matches the resolver: folded artist, title without decorations', () => {
    expect(trackKey('Get Lucky (Radio Edit) feat. Pharrell', 'Daft Punk')).toBe(
      'daft punk|get lucky',
    );
    expect(trackKey('Café del Mar', 'Energy 52 & Friends')).toBe(
      'energy 52 and friends|cafe del mar',
    );
    expect(trackKey('Song', null)).toBe('|song');
  });
});

// The same cases as `collections::identity`'s tests in Rust: both sides must agree.
describe('one identity for saved songs', () => {
  it('drops a decoration after a dash, like brackets', () => {
    const cases: [string, string][] = [
      ['Bohemian Rhapsody - Remastered 2011', 'Bohemian Rhapsody'],
      ['Space Oddity - 2015 Remaster', 'Space Oddity'],
      ['Shallow - From A Star Is Born Soundtrack', 'Shallow'],
      ['One - Live', 'One'],
      ['Song – Radio Edit', 'Song'],
      ['Tom Sawyer - Live - Remastered', 'Tom Sawyer'],
      ['Love Me - Do', 'Love Me - Do'],
      ['Ode to Joy - Symphony No 9', 'Ode to Joy - Symphony No 9'],
      ['- Live', '- Live'],
      ['Song', 'Song'],
    ];
    for (const [title, plain] of cases) expect(plainTitle(title)).toBe(plain);
    expect(keysOf('Get Lucky - Radio Edit', 'Daft Punk')).toEqual([
      'daft punk|get lucky',
      'daft punk|get lucky radio edit',
    ]);
    expect(keysOf('Get Lucky', 'Daft Punk')).toEqual(['daft punk|get lucky']);
  });

  it('knows one song under several artist credits', () => {
    const spotify = songKey('Get Lucky', 'Daft Punk, Pharrell Williams, Nile Rodgers');
    expect(sameSong(spotify, songKey('Get Lucky', 'Daft Punk'))).toBe(true);
    expect(sameSong(songKey('Get Lucky', 'Pharrell Williams'), spotify)).toBe(true);
    expect(
      sameSong(
        songKey('Under Pressure', 'Queen & David Bowie'),
        songKey('Under Pressure', 'David Bowie'),
      ),
    ).toBe(true);
    expect(sameSong(songKey('Yesterday', 'Beatles'), songKey('Yesterday', 'The Beatles'))).toBe(
      true,
    );
    expect(sameSong(songKey('Hello', 'Adele'), songKey('Hello', 'Lionel Richie'))).toBe(false);
    expect(
      sameSong(
        songKey('Paint It Black', 'The Rolling Stones'),
        songKey('Paint It Black', 'The Beatles'),
      ),
    ).toBe(false);
    expect(
      sameSong(songKey('Get Lucky', 'Daft Punk'), songKey('Lose Yourself to Dance', 'Daft Punk')),
    ).toBe(false);
    expect(sameSong(songKey('Song', null), songKey('Song', 'Someone'))).toBe(false);
    expect(sameSong(songKey('Song', null), songKey('Song', null))).toBe(true);
    expect(sameSong('a|', 'a b|')).toBe(false);
  });
});
