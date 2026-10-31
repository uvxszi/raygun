import { mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { MANIFEST_FILE, build, bundle, dev, init, manifestProblems } from '../lib/cli.mjs';

const SDK_DIR = dirname(dirname(fileURLToPath(import.meta.url)));
const EXTENSIONS_DIR = join(SDK_DIR, '..', '..', 'extensions');

let work: string;

beforeEach(async () => {
  work = await mkdtemp(join(tmpdir(), 'raygun-sdk-test-'));
});

afterEach(async () => {
  await rm(work, { recursive: true, force: true });
});

/** A new extension from the template, able to resolve `@raygun/sdk` like an installed one. */
const createExtension = async (name = 'ext') => {
  const dir = join(work, name);
  await init(dir, { id: 'dev.example.test', name: 'Test Source' });
  await mkdir(join(dir, 'node_modules', '@raygun'), { recursive: true });
  await symlink(SDK_DIR, join(dir, 'node_modules', '@raygun', 'sdk'), 'junction');
  return dir;
};

describe('first-party extensions', () => {
  it('have up-to-date bundles committed (run `pnpm build:extensions` if this fails)', async () => {
    const names = (await readdir(EXTENSIONS_DIR, { withFileTypes: true }))
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name);
    expect(names.length).toBeGreaterThan(0);
    for (const name of names) {
      const dir = join(EXTENSIONS_DIR, name);
      const { manifest, code } = await bundle(dir);
      const committed = await readFile(join(dir, manifest.entry), 'utf8');
      expect(committed.replaceAll('\r\n', '\n'), `${name} bundle is stale`).toBe(code);
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
    expect(manifestProblems(valid)).toEqual([]);
  });

  it.each([
    [{ id: 'NoDots' }, 'id'],
    [{ name: '' }, 'name'],
    [{ version: '1.0' }, 'version'],
    [{ apiVersion: 2 }, 'apiVersion'],
    [{ runtime: 'lua' }, 'runtime'],
    [{ entry: '../x.js' }, 'entry'],
    [{ entry: 'dist/index.ts' }, 'entry'],
    [{ provides: [] }, 'provides'],
  ])('rejects %o', (override, field) => {
    const problems = manifestProblems({ ...valid, ...override });
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain(field);
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
