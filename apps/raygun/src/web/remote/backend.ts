// Raygun on the web using a linked app as its back end (docs/WEB.md, "The app as a back end").
// While a linked app is reachable and the switch is on, commands the page doesn't keep for itself
// go to it (routing.ts), its events reach the page's screens, and its pictures and media come
// through the service worker (media.ts). When it goes away the page answers again by itself, and
// tries the app again later.
import type { QueueItem } from '@raygun/bindings';

import { setRemoteFinder, setRemoteHandler } from '../core';
import type { WebCore } from '../core';
import { SyncError } from '../sync/channel';
import type { Connection, PeerRecord } from '../sync/engine';
import { toHex } from '../sync/protocol';
import type { WebInvite } from '../sync/protocol';
import { connectWebRtc } from '../sync/webrtc';
import { remoteState, setRemoteControls, setRemoteState } from '../remoteState';
import type { Found } from '../core/playback';
import { RemoteChannel } from './channel';
import { RemoteClient } from './client';
import type { Piece, Welcome } from './client';
import { answerPort, reply } from './media';
import type { PortLike } from './media';
import { goesToApp } from './routing';
import { toAppLinks, toPageLinks } from './rewrite';

const PREFS = 'raygun.web.backend';
/** Waits between tries while the app can't be reached. */
const RETRY_MS = [2_000, 5_000, 15_000, 30_000, 60_000];
/** How long the app may take to say hello. */
const WELCOME_MS = 15_000;
/** A quiet channel is checked by pings; reads wait this long. */
const READ_TIMEOUT_MS = 60_000;

type Prefs = { enabled: boolean; deviceId: string | null; proxy: string | null };

export type BackendOptions = {
  core: Pick<WebCore, 'sync' | 'invoke'>;
  /** Delivers an event to the page's listeners (ipc.ts). */
  emit: (event: string, payload: unknown) => void;
  /** The page's base address (`https://…/raygun/`). */
  base: string;
  /** Opens the back-end channel's connection (WebRTC; tests connect directly). */
  connect?: (invite: WebInvite) => Promise<Connection>;
  storage?: Pick<Storage, 'getItem' | 'setItem'> | null;
};

const describe = (error: unknown) =>
  error instanceof Error ? error.message : typeof error === 'string' ? error : String(error);

/**
 * An event's links made the page's. The video player's messages carry their request as JSON text
 * (`VideoPlayerMessage.json`), whose links are made the page's inside it.
 */
export const pageLinksInEvent = (event: string, payload: unknown, base: string): unknown => {
  const message = payload as { json?: unknown } | null;
  if (event === 'video-player-event' && typeof message?.json === 'string') {
    try {
      return { ...message, json: JSON.stringify(toPageLinks(JSON.parse(message.json), base)) };
    } catch {
      return payload;
    }
  }
  return toPageLinks(payload, base);
};

const connectRemote = (invite: WebInvite) =>
  connectWebRtc(invite, { id: 1, label: 'raygun-remote', timeoutMs: READ_TIMEOUT_MS });

export class RemoteBackend {
  private client: RemoteClient | null = null;
  private prefs: Prefs;
  private retry = 0;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private trying: Promise<void> | null = null;
  /** Commands the app said it runs only for itself. */
  private readonly denied = new Set<string>();
  private readonly connect: (invite: WebInvite) => Promise<Connection>;
  private stopped = false;

  constructor(private readonly options: BackendOptions) {
    this.connect = options.connect ?? connectRemote;
    this.prefs = { enabled: true, deviceId: null, proxy: null, ...this.readPrefs() };
  }

  private readPrefs(): Partial<Prefs> {
    try {
      const text = this.options.storage?.getItem(PREFS);
      return text ? (JSON.parse(text) as Partial<Prefs>) : {};
    } catch {
      return {};
    }
  }

  private savePrefs(next: Partial<Prefs>) {
    this.prefs = { ...this.prefs, ...next };
    try {
      this.options.storage?.setItem(PREFS, JSON.stringify(this.prefs));
    } catch {
      // A browser that keeps nothing still works for this visit.
    }
  }

  /** The proxy of the app in use (or last used), for links going back to it. */
  get proxy() {
    return this.prefs.proxy;
  }

  get connected() {
    return this.client !== null && !this.client.closed;
  }

  /** The linked app to use: the one chosen, else the one synced with last. */
  private peer(): PeerRecord | null {
    const peers = this.options.core.sync.peers().filter((peer) => peer.invite.hosts.length > 0);
    return (
      peers.find((peer) => peer.deviceId === this.prefs.deviceId) ??
      [...peers].sort((a, b) => (b.lastSyncAt ?? 0) - (a.lastSyncAt ?? 0))[0] ??
      null
    );
  }

  private publish(extra: Partial<Parameters<typeof setRemoteState>[0]> = {}) {
    const peer = this.peer();
    setRemoteState({
      available: peer !== null,
      enabled: this.prefs.enabled,
      deviceId: peer?.deviceId ?? null,
      device: peer?.name ?? null,
      connected: this.connected,
      ...extra,
    });
  }

  /** Starts using a linked app when there is one; the page calls it once. */
  start() {
    setRemoteHandler((command, args, local) => this.handle(command, args, local));
    setRemoteFinder((item) => this.streamFor(item));
    setRemoteControls({
      setEnabled: (enabled) => {
        this.savePrefs({ enabled });
        if (enabled) this.tryNow();
        else this.disconnect();
        this.publish();
      },
      choose: (deviceId) => {
        this.savePrefs({ deviceId, enabled: true });
        this.disconnect();
        this.tryNow();
      },
    });
    if (typeof document !== 'undefined') {
      document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'visible' && !this.connected) this.tryNow();
      });
    }
    if (typeof window !== 'undefined') window.addEventListener('online', () => this.tryNow());
    this.publish();
    this.tryNow();
  }

  /** Stops (tests). */
  stop() {
    this.stopped = true;
    if (this.timer) clearTimeout(this.timer);
    this.disconnect();
    setRemoteHandler(null);
    setRemoteFinder(null);
    setRemoteControls(null);
  }

  private disconnect() {
    const client = this.client;
    this.client = null;
    client?.close();
  }

  /** Tries to connect now, unless a try is under way. */
  tryNow(): Promise<void> {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    this.trying ??= this.attempt().finally(() => {
      this.trying = null;
    });
    return this.trying;
  }

  private later() {
    if (this.stopped || this.timer) return;
    const wait = RETRY_MS[Math.min(this.retry, RETRY_MS.length - 1)] ?? 60_000;
    this.retry += 1;
    this.timer = setTimeout(() => {
      this.timer = null;
      void this.tryNow();
    }, wait);
  }

  private async attempt() {
    if (this.stopped || this.connected || !this.prefs.enabled) return;
    const peer = this.peer();
    if (!peer) {
      this.publish();
      // Linked later: looked for again from time to time.
      this.later();
      return;
    }
    this.publish({ connecting: true, error: null });
    let connection: Connection | null = null;
    try {
      connection = await this.connect(peer.invite);
      const opened = connection;
      const { channel, remote } = await RemoteChannel.open(
        opened.stream,
        this.options.core.sync.identity,
      );
      if (toHex(remote) !== peer.publicKey) {
        throw new SyncError(
          `The device at ${peer.invite.hosts[0] ?? 'that address'} isn't ${peer.name} any more.`,
        );
      }
      const client = new RemoteClient(channel, () => opened.close());
      const welcome = await Promise.race<Welcome>([
        client.welcome,
        new Promise<Welcome>((_, reject) =>
          setTimeout(() => reject(new SyncError(`${peer.name} didn't answer`)), WELCOME_MS),
        ),
      ]);
      if (this.stopped || !this.prefs.enabled) {
        client.close();
        return;
      }
      this.client = client;
      this.retry = 0;
      this.savePrefs({ proxy: welcome.proxy });
      client.onEvent = (event, payload) =>
        this.options.emit(event, pageLinksInEvent(event, payload, this.options.base));
      client.onClose = () => {
        if (this.client !== client) return;
        this.client = null;
        this.publish({ connecting: false });
        void this.refresh();
        this.later();
      };
      this.publish({ connecting: false, error: null, device: welcome.name || peer.name });
      void this.refresh();
    } catch (error) {
      connection?.close();
      this.publish({ connecting: false, error: describe(error) });
      this.later();
    }
  }

  /** The page's screens show the other side's sources now. */
  private async refresh() {
    try {
      const list = await this.options.core.invoke('extensions_list', {});
      this.options.emit('extensions-changed-event', toPageLinks(list, this.options.base));
    } catch {
      // Nothing to refresh with.
    }
  }

  /** Answers a command through the app, or leaves it to the page (null). */
  handle(
    command: string,
    args: Record<string, unknown>,
    local: () => Promise<unknown>,
  ): Promise<unknown> | null {
    const client = this.client;
    if (!client || client.closed || !goesToApp(command) || this.denied.has(command)) return null;
    return client.call(command, args).then(
      (result) => {
        if (result.kind === 'ok') return result.value;
        if (result.kind === 'err') throw result.error;
        this.denied.add(command);
        return local();
      },
      // The app went away mid-call: the page's own answer.
      () => local(),
    );
  }

  /**
   * What a queued song plays from, found by the app: a stream through its proxy (played from the
   * page's `__remote/` paths, HLS by hls.js over the same route) or a YouTube video. Null when no
   * app is in use or it found nothing, so the page's own sources look.
   */
  async streamFor(item: QueueItem): Promise<Found | null> {
    const client = this.client;
    if (!client || client.closed) return null;
    const source = item.source;
    // A plain address plays as it is; one needing headers goes through the app's proxy.
    if (source.kind === 'youTube') return null;
    if (source.kind === 'url' && Object.keys(source.headers ?? {}).length === 0) return null;
    try {
      const result = await client.resolve(toAppLinks(item, this.options.base, this.prefs.proxy));
      if (result.kind !== 'ok') return null;
      const found = result.value as {
        kind?: string;
        url?: string;
        hls?: boolean;
        videoId?: string;
      };
      if (found.kind === 'youTube' && found.videoId)
        return { kind: 'youTube', videoId: found.videoId };
      if (found.kind === 'url' && found.url) {
        return {
          kind: 'url',
          url: toPageLinks(found.url, this.options.base),
          hls: found.hls === true,
          label: remoteState().device,
        };
      }
    } catch {
      // The app went away: the page's own sources look.
    }
    return null;
  }

  /** Fetches a proxy path through the app in use. */
  fetch(path: string, range: string | null): Promise<Piece> {
    const client = this.client;
    if (!client || client.closed) return Promise.reject(new Error('No linked app is in use'));
    return client.fetch(path, range);
  }

  /** Answers a service worker's request for an app's picture or media. */
  answer(port: PortLike, path: string, range: string | null) {
    answerPort(
      port,
      reply((asked, piece) => this.fetch(asked, piece), path, range, this.options.base),
    );
  }

  /** The page's invoke with links made the page's on the way out and the app's on the way in. */
  wrap(
    invoke: (command: string, args: Record<string, unknown>) => Promise<unknown>,
  ): (command: string, args: Record<string, unknown>) => Promise<unknown> {
    return async (command, args) => {
      const answer = await invoke(command, toAppLinks(args, this.options.base, this.prefs.proxy));
      return toPageLinks(answer, this.options.base);
    };
  }

  /** Listens for the service worker's requests (the page calls it once). */
  listenToWorker(container: Pick<ServiceWorkerContainer, 'addEventListener'>) {
    container.addEventListener('message', (event: MessageEvent) => {
      const data = event.data as { type?: string; path?: string; range?: string | null };
      const port = event.ports[0];
      if (data?.type !== 'raygun-remote-fetch' || !port || typeof data.path !== 'string') return;
      this.answer(port, data.path, data.range ?? null);
    });
  }
}
