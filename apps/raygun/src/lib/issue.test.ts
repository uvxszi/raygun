import i18n from '../i18n';
import { issueKind, readIssue } from './issue';

const t = i18n.t.bind(i18n);

describe('reading errors', () => {
  it('takes Rust’s sentence as it is, its details below', () => {
    expect(
      readIssue(
        'Comic Shelf wants a browser check, which Raygun can’t do\ncomics.example answered HTTP 403\nTypeError: x',
        t,
      ),
    ).toEqual({
      summary: 'Comic Shelf wants a browser check, which Raygun can’t do',
      details: 'comics.example answered HTTP 403\nTypeError: x',
      kind: null,
    });
  });

  it('names what a one-line error is about', () => {
    expect(readIssue('Tunes answered HTTP 503', t)).toMatchObject({
      summary: 'Tunes is having problems — try later',
      details: 'Tunes answered HTTP 503',
      kind: 'server-error',
    });
    expect(readIssue('HTTP 404 from https://www.gutenberg.org/ebooks/1', t).summary).toBe(
      'Not found on www.gutenberg.org',
    );
    expect(readIssue('The server answered 429 Too Many Requests', t).summary).toBe(
      'The server is limiting requests — try later',
    );
    expect(readIssue('The server answered 404', t).summary).toBe('Not found on the server');
    // A message saying what failed isn't taken for its source.
    expect(readIssue('Download failed: The server answered 404', t).summary).toBe(
      'Not found on the source',
    );
    // The page's own name for the source wins.
    expect(readIssue('could not reach api.example: dns error', t, 'Indie Tracks').summary).toBe(
      'Couldn’t find Indie Tracks’s site — check your connection',
    );
    expect(readIssue("cannot read property 'title' of undefined", t, 'Royal Road').summary).toBe(
      'Royal Road’s extension failed',
    );
    expect(readIssue('operation timed out', t).summary).toBe('The source didn’t answer in time');
  });

  it('keeps plain sentences, with nothing more to tell', () => {
    expect(readIssue('This chapter has no pages', t)).toEqual({
      summary: 'This chapter has no pages',
      details: null,
      kind: null,
    });
    expect(issueKind('Unknown chart 4040')).toBeNull();
    expect(issueKind('Just a moment... (Cloudflare)')).toBe('bot-check');
    expect(issueKind('Comic Shelf is rate limiting requests; try again shortly')).toBe(
      'rate-limited',
    );
    expect(
      issueKind('No connection could be made because the target machine actively refused it'),
    ).toBe('refused');
  });
});
