import { pageLinksInText, remotePath, toAppLinks, toPageLinks } from './rewrite';

const BASE = 'https://example.org/raygun/';
const PROXY = 'http://127.0.0.1:41234';

describe('links to an app', () => {
  it("makes the app's proxy links the page's own paths, deep inside answers", () => {
    const answer = {
      title: 'Song',
      artworkUrl: `${PROXY}/cover/abc`,
      pages: [`http://localhost:9/image/x?w=2`, 'https://elsewhere.org/cover/1'],
      nothing: null,
      count: 3,
    };
    expect(toPageLinks(answer, BASE)).toEqual({
      title: 'Song',
      artworkUrl: `${BASE}__remote/cover/abc`,
      pages: [`${BASE}__remote/image/x?w=2`, 'https://elsewhere.org/cover/1'],
      nothing: null,
      count: 3,
    });
  });

  it('leaves answers without such links as they were', () => {
    const answer = { list: [{ title: 'a' }], other: `${PROXY}/embed/youtube` };
    expect(toPageLinks(answer, BASE)).toBe(answer);
  });

  it("sends the page's paths back in the app's form once its proxy is known", () => {
    const args = { track: { artworkUrl: `${BASE}__remote/cover/abc`, title: 't' } };
    expect(toAppLinks(args, BASE, PROXY)).toEqual({
      track: { artworkUrl: `${PROXY}/cover/abc`, title: 't' },
    });
    expect(toAppLinks(args, BASE, null)).toBe(args);
  });

  it("rewrites a playlist's links", () => {
    const playlist = `#EXTM3U\n${PROXY}/media/t/https/cdn.example/a.ts\nhttp://127.0.0.1:41234/embed/youtube\n`;
    expect(pageLinksInText(playlist, BASE)).toBe(
      `#EXTM3U\n${BASE}__remote/media/t/https/cdn.example/a.ts\nhttp://127.0.0.1:41234/embed/youtube\n`,
    );
  });

  it('names the proxy path a request is for', () => {
    expect(remotePath(`${BASE}__remote/pdf/l0/2?w=800`, BASE)).toBe('pdf/l0/2?w=800');
    expect(remotePath(`${PROXY}/local/books/a.epub`, BASE)).toBe('local/books/a.epub');
    expect(remotePath(`${BASE}index.html`, BASE)).toBeNull();
  });
});
