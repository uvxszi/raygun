import type { Piece } from './client';
import { PIECE, answerPort, parseRange, reply, totalOf } from './media';
import type { Fetcher, PortLike } from './media';

const BASE = 'https://example.org/raygun/';
const FILE = Uint8Array.from({ length: PIECE * 2 + 1000 }, (_, at) => at % 251);

/** The app's proxy for one file, with ranges, recording what was asked. */
const appFile = (asked: Array<string | null>): Fetcher => {
  return async (_path, range): Promise<Piece> => {
    asked.push(range);
    const parsed = parseRange(range);
    if (!parsed) {
      return { status: 200, headers: { 'content-type': 'video/mp4' }, body: FILE };
    }
    const end = Math.min(parsed.end ?? FILE.length - 1, FILE.length - 1);
    return {
      status: 206,
      headers: {
        'content-type': 'video/mp4',
        'content-range': `bytes ${parsed.start}-${end}/${FILE.length}`,
      },
      body: FILE.slice(parsed.start, end + 1),
    };
  };
};

const drain = async (chunks: AsyncIterator<Uint8Array>) => {
  const parts: Uint8Array[] = [];
  for (;;) {
    const next = await chunks.next();
    if (next.done) {
      const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
      let at = 0;
      for (const part of parts) {
        out.set(part, at);
        at += part.length;
      }
      return out;
    }
    parts.push(next.value);
  }
};

describe('media through a linked app', () => {
  it('reads ranges and lengths', () => {
    expect(parseRange('bytes=10-')).toEqual({ start: 10, end: null });
    expect(parseRange('bytes=10-20')).toEqual({ start: 10, end: 20 });
    expect(parseRange('bytes=-500')).toBeNull();
    expect(parseRange('bytes=20-10')).toBeNull();
    expect(totalOf('bytes 0-99/1000')).toBe(1000);
    expect(totalOf('bytes */1000')).toBe(1000);
    expect(totalOf(undefined)).toBeNull();
  });

  it('answers an open range in pieces, asking for each as it is read', async () => {
    const asked: Array<string | null> = [];
    const answer = await reply(appFile(asked), 'media/v', 'bytes=100-', BASE);
    expect(answer.status).toBe(206);
    expect(answer.headers['content-range']).toBe(`bytes 100-${FILE.length - 1}/${FILE.length}`);
    expect(answer.headers['content-length']).toBe(String(FILE.length - 100));
    expect(asked).toEqual([`bytes=100-${100 + PIECE - 1}`]);
    const body = await drain(answer.chunks);
    expect(body).toEqual(FILE.slice(100));
    expect(asked).toHaveLength(3);
  });

  it('answers a closed range exactly', async () => {
    const answer = await reply(appFile([]), 'media/v', 'bytes=5-9', BASE);
    expect(answer.headers['content-range']).toBe(`bytes 5-9/${FILE.length}`);
    expect(await drain(answer.chunks)).toEqual(FILE.slice(5, 10));
  });

  it('answers whole when no range is asked for, and rewrites playlists', async () => {
    const playlist: Fetcher = async () => ({
      status: 200,
      headers: { 'content-type': 'application/vnd.apple.mpegurl' },
      body: new TextEncoder().encode('#EXTM3U\nhttp://127.0.0.1:5/media/t/https/x/a.ts\n'),
    });
    const answer = await reply(playlist, 'media/t/https/x/a.m3u8', null, BASE);
    const text = new TextDecoder().decode(await drain(answer.chunks));
    expect(text).toBe(`#EXTM3U\n${BASE}__remote/media/t/https/x/a.ts\n`);
    expect(answer.headers['content-length']).toBe(String(new TextEncoder().encode(text).length));
  });

  it('passes errors on as they came', async () => {
    const missing: Fetcher = async () => ({ status: 404, headers: {}, body: new Uint8Array(0) });
    const answer = await reply(missing, 'cover/x', 'bytes=0-', BASE);
    expect(answer.status).toBe(404);
  });

  it('answers on a port: the head, then a chunk per pull, then the end', async () => {
    const sent: unknown[] = [];
    const port: PortLike = {
      postMessage: (message) => sent.push(message),
      onmessage: null,
      close: () => undefined,
    };
    const piece: Piece = {
      status: 200,
      headers: { 'content-type': 'image/png' },
      body: FILE.slice(0, 10),
    };
    answerPort(
      port,
      reply(async () => piece, 'cover/x', null, BASE),
    );
    await vi.waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      head: { status: 200, headers: { 'content-type': 'image/png', 'content-length': '10' } },
    });
    port.onmessage?.({ data: { pull: true } } as MessageEvent);
    port.onmessage?.({ data: { pull: true } } as MessageEvent);
    await vi.waitFor(() => expect(sent).toHaveLength(3));
    expect(new Uint8Array((sent[1] as { chunk: ArrayBuffer }).chunk)).toEqual(FILE.slice(0, 10));
    expect(sent[2]).toEqual({ end: true });
  });
});
