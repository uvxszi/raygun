import { zlibSync } from 'fflate';

import {
  BlobPolyfill,
  DecompressionStreamPolyfill,
  HeadersPolyfill,
  ResponsePolyfill,
  TextDecoderPolyfill,
  TextEncoderPolyfill,
  URLPolyfill,
  URLSearchParamsPolyfill,
  atobPolyfill,
  btoaPolyfill,
} from '../src/shim/polyfills';

describe('URL', () => {
  it('parses absolute URLs', () => {
    const url = new URLPolyfill('https://User:Pw@Lrclib.NET:8443/api/../api/get?a=1&b=two#frag');
    expect(url.protocol).toBe('https:');
    expect(url.hostname).toBe('lrclib.net');
    expect(url.port).toBe('8443');
    expect(url.host).toBe('lrclib.net:8443');
    expect(url.pathname).toBe('/api/get');
    expect(url.search).toBe('?a=1&b=two');
    expect(url.hash).toBe('#frag');
    expect(url.origin).toBe('https://lrclib.net:8443');
    expect(url.searchParams.get('b')).toBe('two');
    expect(url.href).toBe('https://User:Pw@lrclib.net:8443/api/get?a=1&b=two#frag');
  });

  it('resolves relative URLs against a base', () => {
    const base = 'https://lrclib.net/api/v1/search?q=x';
    expect(new URLPolyfill('/api/get', base).href).toBe('https://lrclib.net/api/get');
    expect(new URLPolyfill('get', base).href).toBe('https://lrclib.net/api/v1/get');
    expect(new URLPolyfill('../v2/x', base).href).toBe('https://lrclib.net/api/v2/x');
    expect(new URLPolyfill('?q=y', base).href).toBe('https://lrclib.net/api/v1/search?q=y');
    expect(new URLPolyfill('//cdn.example.net/a', base).href).toBe('https://cdn.example.net/a');
    expect(() => new URLPolyfill('relative/only')).toThrow('Invalid URL');
  });

  it('keeps search and searchParams in step, as plugins set them both ways', () => {
    const url = new URLPolyfill('/search', 'https://lyrics.kugou.com');
    url.search = new URLSearchParamsPolyfill({ keyword: 'Hello, Dolly', ver: '1' }).toString();
    expect(url.toString()).toBe('https://lyrics.kugou.com/search?keyword=Hello%2C+Dolly&ver=1');
    url.searchParams.set('ver', '2');
    url.searchParams.append('man', 'yes');
    expect(url.search).toBe('?keyword=Hello%2C+Dolly&ver=2&man=yes');
  });
});

describe('URLSearchParams', () => {
  it('parses, edits and serializes like the form encoding', () => {
    const params = new URLSearchParamsPolyfill('?a=1&b=x+y&a=2&c=%E2%9C%93');
    expect(params.getAll('a')).toEqual(['1', '2']);
    expect(params.get('b')).toBe('x y');
    expect(params.get('c')).toBe('✓');
    params.set('a', '3');
    params.delete('c');
    expect(params.toString()).toBe('a=3&b=x+y');
    expect([...params]).toEqual([
      ['a', '3'],
      ['b', 'x y'],
    ]);
    expect(new URLSearchParamsPolyfill([['k', "it's (ok)!"]]).toString()).toBe(
      'k=it%27s+%28ok%29%21',
    );
  });
});

describe('text, base64 and headers', () => {
  it('round-trips UTF-8 and base64', () => {
    const text = 'Señorita · 夜に駆ける 🎵';
    const bytes = new TextEncoderPolyfill().encode(text);
    expect(Array.from(bytes)).toEqual(Array.from(new TextEncoder().encode(text)));
    expect(new TextDecoderPolyfill().decode(bytes)).toBe(text);
    expect(btoaPolyfill('Hello, Raygun!')).toBe(btoa('Hello, Raygun!'));
    expect(atobPolyfill(btoa('\u0000ÿ binary'))).toBe('\u0000ÿ binary');
  });

  it('treats header names case-insensitively', () => {
    const headers = new HeadersPolyfill({ 'Content-Type': 'application/json' });
    headers.append('Set-Cookie', 'a=1');
    headers.append('set-cookie', 'b=2');
    expect(headers.get('content-type')).toBe('application/json');
    expect(headers.get('SET-COOKIE')).toBe('a=1, b=2');
    expect(headers.toRecord()).toEqual({
      'content-type': 'application/json',
      'set-cookie': 'a=1, b=2',
    });
  });
});

describe('Blob → DecompressionStream → Response', () => {
  it('inflates deflate data the way the Kugou plugin does', async () => {
    const original = new TextEncoder().encode('[0,1000]<0,500,0>Yoru ni kakeru');
    const blob = new BlobPolyfill([zlibSync(original)]);
    const inflated = blob.stream().pipeThrough(new DecompressionStreamPolyfill('deflate'));
    const text = await new ResponsePolyfill(inflated).text();
    expect(text).toBe('[0,1000]<0,500,0>Yoru ni kakeru');
  });

  it('reads JSON and status like fetch responses', async () => {
    const response = new ResponsePolyfill('{"ok":true}', { status: 201, headers: { 'X-A': '1' } });
    expect(response.ok).toBe(true);
    expect(response.headers.get('x-a')).toBe('1');
    expect(await response.json()).toEqual({ ok: true });
    expect(new ResponsePolyfill('', { status: 404 }).ok).toBe(false);
  });
});
