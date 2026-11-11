import { mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createHash } from 'node:crypto';

import { unzipSync } from 'fflate';

import {
  KINDS,
  MANIFEST_FILE,
  RAYGUN_ROOT,
  SITE_KIND,
  build,
  bundle,
  callArgs,
  checkManifest,
  compareVersions,
  dev,
  init,
  manifestProblems,
  pack,
  packAll,
  sdkDependency,
  testCommand,
  validate,
} from '../lib/cli.mjs';
import {
  allowsHost,
  matchesPattern,
  parseHostPattern,
  parseMatchPattern,
} from '../lib/manifest.mjs';
import { fetchedHosts } from '../lib/check.mjs';

const SDK_DIR = dirname(dirname(fileURLToPath(import.meta.url)));
const EXTENSIONS_DIR = join(SDK_DIR, '..', '..', 'extensions');

let work: string;

beforeEach(async () => {
  work = await mkdtemp(join(tmpdir(), 'raygun-sdk-test-'));
});

afterEach(async () => {
  await rm(work, { recursive: true, force: true });
});

/** A new extension from a template, able to resolve `@raygun/sdk` like an installed one. */
const createExtension = async (name = 'ext', id = 'dev.example.test', kind?: string) => {
  const dir = join(work, name);
  await init(dir, { id, name: 'Test Source', kind });
  await mkdir(join(dir, 'node_modules', '@raygun'), { recursive: true });
  await symlink(SDK_DIR, join(dir, 'node_modules', '@raygun', 'sdk'), 'junction');
  return dir;
};

let made = 0;

/** An extension whose `src/index.ts` is `source` and whose manifest has `overrides`. */
const extensionWith = async (source: string, overrides: Record<string, unknown> = {}) => {
  made += 1;
  const dir = await createExtension(`with-${made}`);
  const manifestPath = join(dir, MANIFEST_FILE);
  const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
  await writeFile(manifestPath, JSON.stringify({ ...manifest, ...overrides }));
  await writeFile(join(dir, 'src/index.ts'), source);
  return dir;
};

describe('first-party extensions', () => {
  const firstParty = async () =>
    (await readdir(EXTENSIONS_DIR, { withFileTypes: true }))
      .filter((entry) => entry.isDirectory())
      .map((entry) => join(EXTENSIONS_DIR, entry.name));

  it('have up-to-date bundles committed (run `pnpm build:extensions` if this fails)', async () => {
    const dirs = await firstParty();
    expect(dirs.length).toBeGreaterThan(0);
    for (const dir of dirs) {
      // Every script it builds: a site-scripts extension may have several.
      const { files } = await bundle(dir);
      for (const [path, code] of Object.entries(files)) {
        const committed = await readFile(join(dir, path), 'utf8');
        expect(committed.replaceAll('\r\n', '\n'), `${dir} ${path} is stale`).toBe(code);
      }
    }
  });

  it('pass raygun-ext validate', async () => {
    for (const dir of await firstParty()) {
      const { errors } = await validate(dir);
      expect(errors, dir).toEqual([]);
    }
  });
});

describe('manifest checks', () => {
  const valid = {
    id: 'dev.example.test',
    name: 'Test',
    version: '1.0.0',
    apiVersion: 1,
    runtime: 'js',
    entry: 'dist/index.js',
    provides: ['music.metadata'],
  };

  it('accepts a valid manifest', () => {
    expect(checkManifest(valid)).toEqual({ errors: [], warnings: [] });
    expect(
      checkManifest({
        ...valid,
        version: '1.0.0-beta.2+build.5',
        permissions: { net: ['api.example.org', '*.example.org', '*'], webview: false },
        contentRating: 'suggestive',
        description: null,
        website: 'https://example.org',
        settings: [
          { key: 'language', type: 'select', options: ['en', 'fr'], default: 'en', label: null },
          { key: 'count', type: 'number', default: 6 },
          { key: 'large', type: 'boolean' },
          { key: 'apiKey', type: 'text', label: 'API key', required: true },
          { key: 'server', type: 'text', required: false },
        ],
      }),
    ).toEqual({ errors: [], warnings: [] });
  });

  it.each([
    [{ id: 'NoDots' }, 'id'],
    [{ name: '' }, 'name'],
    [{ version: '1.0' }, 'version'],
    [{ version: '01.0.0' }, 'version'],
    [{ apiVersion: 2 }, 'apiVersion'],
    [{ runtime: 'lua' }, 'runtime'],
    [{ entry: '../x.js' }, 'entry'],
    [{ entry: 'C:/x.js' }, 'entry'],
    [{ entry: 'dist/index.ts' }, 'entry'],
    [{ provides: [] }, 'provides'],
    [{ provides: ['Music'] }, 'provides'],
    [{ permission: { net: [] } }, 'unknown field "permission"'],
    [{ permissions: { net: ['https://example.org'] } }, 'permissions.net'],
    [{ permissions: { net: ['*.org'] } }, 'permissions.net'],
    [{ permissions: { hosts: [] } }, 'permissions: unknown field "hosts"'],
    [{ permissions: null }, 'permissions'],
    [{ contentRating: 'adult' }, 'contentRating'],
    [
      {
        settings: [
          { key: 'a', type: 'text' },
          { key: 'a', type: 'text' },
        ],
      },
      'declared twice',
    ],
    [{ settings: [{ key: 'a', type: 'select' }] }, 'needs options'],
    [
      { settings: [{ key: 'a', type: 'select', options: ['x'], default: 'y' }] },
      'one of its options',
    ],
    [{ settings: [{ key: 'a', type: 'number', default: '6' }] }, 'must be a number'],
    [{ settings: [{ key: 'a', type: 'text', hint: 'x' }] }, 'unknown field "hint"'],
    [{ settings: [{ key: 'a', type: 'color' }] }, 'type must be one of'],
    [{ settings: [{ key: 'a', type: 'text', required: 'yes' }] }, 'required must be true or false'],
    [{ settings: [{ key: 'a', type: 'boolean', required: true }] }, "can't be required"],
    [{ description: 3 }, 'description must be text'],
  ])('rejects %o', (override, words) => {
    const problems = manifestProblems({ ...valid, ...override });
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain(words);
  });

  it('warns about what Raygun passes over', () => {
    const { errors, warnings } = checkManifest({
      ...valid,
      provides: ['music.metadata', 'music.radio'],
      permissions: { webview: true },
      website: 'example.org',
    });
    expect(errors).toEqual([]);
    expect(warnings.join('\n')).toMatch(/never calls providers of kind "music.radio"/);
    expect(warnings.join('\n')).toMatch(/webview is reserved/);
    expect(warnings.join('\n')).toMatch(/website must start with https/);
  });

  it('reads network patterns like the host', () => {
    expect(parseHostPattern('*')).toEqual({ any: true });
    expect(parseHostPattern(' API.Example.org ')).toEqual({
      host: 'api.example.org',
      subdomains: false,
    });
    expect(parseHostPattern('*.example.org')).toEqual({ host: 'example.org', subdomains: true });
    expect(parseHostPattern('*.org')).toBeNull();
    expect(parseHostPattern('-bad.org')).toBeNull();
    const net = ['archive.org', '*.archive.org'];
    expect(allowsHost(net, 'archive.org')).toBe(true);
    expect(allowsHost(net, 'ia800.us.archive.org')).toBe(true);
    expect(allowsHost(net, 'notarchive.org')).toBe(false);
    expect(allowsHost(['*.example.org'], 'example.org')).toBe(false);
    expect(allowsHost(['*'], 'api.example.org')).toBe(true);
    expect(allowsHost(['*'], 'localhost')).toBe(false);
    expect(allowsHost(['*'], '192.168.1.2')).toBe(false);
  });
});

describe('templates', () => {
  it('has one for every provider kind', async () => {
    const folders = await readdir(join(SDK_DIR, 'templates'));
    for (const kind of Object.keys(KINDS)) {
      expect(folders, kind).toContain(kind.replace(/\./g, '-'));
    }
  });

  it.each(Object.keys(KINDS))('creates a %s source that builds and validates', async (kind) => {
    const dir = await createExtension(kind, `dev.example.${kind.replace('.', '-')}`, kind);
    const manifest = JSON.parse(await readFile(join(dir, MANIFEST_FILE), 'utf8'));
    expect(manifest.provides).toEqual([kind]);
    const { code } = await bundle(dir);
    // A plain script: the host has no module loader.
    expect(code).not.toMatch(/^\s*(import|export)\s/m);
    expect(code).not.toContain('__NAME__');
    expect(code).not.toContain('__ID__');
    const { errors, warnings, providers, sites } = await validate(dir);
    expect(errors).toEqual([]);
    expect(warnings).toEqual([]);
    if (kind === SITE_KIND) {
      // Site scripts register nothing: they run in pages, where their patterns say.
      expect(providers).toEqual([]);
      expect(sites?.length).toBeGreaterThan(0);
      return;
    }
    expect(providers).toHaveLength(1);
    const [provider] = providers;
    expect(provider?.kind).toBe(kind);
    expect(provider?.name).toBe('Test Source');
    for (const method of KINDS[kind]?.required ?? []) {
      expect(provider?.methods, `${kind} ${method}`).toContain(method);
    }
  });

  it('fills in names that need escaping', async () => {
    const dir = join(work, 'quoted');
    await init(dir, { id: 'dev.example.quoted', name: `Bob's "best"`, kind: 'lyrics' });
    const manifest = JSON.parse(await readFile(join(dir, MANIFEST_FILE), 'utf8'));
    expect(manifest.name).toBe(`Bob's "best"`);
    expect(JSON.parse(await readFile(join(dir, 'package.json'), 'utf8')).name).toBe(
      'dev.example.quoted',
    );
    expect(await readFile(join(dir, 'src/index.ts'), 'utf8')).toContain(`name: 'Bob\\'s "best"'`);
  });

  it('refuses a kind without a template', async () => {
    await expect(
      init(join(work, 'x'), { id: 'dev.example.x', name: 'X', kind: 'music.radio' }),
    ).rejects.toThrow(/no template for music.radio; pick one of music.metadata/);
  });

  it('links the SDK outside the workspace and uses it inside', () => {
    expect(sdkDependency(join(RAYGUN_ROOT, 'extensions', 'my-source'))).toBe('workspace:*');
    expect(sdkDependency(join(RAYGUN_ROOT, 'extensions', 'a', 'b'))).toMatch(/^link:/);
    const outside = sdkDependency(join(work, 'my-source'));
    expect(outside).toMatch(/^link:/);
    expect(outside).not.toContain('\\');
    expect(outside.endsWith('packages/sdk')).toBe(true);
  });
});

describe('raygun-ext validate', () => {
  it('reports what would make Raygun refuse a provider or a call', async () => {
    const dir = await extensionWith(`
      import { registerProvider } from '@raygun/sdk';
      registerProvider({ kind: 'lyrics', id: 'words', lyrics: async () => null } as never);
      registerProvider({ kind: 'music.metadata', id: 'Bad Id', search: async () => ({ tracks: [] }) });
      registerProvider({ kind: 'music.metadata', id: 'search' } as never);
      registerProvider({ kind: 'music.metadata', id: 'search', search: async () => ({ tracks: [] }) });
    `);
    const { errors } = await validate(dir);
    expect(errors).toEqual([
      expect.stringMatching(/"words" is a lyrics, which the manifest's provides doesn't list/),
      expect.stringMatching(/"Bad Id": its id must be/),
      expect.stringMatching(/"search" \(music.metadata\) has no search: Raygun calls it/),
      expect.stringMatching(/"search" is registered twice/),
    ]);
  });

  it('warns about likely mistakes', async () => {
    const dir = await extensionWith(
      `
      import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
      const API = 'https://api.example.org';
      const OTHER = 'https://elsewhere.example.com/v1';
      registerProvider(defineProvider({
        kind: 'music.metadata',
        id: 'search',
        search: async ({ query }) => {
          await new Promise((done) => setTimeout(done, 10));
          await fetchJson(\`\${API}/search?q=\${query}\`);
          await fetchJson(\`\${OTHER}/search?q=\${query}\`);
          return { tracks: [] };
        },
        searchArtist: async () => ({ artists: [] }),
      } as never));
    `,
      { permissions: { net: ['api.example.org'] }, provides: ['music.metadata', 'lyrics'] },
    );
    const { errors, warnings } = await validate(dir, { types: false });
    expect(errors).toEqual([]);
    expect(warnings).toEqual([
      expect.stringMatching(/never calls searchArtist .*\(did you mean searchArtists\?\)/),
      'provides lists lyrics, but no provider of that kind is registered',
      expect.stringMatching(
        /fetches from elsewhere.example.com, which permissions.net doesn't list/,
      ),
      expect.stringMatching(
        /uses setTimeout, which Raygun's QuickJS doesn't have: use raygun.time.sleep/,
      ),
    ]);
  });

  it('checks types against the provider contracts', async () => {
    const dir = await extensionWith(`
      import { defineProvider, registerProvider } from '@raygun/sdk';
      registerProvider(defineProvider({
        kind: 'music.metadata',
        id: 'search',
        search: async () => {
          setTimeout(() => {}, 10);
          return { tracks: [{ name: 'No title' }] };
        },
      }));
    `);
    const { errors } = await validate(dir);
    expect(errors).toEqual([
      expect.stringMatching(
        /^type error: src\/index\.ts:6:9 .*\n\s+.*Property 'title' is missing .* 'Track'/s,
      ),
      expect.stringMatching(/^type error: src\/index\.ts:7:11 Cannot find name 'setTimeout'/),
    ]);
  });

  it('reports scripts that fail to load or register nothing', async () => {
    const failing = await extensionWith(`throw new Error('no network at load');`);
    expect((await validate(failing)).errors).toEqual([
      'the script fails while it loads: no network at load',
    ]);
    const empty = await extensionWith(`export {};`);
    expect((await validate(empty)).errors).toEqual([
      'the script registers no providers: call registerProvider at its top level',
    ]);
    const unbuildable = await extensionWith(`import { nope } from './missing';\nnope();`);
    expect((await validate(unbuildable)).errors[0]).toMatch(/^the build fails: .*missing/s);
  });

  it('stops at manifest errors', async () => {
    const dir = await extensionWith('export {};', { version: 'one' });
    const { manifest, errors } = await validate(dir);
    expect(manifest).toBeNull();
    expect(errors).toEqual(['version must be semver, like 1.0.0']);
  });

  it('finds the hosts a bundle fetches from', () => {
    const code = `
      var SITE = "https://www.example.org";
      var SOURCES = \`\${SITE}/sources\`;
      raygun.http.fetch(\`\${SOURCES}/episodes.json\`);
      fetchJson2("https://api.example.com/x?q=" + q);
      const address = \`\${SITE}/books/\${id}\`;
      raygun.http.fetch(address);
      raygun.http.fetch(\`https://\${server}/BookReader\`);
      fetchText(url);
    `;
    expect([...fetchedHosts(code)].sort()).toEqual(['api.example.com', 'www.example.org']);
  });
});

describe('raygun-ext test', () => {
  it('fills a call with the arguments the app passes it', () => {
    expect(callArgs('audiobook.directory', 'search', ['query=jane austen'])).toEqual({
      query: 'jane austen',
      page: 1,
    });
    expect(callArgs('music.stream', 'candidates', ['title=Maple Leaf Rag', 'limit=3'])).toEqual({
      title: 'Maple Leaf Rag',
      artist: null,
      album: null,
      durationMs: null,
      limit: 3,
    });
    expect(callArgs('manga.source', 'pages', ['{"chapterId":"x"}'])).toEqual({ chapterId: 'x' });
    expect(callArgs('', 'whatever', ['a=1', 'b=true', 'c=text'])).toEqual({
      a: 1,
      b: true,
      c: 'text',
    });
    expect(() => callArgs('lyrics', 'lyrics', ['title'])).toThrow(/key=value/);
  });

  it('runs the call example of Raygun’s extension host', () => {
    const { command, args } = testCommand(work, {
      provider: 'books',
      method: 'search',
      args: { query: 'x', page: 1 },
      settings: ['language=English'],
      nsfw: true,
    });
    expect(command).toBe('cargo');
    expect(args.slice(0, 9)).toEqual([
      'run',
      '--quiet',
      '--manifest-path',
      join(RAYGUN_ROOT, 'Cargo.toml'),
      '-p',
      'raygun-ext-host',
      '--example',
      'call',
      '--',
    ]);
    expect(args.slice(9)).toEqual([
      work,
      'books',
      'search',
      '{"query":"x","page":1}',
      '--set',
      'language=English',
      '--nsfw',
    ]);
    // Without a provider it only lists what the extension registers.
    expect(testCommand(work).args.slice(9)).toEqual([work]);
  });
});

describe('raygun-ext', () => {
  it('creates an extension from the template that builds into one script', async () => {
    const dir = await createExtension();
    const manifest = JSON.parse(await readFile(join(dir, MANIFEST_FILE), 'utf8'));
    expect(manifest).toMatchObject({ id: 'dev.example.test', name: 'Test Source' });
    expect(await readFile(join(dir, 'src/index.ts'), 'utf8')).toContain("name: 'Test Source'");

    const { outfile, bytes } = await build(dir);
    const code = await readFile(outfile, 'utf8');
    expect(outfile).toBe(join(dir, 'dist/index.js'));
    expect(bytes).toBeGreaterThan(0);
    expect(code).toMatch(/^\/\/ dev\.example\.test 0\.1\.0\./);
    // A plain script: the host has no module loader.
    expect(code).not.toMatch(/^\s*(import|export)\s/m);
    expect(code).not.toContain('require(');
    expect(code).toContain('raygun.providers.register');
  });

  it('refuses to create an extension in a folder that has files', async () => {
    await writeFile(join(work, 'existing.txt'), 'x');
    await expect(init(work, { id: 'dev.example.test', name: 'X' })).rejects.toThrow('not empty');
  });

  it('reports manifest problems and build errors', async () => {
    const dir = await createExtension();
    const manifestPath = join(dir, MANIFEST_FILE);
    const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
    await writeFile(manifestPath, JSON.stringify({ ...manifest, apiVersion: 9 }));
    await expect(build(dir)).rejects.toThrow('apiVersion must be 1');

    await writeFile(manifestPath, JSON.stringify(manifest));
    await writeFile(join(dir, 'src/index.ts'), 'import { nope } from "./missing";\nnope();');
    await expect(build(dir)).rejects.toThrow(/missing/);
  });

  it('dev installs every good build into an extensions folder', async () => {
    const dir = await createExtension();
    const extensions = join(work, 'app-extensions');
    const events: string[] = [];
    let installed = 0;
    const waitForInstall = (count: number) =>
      vi.waitFor(() => expect(installed).toBeGreaterThanOrEqual(count), { timeout: 10_000 });

    const session = await dev(dir, { into: extensions }, (event) => {
      events.push(event.kind);
      if (event.kind === 'installed') installed += 1;
    });
    try {
      await waitForInstall(1);
      const target = join(extensions, 'dev.example.test');
      expect((await readdir(target)).sort()).toEqual(['dist', MANIFEST_FILE]);

      const source = join(dir, 'src/index.ts');
      const original = await readFile(source, 'utf8');
      await writeFile(source, original.replace("name: 'Test Source'", "name: 'Edited'"));
      await waitForInstall(2);
      await vi.waitFor(
        async () =>
          expect(await readFile(join(target, 'dist/index.js'), 'utf8')).toContain('Edited'),
        { timeout: 10_000 },
      );
      // No staging copies left behind for the app to trip over.
      expect(await readdir(extensions)).toEqual(['dev.example.test']);
    } finally {
      await session.stop();
    }
    expect(events).not.toContain('failed');
  });
});

describe('repositories', () => {
  it('orders versions like semver', () => {
    expect(compareVersions('1.10.0', '1.9.0')).toBe(1);
    expect(compareVersions('1.0.0', '1.0.0')).toBe(0);
    expect(compareVersions('1.0.0-beta', '1.0.0')).toBe(-1);
    expect(compareVersions('2.0.0', '10.0.0')).toBe(-1);
  });

  it('packs reproducibly and indexes the newest version of each extension', async () => {
    const dir = await createExtension();
    const repo = join(work, 'repo');

    const first = await pack(dir, repo);
    const firstBytes = await readFile(join(repo, first.package));
    const second = await pack(dir, repo);
    expect(await readFile(join(repo, second.package))).toEqual(firstBytes);

    const manifestPath = join(dir, MANIFEST_FILE);
    const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
    await writeFile(manifestPath, JSON.stringify({ ...manifest, version: '0.2.0' }));
    const { index } = await pack(dir, repo);

    expect((await readdir(join(repo, 'packages'))).sort()).toEqual([
      'dev.example.test-0.1.0.zip',
      'dev.example.test-0.2.0.zip',
    ]);
    expect(index.extensions).toHaveLength(1);
    const [entry] = index.extensions;
    const packageBytes = await readFile(join(repo, 'packages/dev.example.test-0.2.0.zip'));
    expect(entry).toMatchObject({
      id: 'dev.example.test',
      version: '0.2.0',
      apiVersion: 1,
      package: 'packages/dev.example.test-0.2.0.zip',
      permissions: { net: ['api.openverse.org'], webview: false },
      size: packageBytes.length,
      sha256: createHash('sha256').update(packageBytes).digest('hex'),
    });
    const written = JSON.parse(await readFile(join(repo, 'index.json'), 'utf8'));
    expect(written).toEqual(index);
    // A new repository is named after its folder, or as asked, and keeps its name.
    expect(written.name).toBe('repo');
    expect((await pack(dir, repo, { name: 'Example sources' })).index.name).toBe('Example sources');
    expect((await pack(dir, repo)).index.name).toBe('Example sources');
  });

  it('publishes an extension’s icon beside its packages', async () => {
    const dir = await createExtension();
    const repo = join(work, 'repo');
    const first = await pack(dir, repo);
    expect(first.index.extensions[0]).not.toHaveProperty('icon');

    await writeFile(join(dir, 'icon.webp'), 'webp');
    const { index } = await pack(dir, repo);
    expect(index.extensions[0]).toMatchObject({ icon: 'icons/dev.example.test.webp' });
    expect(await readFile(join(repo, 'icons', 'dev.example.test.webp'), 'utf8')).toBe('webp');

    // A new icon of another type replaces the old one.
    await rm(join(dir, 'icon.webp'));
    await writeFile(join(dir, 'icon.png'), 'png');
    expect((await pack(dir, repo)).index.extensions[0]).toMatchObject({
      icon: 'icons/dev.example.test.png',
    });
    expect(await readdir(join(repo, 'icons'))).toEqual(['dev.example.test.png']);
  });

  it('refuses to publish an extension that fails validation', async () => {
    const dir = await extensionWith(`
      import { registerProvider } from '@raygun/sdk';
      registerProvider({ kind: 'manga.source', id: 'comics', popular: async () => ({ items: [], hasNextPage: false }) } as never);
    `);
    await expect(pack(dir, join(work, 'repo'))).rejects.toThrow(/isn't ready to publish/);
  });

  it('packs every extension in a folder, scripts or not, and indexes them once', async () => {
    await createExtension('extensions/alpha', 'dev.example.alpha');
    await createExtension('extensions/beta', 'dev.example.beta');
    // Not an extension: no manifest.
    await mkdir(join(work, 'extensions', 'notes'), { recursive: true });
    const repo = join(work, 'repo');

    const { packed, index } = await packAll(join(work, 'extensions'), repo);
    expect(packed.map((result) => result.manifest.id)).toEqual([
      'dev.example.alpha',
      'dev.example.beta',
    ]);
    expect(packed.every((result) => !result.kept)).toBe(true);
    expect(index.extensions.map((entry: { id: string }) => entry.id)).toEqual([
      'dev.example.alpha',
      'dev.example.beta',
    ]);
    expect(JSON.parse(await readFile(join(repo, 'index.json'), 'utf8'))).toEqual(index);

    // A manifest laid out another way is the same package; changed code under a published
    // version keeps the published one.
    const alphaPackage = join(repo, 'packages', 'dev.example.alpha-0.1.0.zip');
    const published = await readFile(alphaPackage);
    const manifestPath = join(work, 'extensions', 'alpha', MANIFEST_FILE);
    await writeFile(manifestPath, JSON.stringify(JSON.parse(await readFile(manifestPath, 'utf8'))));
    const source = join(work, 'extensions', 'beta', 'src/index.ts');
    await writeFile(source, `${await readFile(source, 'utf8')}\nconsole.log('changed');\n`);
    const again = await packAll(join(work, 'extensions'), repo);
    expect(again.packed.map((result) => result.kept)).toEqual([false, true]);
    expect(await readFile(alphaPackage)).toEqual(published);
    expect(again.index).toEqual(index);
  });

  it('refuses a folder without extensions', async () => {
    await expect(packAll(work, join(work, 'repo'))).rejects.toThrow('has no extensions');
  });
});

describe('site scripts', () => {
  const site = {
    id: 'dev.example.site',
    name: 'Site',
    version: '1.0.0',
    apiVersion: 1,
    runtime: 'site',
    entry: 'dist/page.js',
    provides: ['site.scripts'],
    site: {
      scripts: [
        {
          matches: ['https://*.example.com/*'],
          excludeMatches: ['https://www.example.com/embed/*'],
          js: ['dist/page.js'],
          css: ['styles/page.css'],
          runAt: 'document-start',
          allFrames: true,
        },
      ],
    },
  };

  it('accepts a site-scripts manifest and checks its site section like the host', () => {
    expect(checkManifest(site)).toEqual({ errors: [], warnings: [] });
    const group = site.site.scripts[0];
    for (const [patch, field] of [
      [{ provides: ['site.scripts', 'lyrics'] }, 'provides'],
      [{ permissions: { net: ['api.example.com'] } }, 'permissions'],
      [{ entry: 'dist/other.js' }, 'entry'],
      [{ site: undefined }, 'site'],
      [{ runtime: 'js' }, 'site'],
      [{ site: { scripts: [] } }, 'site.scripts'],
      [{ site: { scripts: [{ ...group, matches: ['nope'] }] } }, 'matches'],
      [{ site: { scripts: [{ ...group, js: ['../page.js'] }] } }, 'js'],
      [{ site: { scripts: [{ ...group, css: ['page.scss'] }] } }, 'css'],
      [{ site: { scripts: [{ ...group, runAt: 'later' }] } }, 'runAt'],
      [{ site: { scripts: [{ ...group, world: 'main' }] } }, 'world'],
    ] as const) {
      const { errors } = checkManifest({ ...site, ...patch });
      expect(errors.join('\n'), field).toContain(field);
    }
  });

  it('reads match patterns as Raygun does', () => {
    expect(parseMatchPattern('https://*.youtube.com/*')).toMatchObject({ subdomains: true });
    for (const bad of [
      '<all_urls>',
      'https://example.com',
      'https://Example.com/*',
      'ftp://a.b/*',
    ]) {
      expect(parseMatchPattern(bad), bad).toBeNull();
    }
    const cases: [string, string, boolean][] = [
      ['https://*.youtube.com/*', 'https://youtube.com/', true],
      ['https://*.youtube.com/*', 'https://m.youtube.com/watch?v=abc', true],
      ['https://*.youtube.com/*', 'http://m.youtube.com/', false],
      ['https://*.youtube.com/*', 'https://youtube.com.evil.example/', false],
      ['*://www.example.com/watch?v=*', 'https://www.example.com:8443/watch?v=1#t', true],
      ['*://www.example.com/watch?v=*', 'https://www.example.com/watch', false],
      ['https://a.example/x/*/y', 'https://a.example/x/1/2/y', true],
      ['*://*/*', 'https://a.b/', true],
    ];
    for (const [pattern, address, expected] of cases) {
      expect(matchesPattern(pattern, address), `${pattern} ${address}`).toBe(expected);
    }
  });

  it('creates, builds, packs and indexes site scripts with the sites they run on', async () => {
    const dir = await createExtension('wiki', 'dev.example.wiki', 'site-scripts');
    const manifest = JSON.parse(await readFile(join(dir, MANIFEST_FILE), 'utf8'));
    expect(manifest.runtime).toBe('site');
    const { bytes } = await build(dir);
    expect(bytes).toBeGreaterThan(0);
    // A page script: no module syntax, and `raygunPage` left for Raygun's wrapper.
    const code = await readFile(join(dir, 'dist/index.js'), 'utf8');
    expect(code).not.toMatch(/^\s*(import|export)\s/m);
    expect(code).toContain('raygunPage.settings');
    const repo = join(work, 'repo');
    const packed = await pack(dir, repo);
    const listed = packed.index.extensions.find((entry) => entry.id === 'dev.example.wiki');
    expect(listed?.sites).toEqual([
      'https://en.m.wikipedia.org/wiki/*',
      'https://en.wikipedia.org/wiki/*',
    ]);
    const archive = unzipSync(await readFile(join(repo, packed.package)));
    expect(Object.keys(archive).sort()).toEqual([
      'dist/index.js',
      MANIFEST_FILE,
      'styles/index.css',
    ]);
  });

  it('tries site scripts on a page address in Raygun’s host', () => {
    const { args } = testCommand('ext', {
      address: 'https://en.wikipedia.org/wiki/Ada_Lovelace',
      bundle: 'out.js',
      settings: ['readingTime=false'],
    });
    expect(args.slice(args.indexOf('--') + 2)).toEqual([
      'https://en.wikipedia.org/wiki/Ada_Lovelace',
      '--bundle',
      expect.stringMatching(/out\.js$/),
      '--set',
      'readingTime=false',
    ]);
  });
});
