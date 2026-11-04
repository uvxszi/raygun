import { query, resolveUrl, selectValue, tagsValue, toggleValue } from '../src';
import type { SelectFilter, TagsFilter, ToggleFilter } from '../src';

describe('query', () => {
  it('encodes names and values, repeats lists and leaves out empty values', () => {
    expect(query({ q: 'a b&c', page: 2, nsfw: false })).toBe('q=a%20b%26c&page=2&nsfw=false');
    expect(query({ 'fl[]': ['identifier', 'title'], rows: undefined, sort: null })).toBe(
      'fl%5B%5D=identifier&fl%5B%5D=title',
    );
    expect(query({})).toBe('');
  });
});

describe('resolveUrl', () => {
  const base = 'https://www.example.org/books/84/pg84-images.html?x=1#top';

  it.each([
    ['images/fig1.jpg', 'https://www.example.org/books/84/images/fig1.jpg'],
    ['../covers/84.jpg', 'https://www.example.org/books/covers/84.jpg'],
    ['../../../../up.jpg', 'https://www.example.org/up.jpg'],
    ['./same.html', 'https://www.example.org/books/84/same.html'],
    ['/root.css', 'https://www.example.org/root.css'],
    ['//cdn.example.org/a.png', 'https://cdn.example.org/a.png'],
    ['?page=2', 'https://www.example.org/books/84/pg84-images.html?page=2'],
    ['#chapter-2', 'https://www.example.org/books/84/pg84-images.html?x=1#chapter-2'],
    ['', 'https://www.example.org/books/84/pg84-images.html?x=1'],
    ['next.html?a=b#c', 'https://www.example.org/books/84/next.html?a=b#c'],
    ['https://other.example.com/x', 'https://other.example.com/x'],
    ['mailto:someone@example.org', 'mailto:someone@example.org'],
  ])('resolves %s', (href, expected) => {
    expect(resolveUrl(base, href)).toBe(expected);
  });

  it('resolves against a bare host and refuses a relative base', () => {
    expect(resolveUrl('https://example.org', 'a/b.jpg')).toBe('https://example.org/a/b.jpg');
    expect(resolveUrl('https://example.org/dir/', '..')).toBe('https://example.org/');
    expect(() => resolveUrl('/relative', 'x')).toThrow(/absolute base/);
  });
});

describe('filter values', () => {
  const sort: SelectFilter = {
    kind: 'select',
    key: 'sort',
    label: 'Sort by',
    options: [{ value: 'popular' }, { value: 'new' }],
    default: 'new',
  };
  const complete: ToggleFilter = { kind: 'toggle', key: 'complete', label: 'Complete only' };
  const genres: TagsFilter = {
    kind: 'tags',
    key: 'genres',
    label: 'Genres',
    options: [{ value: 'fantasy' }, { value: 'mystery' }],
  };

  it('reads the user’s choices, else the defaults', () => {
    expect(selectValue({ sort: 'popular' }, sort)).toBe('popular');
    expect(selectValue({ sort: 'unknown' }, sort)).toBe('new');
    expect(selectValue(undefined, sort)).toBe('new');
    expect(selectValue(undefined, { ...sort, default: undefined })).toBe('popular');
    expect(toggleValue({ complete: true }, complete)).toBe(true);
    expect(toggleValue({}, { ...complete, default: true })).toBe(true);
    expect(toggleValue(undefined, complete)).toBe(false);
    expect(
      tagsValue({ genres: { include: ['fantasy', 'horror'], exclude: ['mystery'] } }, genres),
    ).toEqual({ include: ['fantasy'], exclude: ['mystery'] });
    expect(tagsValue({ genres: 'fantasy' }, genres)).toEqual({ include: [], exclude: [] });
  });
});
