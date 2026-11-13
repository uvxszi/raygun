// A linked app used as the page's back end (raygun-app web_remote.rs): commands by their Rust
// names, pictures and media in pieces, and the events the page's screens follow.
import { SyncError } from '../sync/channel';
import type { RemoteChannel } from './channel';

/** The app's first message: who it is, and where its proxy listens (for links going back). */
export type Welcome = { deviceId: string; name: string; proxy: string | null };

export type CallResult =
  | { kind: 'ok'; value: unknown }
  | { kind: 'err'; error: unknown }
  /** The app runs this command only for itself; the page answers it. */
  | { kind: 'denied'; reason: string };

/** One fetched piece of a proxy path. */
export type Piece = { status: number; headers: Record<string, string>; body: Uint8Array };

/** The page pings this often; the app drops a page it hasn't heard from in 75 s. */
const PING_EVERY_MS = 20_000;

type Pending = {
  resolve: (value: never) => void;
  reject: (error: Error) => void;
};

type PendingFetch = Pending & {
  head: { status: number; headers: Record<string, string> } | null;
  chunks: Uint8Array[];
};

const joined = (chunks: Uint8Array[]) => {
  const out = new Uint8Array(chunks.reduce((sum, chunk) => sum + chunk.length, 0));
  let at = 0;
  for (const chunk of chunks) {
    out.set(chunk, at);
    at += chunk.length;
  }
  return out;
};

export class RemoteClient {
  private nextId = 1;
  private readonly calls = new Map<number, Pending>();
  private readonly fetches = new Map<number, PendingFetch>();
  private closedWith: Error | null = null;
  private pinger: ReturnType<typeof setInterval> | null = null;
  private welcomed: ((welcome: Welcome) => void) | null = null;
  private refused: ((error: Error) => void) | null = null;
  /** The app's welcome; rejects when it refuses the page. */
  readonly welcome: Promise<Welcome>;
  /** An event the app sent. */
  onEvent: (event: string, payload: unknown) => void = () => undefined;
  /** The connection ended (after the welcome). */
  onClose: (error: Error) => void = () => undefined;

  constructor(
    private readonly channel: RemoteChannel,
    private readonly disconnect: () => void,
  ) {
    this.welcome = new Promise<Welcome>((resolve, reject) => {
      this.welcomed = resolve;
      this.refused = reject;
    });
    // Unhandled until someone waits for it.
    this.welcome.catch(() => undefined);
    void this.read();
    this.pinger = setInterval(() => this.post({ t: 'ping' }), PING_EVERY_MS);
  }

  get closed() {
    return this.closedWith !== null;
  }

  private post(message: Record<string, unknown>) {
    if (this.closedWith) return;
    try {
      this.channel.send(message);
    } catch (error) {
      this.finish(error instanceof Error ? error : new SyncError(String(error)));
    }
  }

  /** Runs one command on the app. */
  call(command: string, args: Record<string, unknown>): Promise<CallResult> {
    if (this.closedWith) return Promise.reject(this.closedWith);
    const id = this.nextId++;
    return new Promise<CallResult>((resolve, reject) => {
      this.calls.set(id, { resolve: resolve as (value: never) => void, reject });
      this.post({ t: 'call', id, cmd: command, args });
    });
  }

  /** Asks the app what a queued song plays from (its resolver; its player isn't touched). */
  resolve(item: unknown): Promise<CallResult> {
    if (this.closedWith) return Promise.reject(this.closedWith);
    const id = this.nextId++;
    return new Promise<CallResult>((resolve, reject) => {
      this.calls.set(id, { resolve: resolve as (value: never) => void, reject });
      this.post({ t: 'resolve', id, item });
    });
  }

  /** Fetches a proxy path (`cover/…`, `media/…`), or a range of it. */
  fetch(path: string, range: string | null): Promise<Piece> {
    if (this.closedWith) return Promise.reject(this.closedWith);
    const id = this.nextId++;
    return new Promise<Piece>((resolve, reject) => {
      this.fetches.set(id, {
        resolve: resolve as (value: never) => void,
        reject,
        head: null,
        chunks: [],
      });
      this.post({ t: 'fetch', id, path, range });
    });
  }

  close() {
    this.finish(new SyncError('closed'));
  }

  private finish(error: Error) {
    if (this.closedWith) return;
    this.closedWith = error;
    if (this.pinger) clearInterval(this.pinger);
    this.disconnect();
    this.refused?.(error);
    for (const pending of [...this.calls.values(), ...this.fetches.values()]) {
      pending.reject(error);
    }
    this.calls.clear();
    this.fetches.clear();
    this.onClose(error);
  }

  private async read() {
    try {
      for (;;) {
        const incoming = await this.channel.next();
        if (incoming.kind === 'body') {
          this.fetches.get(incoming.id)?.chunks.push(incoming.bytes);
          continue;
        }
        this.handle(incoming.value);
      }
    } catch (error) {
      this.finish(error instanceof Error ? error : new SyncError(String(error)));
    }
  }

  private handle(message: Record<string, unknown>) {
    const id = Number(message.id);
    switch (message.t) {
      case 'welcome':
        this.welcomed?.({
          deviceId: String(message.deviceId ?? ''),
          name: String(message.name ?? ''),
          proxy: typeof message.proxy === 'string' ? message.proxy : null,
        });
        this.welcomed = null;
        this.refused = null;
        return;
      case 'refused':
        this.finish(new SyncError(String(message.reason ?? 'refused')));
        return;
      case 'result': {
        const pending = this.calls.get(id);
        if (!pending) return;
        this.calls.delete(id);
        const result: CallResult =
          'denied' in message
            ? { kind: 'denied', reason: String(message.denied) }
            : 'err' in message
              ? { kind: 'err', error: message.err }
              : { kind: 'ok', value: message.ok ?? null };
        (pending.resolve as (value: CallResult) => void)(result);
        return;
      }
      case 'head': {
        const pending = this.fetches.get(id);
        if (pending) {
          pending.head = {
            status: Number(message.status),
            headers: (message.headers ?? {}) as Record<string, string>,
          };
        }
        return;
      }
      case 'end': {
        const pending = this.fetches.get(id);
        if (!pending) return;
        this.fetches.delete(id);
        if (typeof message.error === 'string' || !pending.head) {
          pending.reject(new SyncError(String(message.error ?? 'no answer')));
          return;
        }
        (pending.resolve as (value: Piece) => void)({
          ...pending.head,
          body: joined(pending.chunks),
        });
        return;
      }
      case 'event':
        this.onEvent(String(message.event), message.payload);
        return;
      default:
    }
  }
}
