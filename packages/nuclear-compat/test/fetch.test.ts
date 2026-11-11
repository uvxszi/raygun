import { pluginFetch } from '../src/shim/api';
import { BlobPolyfill, URLSearchParamsPolyfill } from '../src/shim/polyfills';

type Sent = {
  url: string;
  init: {
    method: string;
    headers: Record<string, string>;
    body: string | null;
    bodyBase64?: string;
    binary?: boolean;
  };
};

/** `raygun.http.fetch` as the host answers: text, or bytes when they aren't UTF-8 text. */
const host = (answers: Record<string, string | Uint8Array>) => {
  const sent: Sent[] = [];
  vi.stubGlobal('raygun', {
    http: {
      fetch: async (url: string, init: Sent['init']) => {
        sent.push({ url, init });
        const answer = answers[url] ?? '';
        const bytes = typeof answer === 'string' ? null : answer;
        return {
          status: 200,
          ok: true,
          url,
          headers: { 'content-type': bytes ? 'application/octet-stream' : 'text/html' },
          text: async () => (typeof answer === 'string' ? answer : ''),
          json: async () => JSON.parse(String(answer)),
          bytes: async () => bytes,
        };
      },
    },
  });
  return sent;
};

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('fetch for plugins', () => {
  it('sends form, byte and blob bodies the way a browser does', async () => {
    const sent = host({});
    await pluginFetch('https://api.example.org/form', {
      method: 'POST',
      body: new URLSearchParamsPolyfill({ q: 'Hello, Dolly' }),
    });
    await pluginFetch('https://api.example.org/bytes', {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-protobuf' },
      body: Uint8Array.from([0, 1, 254, 255]),
    });
    await pluginFetch('https://api.example.org/blob', {
      method: 'PUT',
      body: new BlobPolyfill(['hi'], { type: 'text/plain' }),
    });
    expect(sent.map(({ init }) => [init.method, init.body, init.bodyBase64, init.headers])).toEqual(
      [
        [
          'POST',
          'q=Hello%2C+Dolly',
          undefined,
          { 'Content-Type': 'application/x-www-form-urlencoded;charset=UTF-8' },
        ],
        ['POST', null, 'AAH+/w==', { 'Content-Type': 'application/x-protobuf' }],
        ['PUT', null, 'aGk=', { 'Content-Type': 'text/plain' }],
      ],
    );
    expect(sent.every(({ init }) => init.binary === true)).toBe(true);
  });

  it('answers text as text and bytes as bytes, without turning one into the other', async () => {
    const page = `<html>${'Señorita '.repeat(10_000)}</html>`;
    host({
      'https://site.example/page': page,
      'https://site.example/zip': Uint8Array.from([0x50, 0x4b, 0x03, 0x04, 0xff]),
    });
    const text = await pluginFetch('https://site.example/page');
    expect(await text.text()).toBe(page);
    const binary = await pluginFetch({ url: 'https://site.example/zip' });
    expect(Array.from(new Uint8Array(await binary.arrayBuffer()))).toEqual([
      0x50, 0x4b, 0x03, 0x04, 0xff,
    ]);
  });
});
