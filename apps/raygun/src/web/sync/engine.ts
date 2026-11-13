// Device sync in the browser (raygun-sync lib.rs): linking with a Raygun app from its link code,
// then sessions that send what changed here and take what changed there. A session is
// half-duplex: the side that connected sends its changes and waits for the acknowledgement, then
// the other side does the same. A browser can't listen, so it always connects.
import type { SyncKind, SyncPeerInfo, SyncSessionInfo, SyncStatus } from '@raygun/bindings';

import { SyncedRows } from '../core/context';
import type { CoreContext } from '../core/context';
import { stringifyJson } from '../core/json';
import { Channel, SyncError } from './channel';
import type { ByteStream } from './channel';
import { applyRows, changesFor } from './merge';
import type { Applied } from './merge';
import { fromHex, keyFingerprint, toHex } from './protocol';
import type { Message, WebInvite, WireRow } from './protocol';
import { generateKeyPair, keyPairOf } from './noise';
import type { KeyPair } from './noise';

const ALL_KINDS: SyncKind[] = ['settings', 'library', 'history'];
/** Rows per message are cut by size, as Rust's batches are. */
const BATCH_BYTES = 48 * 1024;
const LOG_LENGTH = 20;

const IDENTITY = 'sync.identity';
const KINDS = 'sync.kinds';
const NAME = 'sync.name';
const SESSION_SEQ = 'sync.sessionSeq';

/** A device this browser syncs with, and how to reach it. */
export type PeerRecord = {
  deviceId: string;
  name: string;
  publicKey: string;
  invite: WebInvite;
  pairedAt: number;
  lastSyncAt: number | null;
  sentSeq: number;
  /** The kinds last sent to it; a new one means sending everything again. */
  kinds: SyncKind[];
};

type SessionRecord = {
  id: number;
  peerId: string;
  peerName: string;
  startedAt: number;
  finishedAt: number | null;
  sent: number;
  received: number;
  changed: number;
  error: string | null;
};

/** A connection to another device: a byte stream both ways. */
export type Connection = { stream: ByteStream; close: () => void };
export type Connect = (invite: WebInvite) => Promise<Connection>;

export type Summary = { sent: number; received: number; changed: number };

const defaultName = () => {
  if (typeof navigator === 'undefined') return 'Raygun on the web';
  const agent = navigator.userAgent;
  const browser = /Edg\//.test(agent)
    ? 'Edge'
    : /Firefox\//.test(agent)
      ? 'Firefox'
      : /Chrome\//.test(agent)
        ? 'Chrome'
        : /Safari\//.test(agent)
          ? 'Safari'
          : 'a browser';
  return `Raygun in ${browser}`;
};

const describe = (error: unknown) =>
  error instanceof Error ? error.message : typeof error === 'string' ? error : String(error);

export class SyncEngine {
  private readonly rows: SyncedRows;
  readonly identity: KeyPair;
  /** A code another device may link with (only tests open one: a browser can't be reached). */
  private openCode: string | null = null;
  private running: Promise<unknown> = Promise.resolve();

  constructor(
    private readonly context: CoreContext,
    private readonly connect: Connect,
    /** Called after a session that changed something here. */
    private readonly onChanged: (summary: Summary) => void = () => undefined,
  ) {
    this.rows = new SyncedRows(context);
    const stored = context.meta<string>(IDENTITY);
    const key = stored ? fromHex(stored) : null;
    if (key && key.length === 32) {
      this.identity = keyPairOf(key);
    } else {
      this.identity = generateKeyPair();
      context.setMeta(IDENTITY, toHex(this.identity.privateKey));
    }
  }

  get deviceId() {
    return this.context.deviceId;
  }

  name(): string {
    return this.context.meta<string>(NAME) ?? defaultName();
  }

  setName(name: string) {
    const trimmed = [...name.trim()].slice(0, 40).join('');
    if (!trimmed) throw new SyncError('A device needs a name');
    this.context.setMeta(NAME, trimmed);
  }

  kinds(): SyncKind[] {
    return this.context.meta<SyncKind[]>(KINDS) ?? ALL_KINDS;
  }

  setKinds(kinds: SyncKind[]) {
    this.context.setMeta(KINDS, kinds);
  }

  peers(): PeerRecord[] {
    return this.context.store.all<PeerRecord>('sync_peer').sort((a, b) => a.pairedAt - b.pairedAt);
  }

  peer(deviceId: string): PeerRecord | undefined {
    return this.context.store.get<PeerRecord>('sync_peer', deviceId);
  }

  removePeer(deviceId: string) {
    this.context.store.remove('sync_peer', deviceId);
  }

  private savePeer(peer: PeerRecord) {
    this.context.store.put('sync_peer', peer.deviceId, peer);
  }

  status(error: string | null = null): SyncStatus {
    const peers: SyncPeerInfo[] = this.peers().map((peer) => ({
      deviceId: peer.deviceId,
      name: peer.name,
      fingerprint: keyFingerprint(fromHex(peer.publicKey) ?? new Uint8Array(8)),
      address: peer.invite.hosts[0] ? `${peer.invite.hosts[0]}:${peer.invite.port}` : null,
      pairedAt: peer.pairedAt,
      lastSyncAt: peer.lastSyncAt,
    }));
    const log: SyncSessionInfo[] = this.context.store
      .all<SessionRecord>('sync_session')
      .sort((a, b) => b.id - a.id)
      .slice(0, LOG_LENGTH)
      .map((session) => ({
        id: session.id,
        peerName: session.peerName,
        startedAt: session.startedAt,
        finished: session.finishedAt !== null,
        sent: session.sent,
        received: session.received,
        changed: session.changed,
        error: session.error,
        undone: false,
      }));
    return {
      enabled: true,
      deviceName: this.name(),
      fingerprint: keyFingerprint(this.identity.publicKey),
      pairingText: null,
      pairingQrSvg: null,
      kinds: this.kinds(),
      peers,
      log,
      error,
    };
  }

  /** One session at a time: sessions queue behind each other. */
  private serial<T>(run: () => Promise<T>): Promise<T> {
    const next = this.running.then(run, run);
    this.running = next.catch(() => undefined);
    return next;
  }

  private hello(): Message {
    return {
      type: 'hello',
      deviceId: this.deviceId,
      name: this.name(),
      kinds: this.kinds(),
      port: null,
    };
  }

  private shared(theirs: SyncKind[]) {
    return this.kinds().filter((kind) => theirs.includes(kind));
  }

  private async open(invite: WebInvite, expectedKey: string) {
    const connection = await this.connect(invite);
    try {
      const { channel, remote } = await Channel.initiate(connection.stream, this.identity);
      // Checked before anything else is sent: a device in the middle learns nothing.
      if (toHex(remote) !== expectedKey) {
        throw new SyncError(
          `The device at ${invite.hosts[0] ?? 'that address'} isn't ${invite.name || 'the one linked'} any more. Link it again.`,
        );
      }
      return { channel, connection };
    } catch (error) {
      connection.close();
      throw error;
    }
  }

  /** Links with the app that showed `invite`, then runs a first sync with it. */
  link(invite: WebInvite): Promise<PeerRecord> {
    return this.serial(async () => {
      if (invite.deviceId === this.deviceId) throw new SyncError("That's this browser's own code.");
      const { channel, connection } = await this.open(invite, invite.publicKey);
      try {
        channel.send({
          type: 'pair',
          code: invite.code,
          deviceId: this.deviceId,
          name: this.name(),
          port: null,
        });
        const answer = await channel.recv();
        if (answer.type === 'refused') throw new SyncError(answer.reason);
        if (answer.type !== 'paired' || answer.deviceId !== invite.deviceId) {
          throw new SyncError(`unexpected message: ${answer.type}`);
        }
        const peer: PeerRecord = {
          deviceId: answer.deviceId,
          name: answer.name || invite.name,
          publicKey: invite.publicKey,
          invite: { ...invite, code: '' },
          pairedAt: this.context.now(),
          lastSyncAt: null,
          sentSeq: 0,
          kinds: [],
        };
        this.savePeer(peer);
        return peer;
      } finally {
        connection.close();
      }
    });
  }

  /** Syncs with one linked device at its last known address. */
  syncWith(deviceId: string): Promise<Summary> {
    return this.serial(async () => {
      const peer = this.peer(deviceId);
      if (!peer) throw new SyncError("That device isn't linked.");
      const session = this.startSession(peer);
      try {
        const summary = await this.initiateSession(peer);
        this.finishSession(session, peer, summary, null);
        return summary;
      } catch (error) {
        this.finishSession(session, peer, { sent: 0, received: 0, changed: 0 }, describe(error));
        throw error;
      }
    });
  }

  /** Syncs with every linked device, one after another; failures stay in the log. */
  async syncAll(): Promise<void> {
    for (const peer of this.peers()) {
      try {
        await this.syncWith(peer.deviceId);
      } catch {
        // Kept in the session log.
      }
    }
  }

  private async initiateSession(peer: PeerRecord): Promise<Summary> {
    const { channel, connection } = await this.open(peer.invite, peer.publicKey);
    try {
      channel.send(this.hello());
      const answer = await channel.recv();
      if (answer.type === 'refused') throw new SyncError(answer.reason);
      if (answer.type !== 'hello' || answer.deviceId !== peer.deviceId) {
        throw new SyncError(`unexpected message: ${answer.type}`);
      }
      const kinds = this.shared(answer.kinds);
      const sent = await this.sendChanges(channel, peer, kinds);
      const { received, changed } = await this.receiveChanges(channel, kinds, peer.deviceId);
      return { sent, received, changed };
    } finally {
      connection.close();
    }
  }

  /** Answers a device that connected here (a browser can't be reached, but tests connect two). */
  async respond(stream: ByteStream): Promise<Summary | null> {
    const { channel, remote } = await Channel.respond(stream, this.identity);
    const first = await channel.recv();
    if (first.type === 'pair') {
      this.answerPair(channel, first, toHex(remote));
      return null;
    }
    if (first.type !== 'hello') throw new SyncError(`unexpected message: ${first.type}`);
    const peer = this.peer(first.deviceId);
    if (!peer || peer.publicKey !== toHex(remote)) {
      const reason = "This device isn't paired with the other one.";
      channel.send({ type: 'refused', reason });
      throw new SyncError(reason);
    }
    channel.send(this.hello());
    const kinds = this.shared(first.kinds);
    const session = this.startSession(peer);
    try {
      const { received, changed } = await this.receiveChanges(channel, kinds, peer.deviceId);
      const sent = await this.sendChanges(channel, peer, kinds);
      const summary = { sent, received, changed };
      this.finishSession(session, peer, summary, null);
      return summary;
    } catch (error) {
      this.finishSession(session, peer, { sent: 0, received: 0, changed: 0 }, describe(error));
      throw error;
    }
  }

  /** Lets another device link here while a code is open (tests: a browser can't be reached). */
  acceptCode(code: string) {
    this.openCode = code;
  }

  private answerPair(
    channel: Channel,
    pair: Extract<Message, { type: 'pair' }>,
    publicKey: string,
  ) {
    if (this.openCode === null || pair.code.trim().toLowerCase() !== this.openCode) {
      const reason = 'That pairing code is wrong.';
      channel.send({ type: 'refused', reason });
      throw new SyncError(reason);
    }
    this.openCode = null;
    this.savePeer({
      deviceId: pair.deviceId,
      name: pair.name,
      publicKey,
      invite: {
        name: pair.name,
        deviceId: pair.deviceId,
        publicKey,
        code: '',
        ufrag: '',
        pwd: '',
        fingerprint: '',
        port: 0,
        hosts: [],
      },
      pairedAt: this.context.now(),
      lastSyncAt: null,
      sentSeq: 0,
      kinds: [],
    });
    channel.send({ type: 'paired', deviceId: this.deviceId, name: this.name() });
  }

  /** Sends every change the peer hasn't acknowledged, then waits for its acknowledgement. */
  private async sendChanges(channel: Channel, peer: PeerRecord, kinds: SyncKind[]) {
    // When the shared kinds grew since the last sync, everything goes again.
    const stored = this.peer(peer.deviceId) ?? peer;
    const from = kinds.every((kind) => stored.kinds.includes(kind)) ? stored.sentSeq : 0;
    const { rows, last } = changesFor(this.rows, from, kinds, peer.deviceId);
    let batch: WireRow[] = [];
    let bytes = 0;
    for (const row of rows) {
      batch.push(row);
      bytes += stringifyJson(row).length;
      if (bytes >= BATCH_BYTES) {
        channel.send({ type: 'rows', rows: batch });
        batch = [];
        bytes = 0;
      }
    }
    if (batch.length > 0) channel.send({ type: 'rows', rows: batch });
    channel.send({ type: 'done', upto: last });
    const answer = await channel.recv();
    if (answer.type !== 'ack') throw new SyncError(`unexpected message: ${answer.type}`);
    const current = this.peer(peer.deviceId) ?? peer;
    this.savePeer({ ...current, sentSeq: Math.max(current.sentSeq, answer.upto), kinds });
    return rows.length;
  }

  /** Takes the peer's changes until it says it's done, then acknowledges them. */
  private async receiveChanges(channel: Channel, kinds: SyncKind[], fromPeer: string) {
    const total: Applied = { received: 0, changed: 0 };
    for (;;) {
      const message = await channel.recv();
      if (message.type === 'rows') {
        const applied = applyRows(this.context, this.rows, message.rows, kinds, fromPeer);
        total.received += applied.received;
        total.changed += applied.changed;
      } else if (message.type === 'done') {
        channel.send({ type: 'ack', upto: message.upto });
        return total;
      } else {
        throw new SyncError(`unexpected message: ${message.type}`);
      }
    }
  }

  private startSession(peer: PeerRecord): SessionRecord {
    const id = (this.context.meta<number>(SESSION_SEQ) ?? 0) + 1;
    this.context.setMeta(SESSION_SEQ, id);
    const session: SessionRecord = {
      id,
      peerId: peer.deviceId,
      peerName: peer.name,
      startedAt: this.context.now(),
      finishedAt: null,
      sent: 0,
      received: 0,
      changed: 0,
      error: null,
    };
    this.context.store.put('sync_session', String(id), session);
    // Only the latest sessions are kept.
    for (const old of this.context.store.all<SessionRecord>('sync_session')) {
      if (old.id <= id - LOG_LENGTH) this.context.store.remove('sync_session', String(old.id));
    }
    return session;
  }

  private finishSession(
    session: SessionRecord,
    peer: PeerRecord,
    summary: Summary,
    error: string | null,
  ) {
    const now = this.context.now();
    this.context.store.put('sync_session', String(session.id), {
      ...session,
      ...summary,
      finishedAt: now,
      error,
    });
    if (error === null) {
      const current = this.peer(peer.deviceId) ?? peer;
      this.savePeer({ ...current, lastSyncAt: now });
      if (summary.changed > 0) this.onChanged(summary);
    }
  }
}
