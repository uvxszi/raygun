import type { RemoteHandler } from '../core';
import { remoteState } from '../remoteState';
import { ByteStream, readFrame, writeFrame } from '../sync/channel';
import type { Connection, PeerRecord } from '../sync/engine';
import { Handshake, generateKeyPair } from '../sync/noise';
import type { CipherState, KeyPair } from '../sync/noise';
import { toHex } from '../sync/protocol';
import { RemoteBackend } from './backend';
import { FRAME_PLAIN, KIND_BODY, KIND_LAST } from './channel';

const BASE = 'https://example.org/raygun/';
const PROXY = 'http://127.0.0.1:41234';
const EMPTY = new Uint8Array(0);

type Message = Record<string, unknown>;

/** A linked app's end of the back-end channel (raygun-app web_remote.rs), answering as `answer` says. */
class FakeApp {
  private sending!: CipherState;
  private receiving!: CipherState;
  readonly received: Message[] = [];

  constructor(
    private readonly stream: ByteStream,
    private readonly identity: KeyPair,
    private readonly answer: (message: Message, app: FakeApp) => void,
  ) {}

  async run() {
    const handshake = new Handshake(false, this.identity);
    handshake.read(await readFrame(this.stream));
    writeFrame(this.stream, handshake.write());
    handshake.read(await readFrame(this.stream));
    const transport = handshake.transport();
    this.sending = transport.send;
    this.receiving = transport.receive;
    this.send({ t: 'welcome', deviceId: 'laptop', name: 'Laptop', proxy: PROXY });
    for (;;) {
      const plain = this.receiving.decrypt(EMPTY, await readFrame(this.stream));
      const message = JSON.parse(new TextDecoder().decode(plain.subarray(1))) as Message;
      this.received.push(message);
      this.answer(message, this);
    }
  }

  send(message: Message) {
    const bytes = new TextEncoder().encode(JSON.stringify(message));
    expect(bytes.length).toBeLessThan(FRAME_PLAIN);
    const plain = new Uint8Array(bytes.length + 1);
    plain[0] = KIND_LAST;
    plain.set(bytes, 1);
    writeFrame(this.stream, this.sending.encrypt(EMPTY, plain));
  }

  body(id: number, bytes: Uint8Array) {
    const plain = new Uint8Array(bytes.length + 5);
    plain[0] = KIND_BODY;
    new DataView(plain.buffer).setUint32(1, id);
    plain.set(bytes, 5);
    writeFrame(this.stream, this.sending.encrypt(EMPTY, plain));
  }
}

const appIdentity = generateKeyPair();
const pageIdentity = generateKeyPair();

const peer: PeerRecord = {
  deviceId: 'laptop',
  name: 'Laptop',
  publicKey: toHex(appIdentity.publicKey),
  invite: {
    name: 'Laptop',
    deviceId: 'laptop',
    publicKey: toHex(appIdentity.publicKey),
    code: '',
    ufrag: 'ufrag',
    pwd: 'password-password-pass',
    fingerprint: 'ab'.repeat(32),
    port: 8840,
    hosts: ['192.168.1.20'],
  },
  pairedAt: 1,
  lastSyncAt: 2,
  sentSeq: 0,
  kinds: [],
};

/** An app that answers search commands, refuses its own settings, and serves one cover. */
const answers = (message: Message, app: FakeApp) => {
  const id = message.id as number;
  if (message.t === 'call') {
    if (message.cmd === 'settings_reset') app.send({ t: 'result', id, denied: 'only there' });
    else if (message.cmd === 'broken') app.send({ t: 'result', id, err: 'That source failed' });
    else {
      app.send({
        t: 'result',
        id,
        ok: { cmd: message.cmd, args: message.args, cover: `${PROXY}/cover/abc` },
      });
    }
  } else if (message.t === 'fetch') {
    app.send({ t: 'head', id, status: 200, headers: { 'content-type': 'image/png' } });
    app.body(id, Uint8Array.of(1, 2, 3));
    app.send({ t: 'end', id });
  }
};

const setup = (answer = answers, identity = appIdentity) => {
  const apps: FakeApp[] = [];
  const closers: Array<() => void> = [];
  const connect = async (): Promise<Connection> => {
    const near: ByteStream = new ByteStream((bytes) => queueMicrotask(() => far.push(bytes)));
    const far: ByteStream = new ByteStream((bytes) => queueMicrotask(() => near.push(bytes)));
    const app = new FakeApp(far, identity, answer);
    apps.push(app);
    void app.run().catch(() => undefined);
    const close = () => {
      near.close();
      far.close();
    };
    closers.push(close);
    return { stream: near, close };
  };
  const emitted: Array<[string, unknown]> = [];
  const local = vi.fn(async (...call: [string, unknown?]) => `local ${call[0]}`);
  const backend = new RemoteBackend({
    core: {
      sync: { peers: () => [peer], identity: pageIdentity } as never,
      invoke: (command, args) => local(command, args as never),
    },
    emit: (event, payload) => emitted.push([event, payload]),
    base: BASE,
    connect,
    storage: null,
  });
  const invoke = backend.wrap(async (command, args) => {
    const remote = handlerOf()?.(command, args, () => local(command, args as never));
    return remote ?? local(command, args as never);
  });
  return { backend, apps, closers, emitted, local, invoke };
};

/** The handler the backend set on the web core. */
const core = vi.hoisted(() => ({ handler: null as RemoteHandler | null }));
const handlerOf = () => core.handler;
vi.mock('../core', () => ({
  setRemoteHandler: (handler: RemoteHandler | null) => {
    core.handler = handler;
  },
}));

let stops: Array<() => void> = [];
afterEach(() => {
  for (const stop of stops) stop();
  stops = [];
});

const started = async (answer = answers) => {
  const test = setup(answer);
  test.backend.start();
  stops.push(() => test.backend.stop());
  await vi.waitFor(() => expect(remoteState().connected).toBe(true));
  return test;
};

describe('a linked app as the back end', () => {
  it('runs commands on the app, with links made the page’s both ways', async () => {
    const { invoke, apps } = await started();
    expect(remoteState()).toMatchObject({ device: 'Laptop', enabled: true, available: true });
    const answer = await invoke('catalog_search', {
      query: 'moon',
      cover: `${BASE}__remote/cover/old`,
    });
    expect(answer).toEqual({
      cmd: 'catalog_search',
      args: { query: 'moon', cover: `${BASE}__remote/cover/old` },
      cover: `${BASE}__remote/cover/abc`,
    });
    // The app got its own form.
    expect(apps[0]?.received.find((message) => message.cmd === 'catalog_search')?.args).toEqual({
      query: 'moon',
      cover: `${PROXY}/cover/old`,
    });
  });

  it('keeps the page’s own commands, and those the app refuses, in the page', async () => {
    const { invoke, local } = await started();
    expect(await invoke('favorites_list', {})).toBe('local favorites_list');
    expect(await invoke('settings_reset', {})).toBe('local settings_reset');
    expect(local).toHaveBeenCalledWith('settings_reset', {});
    await expect(invoke('broken', {})).rejects.toBe('That source failed');
  });

  it('passes the app’s events on, and fetches its pictures', async () => {
    const { backend, apps, emitted } = await started();
    apps[0]?.send({
      t: 'event',
      event: 'search-progress-event',
      payload: { cover: `${PROXY}/cover/z` },
    });
    await vi.waitFor(() =>
      expect(emitted).toContainEqual([
        'search-progress-event',
        { cover: `${BASE}__remote/cover/z` },
      ]),
    );
    const piece = await backend.fetch('cover/abc', null);
    expect(piece).toEqual({
      status: 200,
      headers: { 'content-type': 'image/png' },
      body: Uint8Array.of(1, 2, 3),
    });
  });

  it('answers in the page when the app goes away, and connects again', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { invoke, closers } = await started();
      closers[0]?.();
      await vi.waitFor(() => expect(remoteState().connected).toBe(false));
      expect(await invoke('catalog_search', {})).toBe('local catalog_search');
      await vi.advanceTimersByTimeAsync(3_000);
      await vi.waitFor(() => expect(remoteState().connected).toBe(true));
    } finally {
      vi.useRealTimers();
    }
  });

  it('stays in the page while switched off', async () => {
    const test = setup();
    test.backend.start();
    stops.push(() => test.backend.stop());
    await vi.waitFor(() => expect(remoteState().connected).toBe(true));
    const { remoteControls } = await import('../remoteState');
    remoteControls()?.setEnabled(false);
    expect(remoteState()).toMatchObject({ enabled: false, connected: false });
    expect(await test.invoke('catalog_search', {})).toBe('local catalog_search');
  });

  it('never uses an app whose key changed', async () => {
    const test = setup(answers, generateKeyPair());
    test.backend.start();
    stops.push(() => test.backend.stop());
    await vi.waitFor(() => expect(remoteState().error).toMatch(/isn't Laptop any more/));
    expect(remoteState().connected).toBe(false);
    expect(await test.invoke('catalog_search', {})).toBe('local catalog_search');
  });
});
