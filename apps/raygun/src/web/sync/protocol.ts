// Device sync's messages (raygun-sync protocol.rs) and what a link code tells a browser.
import type { SyncKind } from '@raygun/bindings';

export type WireRow = { table: string; row: Record<string, unknown> };

export type Message =
  | { type: 'pair'; code: string; deviceId: string; name: string; port: number | null }
  | { type: 'paired'; deviceId: string; name: string }
  | { type: 'hello'; deviceId: string; name: string; kinds: SyncKind[]; port: number | null }
  | { type: 'rows'; rows: WireRow[] }
  | { type: 'done'; upto: number }
  | { type: 'ack'; upto: number }
  | { type: 'refused'; reason: string };

/**
 * How a browser reaches a Raygun app (docs/WEB.md): the app's sync identity and an open pairing
 * code, and its WebRTC listener (ICE-lite on a UDP port, DTLS with a fixed certificate).
 */
export type WebInvite = {
  /** The app's name, as it shows it. */
  name: string;
  deviceId: string;
  /** Its Noise static key, hex. */
  publicKey: string;
  /** The one-time sync pairing code; empty when only reconnecting. */
  code: string;
  ufrag: string;
  pwd: string;
  /** SHA-256 of its DTLS certificate, hex. */
  fingerprint: string;
  port: number;
  hosts: string[];
};

const LINK_PREFIX = 'raygun:link?';

const HEX_KEY = /^[0-9a-f]{64}$/;

/**
 * Reads a link code (`raygun:link?…`) that carries an app's web link. Codes from apps that can't
 * link with a browser, and other text, give null.
 */
export const parseInvite = (text: string): WebInvite | null => {
  const trimmed = text.trim();
  if (!trimmed.toLowerCase().startsWith(LINK_PREFIX)) return null;
  const query = new URLSearchParams(trimmed.slice(LINK_PREFIX.length));
  const [ufrag = '', pwd = ''] = (query.get('ice') ?? '').split(':');
  const port = Number(query.get('udp'));
  const invite: WebInvite = {
    name: (query.get('name') ?? '').trim().slice(0, 40),
    deviceId: (query.get('sync') ?? '').trim(),
    publicKey: (query.get('key') ?? '').trim().toLowerCase(),
    code: (query.get('pair') ?? '').trim().toLowerCase(),
    ufrag,
    pwd,
    fingerprint: (query.get('fp') ?? '').trim().toLowerCase(),
    port,
    hosts: (query.get('ips') ?? '')
      .split(',')
      .map((host) => host.trim())
      .filter(Boolean),
  };
  const usable =
    invite.deviceId !== '' &&
    HEX_KEY.test(invite.publicKey) &&
    invite.code !== '' &&
    invite.ufrag !== '' &&
    invite.pwd !== '' &&
    HEX_KEY.test(invite.fingerprint) &&
    Number.isInteger(port) &&
    port > 0 &&
    port < 65536 &&
    invite.hosts.length > 0;
  return usable ? invite : null;
};

export const isLinkText = (text: string) => text.trim().toLowerCase().startsWith(LINK_PREFIX);

export const toHex = (bytes: Uint8Array) =>
  Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');

export const fromHex = (text: string): Uint8Array | null => {
  if (text.length % 2 !== 0 || !/^[0-9a-fA-F]*$/.test(text)) return null;
  const bytes = new Uint8Array(text.length / 2);
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number.parseInt(text.slice(index * 2, index * 2 + 2), 16);
  }
  return bytes;
};

/** The first 8 bytes of a public key as four groups of hex, as Rust's `fingerprint` shows it. */
export const keyFingerprint = (publicKey: Uint8Array) =>
  (toHex(publicKey.slice(0, 8)).match(/.{4}/g) ?? []).join(' ');
