import { describe, expect, it } from 'vitest';

import { trackKey } from './trackKey';

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
