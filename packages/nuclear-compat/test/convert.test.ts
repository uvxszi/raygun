import type { LyricsResult, Provider } from '@raygun/sdk';

import { buildShim } from '../scripts/build.mjs';
import { convertPlugin } from '../src/convert';

let shim: string;

beforeAll(async () => {
  shim = await buildShim();
});

const PACKAGE = {
  name: 'nuclear-plugin-sample',
  version: '1.2.3',
  description: 'A sample',
  author: { name: 'Someone' },
  main: 'src/index.ts',
  nuclear: { displayName: 'Sample', categories: ['metadata', 'streaming', 'lyrics', 'dashboard'] },
};

/** A small multi-file plugin in Nuclear's style: TS, relative imports, a default export. */
const pluginFiles = (pkg: object = PACKAGE): Record<string, string> => ({
  'sample-1.2.3/package.json': JSON.stringify(pkg),
  'sample-1.2.3/src/config.ts': `export const config = { apiBase: 'https://api.sample.org', id: 'Sample Provider' } as const;`,
  'sample-1.2.3/src/client.ts': `
    import type { FetchFunction } from '@nuclearplayer/plugin-sdk';
    import { config } from './config';
    export class Client {
      constructor(private readonly fetch: FetchFunction) {}
      async search(query: string): Promise<{ title: string; artist: string }[]> {
        const url = new URL('/search', config.apiBase);
        url.search = new URLSearchParams({ q: query }).toString();
        const response = await this.fetch(url.toString(), { headers: { Accept: 'application/json' } });
        if (!response.ok) throw new Error('HTTP ' + response.status);
        return (await response.json()) as { title: string; artist: string }[];
      }
    }`,
  'sample-1.2.3/src/index.ts': `
    import type { NuclearPlugin, NuclearPluginAPI } from '@nuclearplayer/plugin-sdk';
    import { Client } from './client';
    import { config } from './config';
    const plugin: NuclearPlugin = {
      async onLoad(api: NuclearPluginAPI) {
        await api.Settings.register([{ id: 'region', default: 'eu' }]);
      },
      async onEnable(api: NuclearPluginAPI) {
        const client = new Client(api.Http.fetch);
        const region = await api.Settings.get('region');
        api.Providers.register({
          id: config.id, kind: 'metadata', name: 'Sample',
          async searchTracks({ query }: { query: string }) {
            const songs = await client.search(query);
            return songs.map((song, index) => ({
              title: song.title, artists: [{ name: song.artist, roles: [] }], durationMs: 1000 * (index + 1),
              artwork: { items: [{ url: 'https://img.sample.org/s.jpg', width: 100 }, { url: 'https://img.sample.org/l.jpg', width: 600 }] },
              source: { provider: 'sample', id: 'track-' + index + '-' + region },
            }));
          },
        });
        api.Providers.register({
          id: 'sample-streams', kind: 'streaming', name: 'Sample streams',
          async searchForTrack(artist: string, title: string) {
            return [
              { id: 'c1', title: artist + ' - ' + title, failed: false, source: { provider: 'sample', id: 'c1' } },
              { id: 'bad', title: 'x', failed: true, source: { provider: 'sample', id: 'bad' } },
            ];
          },
          async getStreamUrl(id: string) {
            return { url: 'https://cdn.sample.org/' + id + '.mp3', protocol: 'https', source: { provider: 'sample', id } };
          },
        });
        api.Providers.register({
          id: 'sample-lyrics', kind: 'lyrics', name: 'Sample lyrics',
          async getLyrics(track: { title: string }) {
            return { type: 'lineSynced', metadata: {}, sections: [
              { lines: [ { startMs: 1000, endMs: 2000, segments: [{ text: 'Hello ' }, { text: track.title }] } ] },
            ] };
          },
        });
        api.Providers.register({ id: 'dash', kind: 'dashboard', name: 'Dash' });
      },
    };
    export default plugin;`,
  'sample-1.2.3/src/index.test.ts': `this file must be skipped`,
  'sample-1.2.3/README.md': 'docs',
});

/** A registered provider; its methods are called directly in these tests. */
type Registered = Pick<Provider, 'kind' | 'id' | 'name'> & {
  search(args: unknown): Promise<unknown>;
  candidates(args: unknown): Promise<unknown>;
  resolve(args: unknown): Promise<unknown>;
  lyrics(args: unknown): Promise<unknown>;
};

/** Runs a converted bundle with a fake `raygun` global; resolves with the registered providers. */
const runBundle = async (bundle: string, responses: Record<string, unknown> = {}) => {
  const providers: Registered[] = [];
  const storage = new Map<string, unknown>();
  const requests: { url: string; headers: Record<string, string> }[] = [];
  const raygun = {
    log: { debug() {}, info() {}, warn() {}, error() {} },
    time: { sleep: (ms: number) => new Promise((resolve) => setTimeout(resolve, ms)) },
    storage: {
      get: (key: string) => storage.get(key) ?? null,
      set: (key: string, value: unknown) => storage.set(key, value),
    },
    providers: { register: (provider: Registered) => providers.push(provider) },
    http: {
      fetch: async (url: string, init: { headers: Record<string, string> }) => {
        requests.push({ url, headers: init.headers });
        const body = responses[url];
        return {
          status: body === undefined ? 404 : 200,
          ok: body !== undefined,
          url,
          headers: { 'content-type': 'application/json' },
          text: async () => JSON.stringify(body ?? {}),
          json: async () => body,
        };
      },
    },
  };
  const scope = { raygun } as Record<string, unknown>;
  new Function('globalThis', 'raygun', bundle).call(scope, scope, raygun);
  await (scope.__rg_ready as Promise<void>);
  return { providers, requests };
};

describe('convertPlugin', () => {
  it('builds a manifest and report from package.json and the sources', () => {
    const { manifest, report } = convertPlugin(pluginFiles(), shim);
    expect(manifest).toMatchObject({
      id: 'nuclear.nuclear-plugin-sample',
      name: 'Sample',
      version: '1.2.3',
      apiVersion: 1,
      runtime: 'js',
      entry: 'dist/index.js',
      author: 'Someone',
      provides: ['music.metadata', 'music.stream', 'lyrics'],
      permissions: { net: ['api.sample.org', 'cdn.sample.org', 'img.sample.org'] },
    });
    expect(report).toEqual({
      supported: ['music.metadata', 'music.stream', 'lyrics'],
      unsupportedCategories: ['dashboard'],
      stubbedApis: [],
      uiImports: [],
      hosts: ['api.sample.org', 'cdn.sample.org', 'img.sample.org'],
      modules: 3,
    });
  });

  it('runs the plugin and exposes its providers as Raygun providers', async () => {
    const { bundle } = convertPlugin(pluginFiles(), shim);
    const { providers, requests } = await runBundle(bundle, {
      'https://api.sample.org/search?q=Hello+Dolly': [
        { title: 'Hello, Dolly', artist: 'Louis Armstrong' },
      ],
    });
    expect(providers.map((provider) => [provider.kind, provider.id, provider.name])).toEqual([
      ['music.metadata', 'sample-provider', 'Sample'],
      ['music.stream', 'sample-streams', 'Sample streams'],
      ['lyrics', 'sample-lyrics', 'Sample lyrics'],
    ]);
    const [metadata, streams, lyrics] = providers as [Registered, Registered, Registered];

    expect(await metadata.search({ query: 'Hello Dolly', limit: 10 })).toEqual({
      tracks: [
        {
          id: 'track-0-eu',
          title: 'Hello, Dolly',
          artist: 'Louis Armstrong',
          album: null,
          durationMs: 1000,
          artworkUrl: 'https://img.sample.org/l.jpg',
        },
      ],
    });
    expect(requests[0]?.headers).toEqual({ Accept: 'application/json' });

    const found = (await streams.candidates({
      title: 'Song',
      artist: 'Band',
      album: null,
      durationMs: null,
      limit: 5,
    })) as { candidates: { id: string }[] };
    expect(found.candidates.map((candidate) => candidate.id)).toEqual(['c1']);
    expect(await streams.resolve({ id: 'c1' })).toEqual({
      source: { kind: 'url', url: 'https://cdn.sample.org/c1.mp3', headers: {} },
    });

    const result = (await lyrics.lyrics({
      title: 'World',
      artist: 'Band',
      album: null,
      durationMs: null,
    })) as LyricsResult;
    expect(result).toEqual({ synced: [{ timeMs: 1000, text: 'Hello World' }] });
  });

  it('stubs unsupported APIs and reports them, and UI imports stay inert', async () => {
    const files = pluginFiles({ ...PACKAGE, nuclear: { categories: ['lyrics'] } });
    files['sample-1.2.3/src/index.ts'] = `
      import { jsx } from 'react/jsx-runtime';
      import { Button } from '@nuclearplayer/ui';
      export default {
        onEnable(api) {
          const view = jsx(Button, {});
          try { api.Queue.addToQueue([]); } catch (error) { api.Logger.info(String(error)); }
          api.Providers.register({ id: 'l', kind: 'lyrics', name: 'L', getLyrics: async () => ({ type: 'instrumental', metadata: {} }) });
          return view && undefined;
        },
      };`;
    const { bundle, report } = convertPlugin(files, shim);
    expect(report.stubbedApis).toEqual(['Queue']);
    expect(report.uiImports).toEqual(['@nuclearplayer/ui', 'react/jsx-runtime']);
    const { providers } = await runBundle(bundle);
    const lyrics = providers[0] as Registered;
    expect(
      await lyrics.lyrics({ title: 'x', artist: null, album: null, durationMs: null }),
    ).toBeNull();
  });

  it('declares providers the code registers beyond its categories (OmniSource)', async () => {
    // Says "streaming" but also registers metadata, as OmniSource 1.0.1 does.
    const files = pluginFiles({ ...PACKAGE, nuclear: { categories: ['streaming'] } });
    const { manifest, bundle } = convertPlugin(files, shim);
    expect(manifest.provides).toEqual(expect.arrayContaining(['music.stream', 'music.metadata']));
    const { providers } = await runBundle(bundle);
    expect(providers.map((provider) => provider.kind)).toContain('music.metadata');
  });

  it('rejects plugins Raygun has nothing to run for', () => {
    const files = pluginFiles({ ...PACKAGE, nuclear: { categories: ['dashboard', 'playlists'] } });
    // Its code registers nothing Raygun runs either.
    files['sample-1.2.3/src/index.ts'] =
      `export default { onEnable(api) { api.Logger.info('hi'); } };`;
    expect(() => convertPlugin(files, shim)).toThrow(
      "Raygun can't run this Nuclear plugin yet: it provides dashboard, playlists",
    );
    expect(() => convertPlugin({ 'x/readme.md': '' }, shim)).toThrow('there is no package.json');
  });
});

describe('the committed converter', () => {
  it('matches a fresh build (run `pnpm --filter @raygun/nuclear-compat build` if this fails)', async () => {
    const { readFile } = await import('node:fs/promises');
    const { OUTPUT, buildConverter } = await import('../scripts/build.mjs');
    const committed = (await readFile(OUTPUT, 'utf8')).replaceAll('\r\n', '\n');
    expect(committed).toBe(await buildConverter());
  });
});
