// Pictures and media from a linked app (docs/WEB.md, "The app as a back end"): the service worker
// (web-build/remoteWorker.ts) hands each request for a `__remote/…` path to the page with a
// MessagePort; the page fetches it through the app in pieces and answers on the port as the
// browser reads: the head first, then a piece per pull, so a long video is never held whole.
import { pageLinksInText } from './rewrite';
import type { Piece } from './client';

/** Bytes asked of the app at a time for a ranged request. */
export const PIECE = 512 * 1024;
/** HLS and DASH playlists, whose links are rewritten for the page. */
const PLAYLIST = /mpegurl|dash\+xml/i;
const PASSED_HEADERS = ['content-type', 'accept-ranges', 'last-modified', 'etag'];

export type Fetcher = (path: string, range: string | null) => Promise<Piece>;

/** An answer: its head, then its body as the browser reads it. */
export type Reply = {
  status: number;
  headers: Record<string, string>;
  chunks: AsyncIterator<Uint8Array>;
};

/** One `bytes=<start>-<end?>` range; other forms (suffixes, several ranges) give null. */
export const parseRange = (header: string | null): { start: number; end: number | null } | null => {
  const match = /^bytes=(\d+)-(\d*)$/.exec(header?.trim() ?? '');
  if (!match) return null;
  const start = Number(match[1]);
  const end = match[2] ? Number(match[2]) : null;
  if (end !== null && end < start) return null;
  return { start, end };
};

/** The whole length from a `Content-Range` (`bytes 0-99/1000`), when it's given. */
export const totalOf = (contentRange: string | undefined): number | null => {
  const match = /\/(\d+)\s*$/.exec(contentRange ?? '');
  return match ? Number(match[1]) : null;
};

const passed = (headers: Record<string, string>) =>
  Object.fromEntries(
    PASSED_HEADERS.flatMap((name) => (headers[name] ? [[name, headers[name]]] : [])),
  ) as Record<string, string>;

async function* once(body: Uint8Array) {
  if (body.length > 0) yield body;
}

/** A whole answer, its playlist links made the page's. */
const whole = (piece: Piece, base: string): Reply => {
  let body = piece.body;
  if (PLAYLIST.test(piece.headers['content-type'] ?? '')) {
    body = new TextEncoder().encode(pageLinksInText(new TextDecoder().decode(body), base));
  }
  const headers = passed(piece.headers);
  headers['content-length'] = String(body.length);
  if (piece.headers['content-range']) headers['content-range'] = piece.headers['content-range'];
  return { status: piece.status, headers, chunks: once(body) };
};

/** Answers a request for `path` (with the browser's Range header, if any) through `fetcher`. */
export const reply = async (
  fetcher: Fetcher,
  path: string,
  rangeHeader: string | null,
  base: string,
): Promise<Reply> => {
  const range = parseRange(rangeHeader);
  if (!range) return whole(await fetcher(path, null), base);
  const { start } = range;
  const firstEnd = Math.min(range.end ?? Number.MAX_SAFE_INTEGER, start + PIECE - 1);
  const first = await fetcher(path, `bytes=${start}-${firstEnd}`);
  const total = totalOf(first.headers['content-range']);
  // No ranges there (or an error): the answer as it came.
  if (first.status !== 206 || total === null) return whole(first, base);
  const end = Math.min(range.end ?? total - 1, total - 1);
  const headers = passed(first.headers);
  headers['accept-ranges'] = 'bytes';
  headers['content-range'] = `bytes ${start}-${end}/${total}`;
  headers['content-length'] = String(end - start + 1);
  async function* pieces() {
    let offset = start;
    let body = first.body;
    for (;;) {
      if (body.length === 0) throw new Error('the app sent nothing for a range');
      const wanted = end - offset + 1;
      yield body.length > wanted ? body.subarray(0, wanted) : body;
      offset += body.length;
      if (offset > end) return;
      const next = await fetcher(path, `bytes=${offset}-${Math.min(end, offset + PIECE - 1)}`);
      if (next.status !== 206) throw new Error(`the app answered ${next.status} for a range`);
      body = next.body;
    }
  }
  return { status: 206, headers, chunks: pieces() };
};

/** The page's end of a service worker's port. */
export type PortLike = {
  postMessage: (message: unknown, transfer: Transferable[]) => void;
  onmessage: ((event: MessageEvent) => void) | null;
  close: () => void;
};

const describe = (error: unknown) => (error instanceof Error ? error.message : String(error));

/** Answers on `port`: the head once `replying` has it, then a chunk for each pull. */
export const answerPort = (port: PortLike, replying: Promise<Reply>) => {
  let chunks: AsyncIterator<Uint8Array> | null = null;
  let finished = false;
  let turn: Promise<void> = Promise.resolve();
  const finish = () => {
    finished = true;
    port.close();
  };
  port.onmessage = (event: MessageEvent) => {
    const data = event.data as { pull?: boolean; cancel?: boolean };
    if (data.cancel) {
      void chunks?.return?.();
      finish();
      return;
    }
    if (!data.pull) return;
    // One pull at a time, in order.
    turn = turn.then(async () => {
      if (finished || !chunks) return;
      try {
        const next = await chunks.next();
        if (next.done) {
          port.postMessage({ end: true }, []);
          finish();
          return;
        }
        const buffer = next.value.slice().buffer;
        port.postMessage({ chunk: buffer }, [buffer]);
      } catch (error) {
        port.postMessage({ error: describe(error) }, []);
        finish();
      }
    });
  };
  replying.then(
    (answer) => {
      chunks = answer.chunks;
      port.postMessage({ head: { status: answer.status, headers: answer.headers } }, []);
    },
    (error: unknown) => {
      port.postMessage({ error: describe(error) }, []);
      finish();
    },
  );
};
