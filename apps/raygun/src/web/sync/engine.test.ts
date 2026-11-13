import { commands } from '@raygun/bindings';
import type { TrackInput } from '@raygun/bindings';

import type { WebCore } from '../core';
import { openCore } from '../test/webCore';
import type { TestCore } from '../test/webCore';
import { ByteStream } from './channel';
import type { Connect, SyncEngine } from './engine';
import { parseInvite, toHex } from './protocol';
import type { WebInvite } from './protocol';

const track = (title: string): TrackInput => ({
  title,
  artist: 'Artist',
  album: null,
  durationMs: 1000,
  artworkUrl: null,
  source: null,
});

/** Two byte streams wired to each other; `responder` answers the far end. */
const directTo =
  (responder: SyncEngine): Connect =>
  async () => {
    const near: ByteStream = new ByteStream((bytes) => queueMicrotask(() => far.push(bytes)));
    const far: ByteStream = new ByteStream((bytes) => queueMicrotask(() => near.push(bytes)));
    void responder.respond(far).catch(() => undefined);
    return {
      stream: near,
      close: () => {
        near.close();
        far.close();
      },
    };
  };

const inviteFor = (core: WebCore, code: string): WebInvite => ({
  name: 'Phone',
  deviceId: core.context.deviceId,
  publicKey: toHex(core.sync.identity.publicKey),
  code,
  ufrag: 'ufrag',
  pwd: 'password-password-pass',
  fingerprint: 'ab'.repeat(32),
  port: 8840,
  hosts: ['192.168.1.20'],
});

let cores: TestCore[] = [];
afterEach(() => {
  for (const core of cores) core.ipc.uninstall();
  cores = [];
});

/** Two web cores; the second answers the first's connections. */
const twoDevices = async () => {
  const phone = await openCore();
  const browser = await openCore({}, { connect: directTo(phone.core.sync) });
  cores = [phone, browser];
  return { phone: phone.core, browser: browser.core };
};

describe('device sync between two web cores', () => {
  it('reads a link code that carries a web link, and refuses others', () => {
    const text =
      'raygun:link?at=192.168.1.20%3A8820&code=abcd234567&name=Pixel&sync=0190-id&key=' +
      'a1'.repeat(32) +
      '&pair=596rr88k&ice=uIKc%3ATbRL5rG3qOG79WSlOU6sfQIx&fp=' +
      'df'.repeat(32) +
      '&udp=8840&ips=192.168.1.20,127.0.0.1';
    expect(parseInvite(text)).toEqual({
      name: 'Pixel',
      deviceId: '0190-id',
      publicKey: 'a1'.repeat(32),
      code: '596rr88k',
      ufrag: 'uIKc',
      pwd: 'TbRL5rG3qOG79WSlOU6sfQIx',
      fingerprint: 'df'.repeat(32),
      port: 8840,
      hosts: ['192.168.1.20', '127.0.0.1'],
    });
    // An app too old to link with browsers sends the code without the web fields.
    expect(parseInvite('raygun:link?at=192.168.1.20%3A8820&code=abcd234567&name=Pixel')).toBeNull();
    expect(parseInvite('hello')).toBeNull();
  });

  it('links with a code, then syncs both ways', async () => {
    const { phone, browser } = await twoDevices();
    phone.sync.acceptCode('abcd2345');
    // Each side has something the other doesn't.
    phone.collections.setFavorite(track('On the phone'), true);
    browser.collections.setFavorite(track('In the browser'), true);

    const peer = await browser.sync.link(inviteFor(phone, 'abcd2345'));
    expect(peer.name).toBe(phone.sync.name());
    expect(phone.sync.peers().map((other) => other.deviceId)).toEqual([browser.context.deviceId]);

    const summary = await browser.sync.syncWith(phone.context.deviceId);
    expect(summary).toMatchObject({ sent: 1, received: 1, changed: 1 });
    const titles = (core: WebCore) =>
      core.collections
        .favorites()
        .map((favorite) => favorite.title)
        .sort();
    expect(titles(browser)).toEqual(['In the browser', 'On the phone']);
    expect(titles(phone)).toEqual(['In the browser', 'On the phone']);

    // Nothing new: nothing goes again.
    expect(await browser.sync.syncWith(phone.context.deviceId)).toMatchObject({
      sent: 0,
      received: 0,
    });
  });

  it('refuses a wrong code and a device it doesn’t know', async () => {
    const { phone, browser } = await twoDevices();
    phone.sync.acceptCode('right123');
    await expect(browser.sync.link(inviteFor(phone, 'wrong123'))).rejects.toThrow(
      'That pairing code is wrong.',
    );
    await expect(browser.sync.syncWith(phone.context.deviceId)).rejects.toThrow("isn't linked");
  });

  it('settles a song both devices favorited on one row', async () => {
    const { phone, browser } = await twoDevices();
    phone.sync.acceptCode('abcd2345');
    await browser.sync.link(inviteFor(phone, 'abcd2345'));
    phone.collections.setFavorite(track('Same song'), true);
    browser.collections.setFavorite(track('Same song'), true);
    await browser.sync.syncWith(phone.context.deviceId);
    // The tombstone made on one side goes back on the next sync.
    await browser.sync.syncWith(phone.context.deviceId);
    const live = (core: WebCore) =>
      core.collections.favorites().filter((favorite) => favorite.title === 'Same song');
    expect(live(browser)).toHaveLength(1);
    expect(live(phone)).toHaveLength(1);
  });

  it('carries settings but not the ones each device keeps', async () => {
    const { phone, browser } = await twoDevices();
    phone.sync.acceptCode('abcd2345');
    await browser.sync.link(inviteFor(phone, 'abcd2345'));
    // Through the page's own bindings, as Settings changes them.
    await commands.settingsUpdate({ key: 'theme', value: 'dark' });
    await commands.settingsUpdate({ key: 'checkUpdates', value: false });
    await browser.sync.syncWith(phone.context.deviceId);
    expect(phone.settings.stored().theme).toBe('dark');
    expect(phone.settings.stored().checkUpdates).toBe(true);
  });
});
