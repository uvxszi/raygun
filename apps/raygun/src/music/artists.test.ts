import { creditedArtists, tidyCredit } from './artists';

describe('credits', () => {
  it('lose the quotes a source left around a name, and keep the ones that belong to it', () => {
    expect(tidyCredit('"Five Finger Death Punch"')).toBe('Five Finger Death Punch');
    expect(tidyCredit('"Five Finger Death Punch')).toBe('Five Finger Death Punch');
    expect(tidyCredit('Five Finger Death Punch” ')).toBe('Five Finger Death Punch');
    expect(tidyCredit('“Daft Punk”')).toBe('Daft Punk');
    expect(tidyCredit('"Weird Al" Yankovic')).toBe('"Weird Al" Yankovic');
    expect(tidyCredit("'Til Tuesday")).toBe("'Til Tuesday");
    expect(tidyCredit("Guns N' Roses")).toBe("Guns N' Roses");
    expect(tidyCredit('"')).toBe('');
  });

  it('split into artists without stray quotes', () => {
    expect(creditedArtists('"Five Finger Death Punch')).toEqual(['Five Finger Death Punch']);
    expect(creditedArtists('"Daft Punk & Pharrell Williams"')).toEqual([
      'Daft Punk',
      'Pharrell Williams',
    ]);
    expect(creditedArtists('"Weird Al" Yankovic feat. "Someone"')).toEqual([
      '"Weird Al" Yankovic',
      'Someone',
    ]);
    expect(creditedArtists('(Daft Punk)')).toEqual(['Daft Punk']);
  });
});
