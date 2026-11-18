import { commands } from '@raygun/bindings';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { openCore } from '../test/webCore';
import { NEEDS_APP, addressesOf, githubPlaces, sameRepo } from './sources';

type Site = Record<string, { body: string; type?: string; status?: number }>;

/** Stands in for the web: listed addresses answer, the rest fail as a CORS refusal does. */
const serve = (site: Site) => {
  const asked: string[] = [];
  vi.stubGlobal('fetch', async (input: RequestInfo | URL) => {
    const url = String(input);
    asked.push(url);
    const page = site[url];
    if (!page) throw new TypeError('Failed to fetch');
    return new Response(page.body, {
      status: page.status ?? 200,
      headers: { 'content-type': page.type ?? 'application/json' },
    });
  });
  return asked;
};

const CINEMETA = {
  id: 'com.linvo.cinemeta',
  version: '3.0.13',
  name: 'Cinemeta',
  description: 'The official addon for movie and series catalogs',
  resources: ['catalog', 'meta', 'addon_catalog'],
  types: ['movie', 'series'],
  catalogs: [
    { type: 'movie', id: 'top' },
    { type: 'series', id: 'top' },
  ],
};

const REPO = {
  formatVersion: 1,
  name: 'Open sources',
  extensions: [
    {
      id: 'dev.example.books',
      name: 'Books',
      version: '1.0.0',
      provides: ['novel.source'],
      package: 'packages/books.zip',
      sha256: 'a'.repeat(64),
    },
  ],
};

afterEach(() => vi.unstubAllGlobals());

describe('reading addresses', () => {
  it('takes stremio links, bare hosts and GitHub pages as the apps do', () => {
    expect(addressesOf('stremio://films.example/manifest.json')).toEqual([
      'https://films.example/manifest.json',
    ]);
    expect(addressesOf('films.example')).toEqual(['https://films.example/']);
    expect(() => addressesOf('films')).toThrow("isn't an address");
    expect(() => addressesOf('C:\\repos\\mine')).toThrow("can't read files");
    expect(githubPlaces('https://github.com/someone/sources')?.[1]).toBe(
      'https://raw.githubusercontent.com/someone/sources/HEAD/index.json',
    );
    expect(githubPlaces('https://example.org/someone/sources')).toBeNull();
    expect(
      sameRepo(
        'https://raw.githubusercontent.com/uvxszi/paraline-raygun/main/index.json',
        'https://raw.githubusercontent.com/uvxszi/paraline-raygun/HEAD/index.json',
      ),
    ).toBe(true);
  });
});

describe('Sources → Add in the browser', () => {
  it('reads a Stremio addon and says adding it needs the app', async () => {
    serve({ 'https://v3-cinemeta.strem.io/manifest.json': { body: JSON.stringify(CINEMETA) } });
    await openCore();
    const found = await commands.sourcesDetect('stremio://v3-cinemeta.strem.io/manifest.json');
    expect(found).toMatchObject({
      status: 'ok',
      data: { kind: 'addon', name: 'Cinemeta', note: NEEDS_APP },
    });
    if (found.status !== 'ok') return;
    expect(found.data.addon?.provides).toContainEqual({
      resource: 'catalog',
      kind: 'movie',
      count: 1,
    });
    const added = await commands.sourcesAdd('v3-cinemeta.strem.io/manifest.json');
    expect(added).toEqual({ status: 'error', error: NEEDS_APP });
  });

  it("adds a Raygun repository found from its project's GitHub page", async () => {
    const asked = serve({
      'https://raw.githubusercontent.com/someone/sources/HEAD/index.json': {
        body: JSON.stringify(REPO),
      },
    });
    await openCore();
    const found = await commands.sourcesDetect('https://github.com/someone/sources');
    expect(found).toMatchObject({
      status: 'ok',
      data: { kind: 'repository', format: 'raygun', name: 'Open sources', added: false },
    });
    if (found.status === 'ok')
      expect(found.data.entries.map((entry) => entry.id)).toEqual(['dev.example.books']);
    expect(asked).toContain(
      'https://raw.githubusercontent.com/someone/sources/repo/index.min.json',
    );

    const added = await commands.sourcesAdd('https://github.com/someone/sources');
    expect(added).toMatchObject({ status: 'ok', data: { added: true } });
    const repos = await commands.extensionRepos();
    expect(repos.status === 'ok' && repos.data.map((repo) => repo.name)).toContain('Open sources');
  });

  it("names other apps' repositories and servers, and leaves them to the app", async () => {
    serve({
      'https://novels.example/index.json': {
        body: JSON.stringify({ scripts: [{ name: 'One' }, { name: 'Two' }], libraries: [] }),
      },
      'https://music.example/rest/ping.view?v=1.16.1&c=Raygun&f=json': {
        body: JSON.stringify({ 'subsonic-response': { status: 'failed' } }),
      },
      'https://music.example/': { body: '<html></html>', type: 'text/html' },
      'https://navidrome.example/rest/ping.view?v=1.16.1&c=Raygun&f=json': {
        body: JSON.stringify({
          'subsonic-response': { status: 'failed', type: 'navidrome', openSubsonic: true },
        }),
      },
      'https://navidrome.example/': { body: '<html></html>', type: 'text/html' },
    });
    await openCore();
    expect(await commands.sourcesDetect('https://novels.example/index.json')).toMatchObject({
      status: 'ok',
      data: {
        kind: 'repository',
        format: 'shosetsu',
        note: NEEDS_APP,
        description: '2 extensions',
      },
    });
    expect(await commands.sourcesDetect('music.example')).toMatchObject({
      status: 'ok',
      data: { kind: 'server', server: 'subsonic', url: 'https://music.example/' },
    });
    expect(await commands.sourcesDetect('navidrome.example')).toMatchObject({
      status: 'ok',
      data: { kind: 'server', server: 'navidrome', url: 'https://navidrome.example/' },
    });
  });

  it('says when a site keeps web pages from reading it', async () => {
    serve({});
    await openCore();
    const found = await commands.sourcesDetect('https://closed.example/index.json');
    expect(found.status === 'error' && found.error).toContain(
      "closed.example doesn't let web pages read it",
    );
  });

  it('adds a list one address after another', async () => {
    serve({ 'https://repo.example/index.json': { body: JSON.stringify(REPO) } });
    await openCore();
    const queued = await commands.sourcesQueueAdd(
      '# mine\nhttps://repo.example/index.json\nhttps://repo.example/index.json\nnot one\n',
    );
    expect(queued).toMatchObject({ queued: 2, duplicates: 1 });
    await vi.waitFor(async () => {
      const status = await commands.sourcesQueueStatus();
      expect(status).toMatchObject({ running: false, added: 1, failed: 1 });
    });
  });
});
