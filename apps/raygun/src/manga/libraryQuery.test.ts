import { libraryEntry } from '../test/libraryEntry';
import { matchesQuery, parseLibraryQuery } from './libraryQuery';

const FRIEREN = libraryEntry({
  title: "Frieren: Beyond Journey's End",
  authors: ['Kanehito Yamada'],
  artists: ['Tsukasa Abe'],
  genres: ['Fantasy', 'Adventure'],
  sourceName: 'MangaDex',
  status: 'ongoing',
  list: 'reading',
  favorite: true,
  chapters: 120,
  unread: 3,
  downloaded: 2,
  addedAt: new Date(2026, 0, 15, 12).getTime(),
});
const BERSERK = libraryEntry({
  title: 'Berserk',
  authors: ['Kentaro Miura'],
  genres: ['Dark Fantasy'],
  sourceName: 'Local comics',
  local: true,
  status: 'unknown',
  chapters: 40,
  unread: 0,
  addedAt: new Date(2026, 1, 20, 12).getTime(),
});

const shown = (query: string) =>
  [FRIEREN, BERSERK]
    .filter((entry) => matchesQuery(entry, parseLibraryQuery(query)))
    .map((entry) => entry.title.split(':')[0]);

describe('library search language', () => {
  it('matches words against titles, people, genres and sources', () => {
    expect(shown('')).toEqual(['Frieren', 'Berserk']);
    expect(shown('frieren')).toEqual(['Frieren']);
    expect(shown('miura')).toEqual(['Berserk']);
    expect(shown('mangadex')).toEqual(['Frieren']);
    expect(shown('fantasy')).toEqual(['Frieren', 'Berserk']);
    expect(shown('local')).toEqual(['Berserk']);
    expect(shown('fantasy adventure'), 'words all have to match').toEqual(['Frieren']);
  });

  it('understands fields, negation, either-or and groups', () => {
    expect(shown('genre:"dark fantasy"')).toEqual(['Berserk']);
    expect(shown('author:yamada')).toEqual(['Frieren']);
    expect(shown('-author:yamada')).toEqual(['Berserk']);
    expect(shown('list:reading')).toEqual(['Frieren']);
    expect(shown('list:favorite')).toEqual(['Frieren']);
    expect(shown('list:""'), 'an empty value finds titles without one').toEqual(['Berserk']);
    expect(shown('berserk | frieren')).toEqual(['Frieren', 'Berserk']);
    expect(shown('berserk || -fantasy')).toEqual(['Berserk']);
    expect(shown('-(miura || yamada)')).toEqual([]);
    expect(shown('(fantasy, adventure'), 'a missing ")" is forgiven').toEqual(['Frieren']);
    expect(shown('nosuch:thing'), 'an unknown field is plain text').toEqual([]);
    expect(shown('"journey\'s end"')).toEqual(['Frieren']);
  });

  it('compares counts and dates', () => {
    expect(shown('unread>0')).toEqual(['Frieren']);
    expect(shown('unread=0')).toEqual(['Berserk']);
    expect(shown('total>=100')).toEqual(['Frieren']);
    expect(shown('read>100')).toEqual(['Frieren']);
    expect(shown('downloaded>1 && author:yamada')).toEqual(['Frieren']);
    expect(shown('added>=2026-02-01')).toEqual(['Berserk']);
    expect(shown('added<2026-02-01')).toEqual(['Frieren']);
    expect(shown('added:2026-01-15')).toEqual(['Frieren']);
    expect(shown('unread>lots'), 'a value that is no number matches nothing').toEqual([]);
  });
});
