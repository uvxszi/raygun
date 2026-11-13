import { openCore } from '../test/webCore';
import type { TestCore } from '../test/webCore';
import { ExtensionsService, PARALINE_URL, matchScore, mediaSourceOf, trackOf } from './service';
import { readZip } from './zip';

const encoder = new TextEncoder();

const deflate = async (data: Uint8Array) => {
  const stream = new ReadableStream<Uint8Array<ArrayBuffer>>({
    start: (controller) => {
      controller.enqueue(data.slice());
      controller.close();
    },
  }).pipeThrough(new CompressionStream('deflate-raw'));
  return new Uint8Array(await new Response(stream).arrayBuffer());
};

/** A zip archive: the first file stored, the others deflated (CRCs aren't checked). */
const zipOf = async (files: Record<string, string>) => {
  const locals: Uint8Array[] = [];
  const directory: Uint8Array[] = [];
  let offset = 0;
  let index = 0;
  for (const [name, text] of Object.entries(files)) {
    const plain = encoder.encode(text);
    const method = index === 0 ? 0 : 8;
    const data = method === 0 ? plain : await deflate(plain);
    const nameBytes = encoder.encode(name);
    const local = new Uint8Array(30 + nameBytes.length + data.length);
    const localView = new DataView(local.buffer);
    localView.setUint32(0, 0x04034b50, true);
    localView.setUint16(8, method, true);
    localView.setUint32(18, data.length, true);
    localView.setUint32(22, plain.length, true);
    localView.setUint16(26, nameBytes.length, true);
    local.set(nameBytes, 30);
    local.set(data, 30 + nameBytes.length);
    const entry = new Uint8Array(46 + nameBytes.length);
    const entryView = new DataView(entry.buffer);
    entryView.setUint32(0, 0x02014b50, true);
    entryView.setUint16(10, method, true);
    entryView.setUint32(20, data.length, true);
    entryView.setUint32(24, plain.length, true);
    entryView.setUint16(28, nameBytes.length, true);
    entryView.setUint32(42, offset, true);
    entry.set(nameBytes, 46);
    locals.push(local);
    directory.push(entry);
    offset += local.length;
    index += 1;
  }
  const directorySize = directory.reduce((total, part) => total + part.length, 0);
  const end = new Uint8Array(22);
  const endView = new DataView(end.buffer);
  endView.setUint32(0, 0x06054b50, true);
  endView.setUint16(8, directory.length, true);
  endView.setUint16(10, directory.length, true);
  endView.setUint32(12, directorySize, true);
  endView.setUint32(16, offset, true);
  const parts = [...locals, ...directory, end];
  const archive = new Uint8Array(parts.reduce((total, part) => total + part.length, 0));
  let at = 0;
  for (const part of parts) {
    archive.set(part, at);
    at += part.length;
  }
  return archive;
};

const sha256 = async (bytes: Uint8Array) =>
  Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes.slice())), (byte) =>
    byte.toString(16).padStart(2, '0'),
  ).join('');

let open: TestCore | null = null;
afterEach(() => {
  open?.ipc.uninstall();
  open = null;
  vi.unstubAllGlobals();
});

describe('sources in the browser', () => {
  it('reads packages stored or deflated', async () => {
    const archive = await zipOf({
      'raygun-extension.json': '{"id":"a"}',
      'dist/index.js': 'x'.repeat(500),
    });
    const files = await readZip(archive);
    expect(new TextDecoder().decode(files.get('raygun-extension.json'))).toBe('{"id":"a"}');
    expect(files.get('dist/index.js')).toHaveLength(500);
    await expect(readZip(encoder.encode('not a zip'))).rejects.toThrow("isn't a zip archive");
  });

  it('checks what providers answer, as the app’s host does', () => {
    expect(
      trackOf({
        id: 7,
        title: 'Song',
        artist: ' ',
        durationMs: 1234.4,
        artworkUrl: 'file:///etc/passwd',
        source: { kind: 'url', url: 'https://example.com/a.mp3', headers: {} },
      }),
    ).toEqual({
      externalId: '7',
      title: 'Song',
      artist: null,
      album: null,
      durationMs: 1234,
      artworkUrl: null,
      source: { kind: 'url', url: 'https://example.com/a.mp3', headers: {} },
      explicit: false,
    });
    expect(trackOf({ title: '' })).toBeNull();
    // A source without headers is dropped, as the SDK says.
    expect(mediaSourceOf({ kind: 'url', url: 'https://example.com/a.mp3' })).toBeNull();
    expect(mediaSourceOf({ kind: 'url', url: 'javascript:alert(1)', headers: {} })).toBeNull();
    expect(mediaSourceOf({ kind: 'youTube', videoId: 'abc' })).toEqual({
      kind: 'youTube',
      videoId: 'abc',
    });
  });

  it('matches candidates by title first, then artist', () => {
    const wanted = { title: 'Clair de Lune', artist: 'Debussy' };
    expect(
      matchScore(wanted, { title: 'Clair de lune', artist: 'Claude Debussy' }),
    ).toBeGreaterThan(0.8);
    expect(matchScore(wanted, { title: 'Moonlight Sonata', artist: 'Beethoven' })).toBeLessThan(
      0.3,
    );
  });

  it('lists Paraline’s sources and installs one from its package', async () => {
    open = await openCore();
    const script =
      'raygun.providers.register({ kind: "music.metadata", id: "songs", search: async () => ({ tracks: [] }) });';
    const archive = await zipOf({
      'raygun-extension.json': JSON.stringify({
        id: 'dev.raygun.test',
        name: 'Test source',
        version: '1.2.0',
        runtime: 'js',
        entry: 'dist/index.js',
        provides: ['music.metadata'],
        permissions: { net: ['example.com'] },
        settings: [{ key: 'count', type: 'number', default: 6 }],
      }),
      'dist/index.js': script,
    });
    const index = {
      name: 'Paraline',
      extensions: [
        {
          id: 'dev.raygun.test',
          name: 'Test source',
          version: '1.2.0',
          provides: ['music.metadata'],
          package: 'packages/test.zip',
          sha256: await sha256(archive),
        },
        { id: 'org.kodi.thing', name: 'A Kodi add-on', format: 'kodi', package: 'x.zip' },
      ],
    };
    const fetched: string[] = [];
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) => {
        fetched.push(String(url));
        if (String(url) === PARALINE_URL) return new Response(JSON.stringify(index));
        return new Response(archive.slice());
      }),
    );
    const service = new ExtensionsService(open.core.context, () => open!.core.settings.stored());
    const [catalog] = await service.catalog(false);
    expect(catalog?.error).toBeNull();
    expect(catalog?.entries.map((entry) => [entry.id, entry.state, entry.unsupported])).toEqual([
      ['dev.raygun.test', 'available', null],
      [
        'org.kodi.thing',
        'available',
        "Only Raygun's own sources run in the browser; this one needs the app.",
      ],
    ]);

    await service.install('paraline', 'dev.raygun.test');
    expect(fetched).toContain(new URL('packages/test.zip', PARALINE_URL).href);
    const [installed] = service.installed();
    expect(installed?.manifest.name).toBe('Test source');
    expect(installed?.source).toBe(script);
    const [info] = service.list();
    expect(info?.settings[0]).toMatchObject({
      key: 'count',
      kind: 'number',
      default: 6,
      value: null,
    });
    const [again] = await service.catalog(false);
    expect(again?.entries[0]?.state).toBe('installed');

    await expect(service.install('paraline', 'org.kodi.thing')).rejects.toThrow('needs the app');
    service.uninstall('dev.raygun.test');
    expect(service.installed()).toEqual([]);
  });

  it('refuses a package that doesn’t match its checksum', async () => {
    open = await openCore();
    const index = {
      extensions: [{ id: 'dev.raygun.test', package: 'p.zip', sha256: '00'.repeat(32) }],
    };
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) =>
        String(url) === PARALINE_URL
          ? new Response(JSON.stringify(index))
          : new Response(encoder.encode('tampered')),
      ),
    );
    const service = new ExtensionsService(open.core.context, () => open!.core.settings.stored());
    await expect(service.install('paraline', 'dev.raygun.test')).rejects.toThrow('checksum');
  });
});
