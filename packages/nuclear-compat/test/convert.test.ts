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
  nuclear: { displayName: 'Sample', categories: ['metadata', 'streaming', 'lyrics', 'playlists'] },
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
        api.Providers.register({ id: 'lists', kind: 'playlists', name: 'Lists' });
      },
    };
    export default plugin;`,
  'sample-1.2.3/src/index.test.ts': `this file must be skipped`,
  'sample-1.2.3/README.md': 'docs',
});

/** A registered provider; its methods are called directly in these tests. */
type Registered = Pick<Provider, 'kind' | 'id' | 'name'> & {
  search(args: unknown): Promise<unknown>;
  searchPlaylists(args: unknown): Promise<unknown>;
  playlist(args: unknown): Promise<unknown>;
  candidates(args: unknown): Promise<unknown>;
  resolve(args: unknown): Promise<unknown>;
  lyrics(args: unknown): Promise<unknown>;
  charts(args: unknown): Promise<{ charts: unknown[] }>;
  chart(args: unknown): Promise<{ tracks: { title: string }[] }>;
  radio(args: unknown): Promise<{ tracks: { title: string }[] }>;
};

/** Runs a converted bundle with a fake `raygun` global; resolves with the registered providers. */
const runBundle = async (
  bundle: string,
  responses: Record<string, unknown> = {},
  youtube: Record<string, unknown> = {},
) => {
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
    // Nothing set on the plugin's page: plugins read their own stored values and defaults.
    settings: { get: () => null },
    youtube,
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
      unsupportedCategories: [],
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
        async onEnable(api) {
          const view = jsx(Button, {});
          // Listening while it starts is taken (and never called back); other calls refuse.
          const stop = api.Events.on('trackFinished', () => {});
          api.Queue.subscribeToCurrentItem(() => {});
          stop();
          const refused = await api.Queue.addToQueue([]).catch((error) => String(error));
          api.Providers.register({ id: 'l', kind: 'lyrics', name: 'L', getLyrics: async () => ({ type: 'instrumental', metadata: { refused } }) });
          return view && undefined;
        },
      };`;
    const { bundle, report } = convertPlugin(files, shim);
    expect(report.stubbedApis).toEqual(['Queue', 'Events']);
    expect(report.uiImports).toEqual(['@nuclearplayer/ui', 'react/jsx-runtime']);
    const { providers } = await runBundle(bundle);
    const lyrics = providers[0] as Registered;
    expect(
      await lyrics.lyrics({ title: 'x', artist: null, album: null, durationMs: null }),
    ).toBeNull();
  });

  it('finds modules beside an entry at the plugin root (Bandcamp Dashboard)', async () => {
    const files: Record<string, string> = {
      'package.json': JSON.stringify({
        name: 'nuclear-plugin-rooted',
        version: '0.1.0',
        main: 'src/index.ts',
        nuclear: { categories: ['lyrics'] },
      }),
      'client.ts': `export const words = () => 'from the client';`,
      'index.ts': `
        import { words } from './client.ts';
        export default {
          onEnable(api) {
            api.Providers.register({ id: 'l', kind: 'lyrics', name: 'L', getLyrics: async () => ({ type: 'plain', metadata: {}, sections: [{ lines: [{ segments: [{ text: words() }] }] }] }) });
          },
        };`,
    };
    const { bundle } = convertPlugin(files, shim);
    const { providers } = await runBundle(bundle);
    expect(
      await (providers[0] as Registered).lyrics({
        title: 'x',
        artist: null,
        album: null,
        durationMs: null,
      }),
    ).toEqual({ plain: 'from the client' });
  });

  it('declares providers the code registers beyond its categories (OmniSource)', async () => {
    // Says "streaming" but also registers metadata, as OmniSource 1.0.1 does.
    const files = pluginFiles({ ...PACKAGE, nuclear: { categories: ['streaming'] } });
    const { manifest, bundle } = convertPlugin(files, shim);
    expect(manifest.provides).toEqual(expect.arrayContaining(['music.stream', 'music.metadata']));
    const { providers } = await runBundle(bundle);
    expect(providers.map((provider) => provider.kind)).toContain('music.metadata');
  });

  it('loads plugins that draw in a page or call newer APIs (mini-player, Subsonic)', async () => {
    const files = pluginFiles({ ...PACKAGE, nuclear: { categories: ['lyrics'] } });
    files['sample-1.2.3/src/index.ts'] = `
      export default {
        async onEnable(api) {
          const panel = document.createElement('div');
          panel.classList.add('mini');
          panel.style.setProperty('--accent', 'red');
          panel.animate([], { duration: 1 }).play();
          document.body.appendChild(panel);
          document.fonts.ready;
          window.addEventListener('resize', () => {});
          window.requestAnimationFrame(() => { throw new Error('no frames are drawn'); });
          new window.MutationObserver(() => {}).observe(document.body, { childList: true });
          if (window.matchMedia('(prefers-color-scheme: dark)').matches) throw new Error('no page');
          api.Settings.registerWidget('test', () => null);
          await api.Settings.somethingNewer({ id: 'x' });
          api.Providers.register({ id: 'l', kind: 'lyrics', name: 'L', getLyrics: async () => ({ type: 'instrumental', metadata: {} }) });
        },
      };`;
    const { bundle } = convertPlugin(files, shim);
    const { providers } = await runBundle(bundle);
    expect(providers.map((provider) => provider.kind)).toEqual(['lyrics']);
  });

  it("plays a stream's HLS playlist (SoundCloud's), and refuses what isn't on the web", async () => {
    const files = pluginFiles({ ...PACKAGE, nuclear: { categories: ['streaming'] } });
    files['sample-1.2.3/src/index.ts'] = `
      export default {
        onEnable(api) {
          api.Providers.register({ id: 'cloud', kind: 'streaming', name: 'Cloud',
            searchForTrack: async () => [
              { id: 'h', title: 'In pieces', failed: false, source: { provider: 'c', id: 'h' } },
              { id: 'f', title: 'On disk', failed: false, source: { provider: 'c', id: 'f' } },
            ],
            getStreamUrl: async (id) => id === 'h'
              ? { url: 'https://hls.cloud.example/x/playlist.m3u8?sig=1', protocol: 'hls', source: { provider: 'c', id } }
              : { url: 'file:///sdcard/x.mp3', protocol: 'file', source: { provider: 'c', id } },
          });
        },
      };`;
    const { bundle } = convertPlugin(files, shim);
    const { providers } = await runBundle(bundle);
    const streams = providers[0] as Registered;
    await streams.candidates({ title: 'x', artist: 'y', album: null, durationMs: null, limit: 5 });
    expect(await streams.resolve({ id: 'h' })).toEqual({
      source: {
        kind: 'url',
        url: 'https://hls.cloud.example/x/playlist.m3u8?sig=1',
        headers: {},
      },
    });
    await expect(streams.resolve({ id: 'f' })).rejects.toThrow(
      "returned a file stream Raygun can't play",
    );
  });

  it('runs dashboards as charts and discovery as song radios (M46.23)', async () => {
    const files = pluginFiles({ ...PACKAGE, nuclear: { categories: ['dashboard', 'discovery'] } });
    files['sample-1.2.3/src/index.ts'] = `
      const track = (title) => ({ title, artists: [{ name: 'A', roles: ['main'] }], source: { provider: 'p', id: title } });
      export default {
        onEnable(api) {
          api.Providers.register({ id: 'top', kind: 'dashboard', name: 'Top', capabilities: ['topTracks'],
            fetchTopTracks: async () => [track('One'), track('Two'), track('Three')] });
          api.Providers.register({ id: 'like', kind: 'discovery', name: 'Like',
            getRecommendations: async (context, options) =>
              [track(context[0].artists[0].name + ' ' + options.limit)] });
        },
      };`;
    const { manifest, bundle } = convertPlugin(files, shim);
    expect(manifest.provides).toEqual(expect.arrayContaining(['music.charts', 'music.recommend']));
    const { providers } = await runBundle(bundle);
    const charts = providers.find((provider) => provider.kind === 'music.charts') as Registered;
    const { charts: list } = await charts.charts({ region: null });
    expect(list).toEqual([expect.objectContaining({ id: 'topTracks', group: 'top' })]);
    const top = await charts.chart({ id: 'topTracks', region: null, limit: 2 });
    expect(top.tracks.map((song) => song.title)).toEqual(['One', 'Two']);
    const radio = providers.find((provider) => provider.kind === 'music.recommend') as Registered;
    const songs = await radio.radio({ artist: 'Nina', title: null, limit: 5 });
    expect(songs.tracks[0]?.title).toBe('Nina 5');
  });

  it("answers Nuclear's Ytdlp from raygun.youtube, keeping what Nuclear's shapes drop", async () => {
    // A streaming and playlists plugin in the style of Nuclear's YouTube ones.
    const files: Record<string, string> = {
      'tube/package.json': JSON.stringify({
        name: 'nuclear-plugin-tube',
        version: '1.0.0',
        main: 'index.ts',
        nuclear: { displayName: 'Tube', categories: ['streaming', 'playlists'] },
      }),
      'tube/index.ts': `
        import type { NuclearPluginAPI } from '@nuclearplayer/plugin-sdk';
        const WATCH = 'https://www.youtube.com/watch?v=';
        export default {
          onEnable(api: NuclearPluginAPI) {
            api.Providers.register({
              id: 'tube', kind: 'streaming', name: 'Tube',
              async searchForTrack(artist: string, title: string) {
                const found = await api.Ytdlp.search(artist + ' ' + title);
                return found.map((result) => ({
                  id: result.id, title: result.title, failed: false,
                  durationMs: result.duration ? result.duration * 1000 : undefined,
                  source: { provider: 'tube', id: result.id },
                }));
              },
              async getStreamUrl(id: string) {
                const info = await api.Ytdlp.getStream(WATCH + id);
                return { url: info.stream_url, protocol: 'https', source: { provider: 'tube', id } };
              },
            });
            api.Providers.register({
              id: 'tube-lists', kind: 'playlists', name: 'Tube lists',
              matchesUrl: (url: string) => new URL(url).searchParams.has('list'),
              async fetchPlaylistByUrl(url: string) {
                const info = await api.Ytdlp.getPlaylist(url);
                return {
                  id: 'tube-' + info.id, name: info.title, isReadOnly: true,
                  origin: { provider: 'tube', id: info.id, url },
                  items: info.entries.map((entry) => ({
                    id: entry.id,
                    track: {
                      title: entry.title,
                      artists: [{ name: entry.channel, roles: ['main'] }],
                      durationMs: entry.duration * 1000,
                      artwork: { items: entry.thumbnails.map((thumb) => ({ ...thumb, purpose: 'thumbnail' })) },
                      source: { provider: 'tube', id: entry.id },
                    },
                  })),
                };
              },
            });
          },
        };`,
    };
    const { report, manifest, bundle } = convertPlugin(files, shim);
    expect(report.stubbedApis).toEqual([]);
    expect(report.supported).toEqual(['music.stream', 'music.metadata']);
    // What `Ytdlp` reaches on the plugin's behalf.
    expect(manifest.permissions).toEqual({
      net: ['*.googlevideo.com', 'music.youtube.com', 'www.youtube.com'],
    });

    const asked: unknown[][] = [];
    const STREAM = 'https://rr1---sn-x.googlevideo.com/videoplayback?itag=251';
    const LIST = 'https://www.youtube.com/playlist?list=PLSbDLF8wQ3oIkmgwFI3HmEYV-qhLwLVHx';
    const { providers } = await runBundle(
      bundle,
      {},
      {
        search: async (query: string, limit?: number) => {
          asked.push(['search', query, limit]);
          return [
            {
              id: 'wU26xVT_vBU',
              title: 'One More Time (Radio Edit)',
              channel: 'Daft Punk',
              album: 'One More Time',
              durationMs: 321_000,
              thumbnailUrl: 'https://i.ytimg.com/vi/wU26xVT_vBU/hqdefault.jpg',
              thumbnails: [],
              kind: 'song',
            },
          ];
        },
        stream: async (video: string, options: unknown) => {
          asked.push(['stream', video, options]);
          return {
            url: STREAM,
            headers: { 'User-Agent': 'com.google.visionos.youtube/1.04' },
            expiresAt: 1_900_000_000_000,
            mimeType: 'audio/webm; codecs="opus"',
            container: 'webm',
            codec: 'opus',
            bitrateKbps: 150,
            contentLength: 5_319_766,
            durationMs: 320_000,
            title: 'One More Time (Radio Edit)',
            channel: 'Daft Punk',
            album: null,
            artists: ['Daft Punk'],
            uploadDate: null,
            client: 'VISIONOS',
          };
        },
        playlist: async (playlist: string) => {
          asked.push(['playlist', playlist]);
          return {
            id: 'PLSbDLF8wQ3oIkmgwFI3HmEYV-qhLwLVHx',
            title: 'Mix Clips',
            author: 'Daft Punk',
            videos: [
              {
                id: 'FGBhQbmPwH8',
                title: 'One More Time',
                channel: 'Daft Punk',
                album: null,
                durationMs: 322_000,
                thumbnailUrl: 'https://i.ytimg.com/vi/FGBhQbmPwH8/hqdefault.jpg',
                thumbnails: [
                  { url: 'https://i.ytimg.com/vi/FGBhQbmPwH8/mq.jpg', width: 320, height: 180 },
                  {
                    url: 'https://i.ytimg.com/vi/FGBhQbmPwH8/hqdefault.jpg',
                    width: 480,
                    height: 360,
                  },
                ],
                kind: 'video',
              },
            ],
          };
        },
      },
    );
    const streams = providers.find((provider) => provider.kind === 'music.stream') as Registered;
    const lists = providers.find((provider) => provider.kind === 'music.metadata') as Registered;
    expect(lists.id).toBe('tube-lists');

    // Candidates are named by the channel the search gave, so the resolver can match them.
    expect(
      await streams.candidates({
        title: 'One More Time',
        artist: 'Daft Punk',
        album: null,
        durationMs: null,
        limit: 5,
      }),
    ).toEqual({
      candidates: [
        {
          id: 'wU26xVT_vBU',
          title: 'One More Time (Radio Edit)',
          artist: 'Daft Punk',
          durationMs: 321_000,
          quality: null,
          source: null,
        },
      ],
    });
    // What the stream is for reaches YouTube; its headers and expiry come back.
    expect(await streams.resolve({ id: 'wU26xVT_vBU', targetKbps: 128, decodable: true })).toEqual({
      source: {
        kind: 'url',
        url: STREAM,
        headers: { 'User-Agent': 'com.google.visionos.youtube/1.04' },
      },
      expiresAt: 1_900_000_000_000,
    });
    expect(asked[1]).toEqual([
      'stream',
      'https://www.youtube.com/watch?v=wU26xVT_vBU',
      { targetKbps: 128, decodable: true },
    ]);

    // Playlists by address: anything else finds nothing, without asking YouTube.
    expect(await lists.search({ query: 'one more time', limit: 5 })).toEqual({ tracks: [] });
    expect(
      await lists.searchPlaylists({ query: 'https://example.com/x', limit: 5, featured: false }),
    ).toEqual({ playlists: [] });
    const card = {
      id: LIST,
      title: 'Mix Clips',
      author: 'Daft Punk',
      artworkUrl: 'https://i.ytimg.com/vi/FGBhQbmPwH8/hqdefault.jpg',
      trackCount: 1,
      featured: false,
    };
    expect(await lists.searchPlaylists({ query: ` ${LIST} `, limit: 5, featured: false })).toEqual({
      playlists: [card],
    });
    const song = {
      id: 'FGBhQbmPwH8',
      title: 'One More Time',
      artist: 'Daft Punk',
      album: null,
      durationMs: 322_000,
      artworkUrl: 'https://i.ytimg.com/vi/FGBhQbmPwH8/hqdefault.jpg',
    };
    expect(await lists.playlist({ id: LIST })).toEqual({ playlist: card, tracks: [song] });
    expect(await lists.search({ query: LIST, limit: 5 })).toEqual({ tracks: [song] });
    // Read once for the card, its page and the search.
    expect(asked.filter(([call]) => call === 'playlist')).toEqual([['playlist', LIST]]);
  });

  it('rejects plugins Raygun has nothing to run for', () => {
    const files = pluginFiles({ ...PACKAGE, nuclear: { categories: ['queue', 'shell'] } });
    // Its code registers nothing Raygun runs either.
    files['sample-1.2.3/src/index.ts'] =
      `export default { onEnable(api) { api.Logger.info('hi'); } };`;
    expect(() => convertPlugin(files, shim)).toThrow(
      "Raygun can't run this Nuclear plugin yet: it provides queue, shell",
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
