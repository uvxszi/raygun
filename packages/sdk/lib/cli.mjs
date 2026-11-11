// raygun-ext: creates, checks, tries and builds Raygun extensions, installs them into a running
// app and packs them into repositories (M12, M13). Plain ESM with JSDoc types, so it runs on Node
// without a build step; tsc checks it (checkJs). The guide is docs/extensions.

import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, watch } from 'node:fs';
import { cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { isDeepStrictEqual } from 'node:util';

import * as esbuild from 'esbuild';
import { unzipSync, zipSync } from 'fflate';

import { checkBundle } from './check.mjs';
import {
  KINDS,
  SITE_KIND,
  checkManifest,
  manifestProblems,
  siteFiles,
  sitePatterns,
} from './manifest.mjs';
import { typeProblems } from './typecheck.mjs';

export { KINDS, SITE_KIND, checkManifest, manifestProblems, siteFiles, sitePatterns };

export const MANIFEST_FILE = 'raygun-extension.json';
export const SOURCE_ENTRY = 'src/index.ts';
export const ANDROID_PACKAGE = 'app.raygun.player';
const SDK_DIR = fileURLToPath(new URL('..', import.meta.url));
const SDK_INDEX = join(SDK_DIR, 'src', 'index.ts');
const SDK_SITE = join(SDK_DIR, 'src', 'site.ts');
const TEMPLATES_DIR = join(SDK_DIR, 'templates');
/** The Raygun checkout the SDK sits in: its Cargo workspace runs `raygun-ext test`. */
export const RAYGUN_ROOT = resolve(SDK_DIR, '..', '..');
/** The kind `init` creates without `--kind`. */
export const DEFAULT_KIND = 'music.metadata';
/** Raygun's largest package (crates/raygun-ext-host/src/package.rs). */
const MAX_PACKAGE_BYTES = 10 * 1024 * 1024;

/** @typedef {import('../src/types').Manifest} Manifest */

/**
 * @param {string} dir
 * @returns {Promise<Manifest>}
 */
export const readManifest = async (dir) => {
  const path = join(dir, MANIFEST_FILE);
  let manifest;
  try {
    manifest = JSON.parse(await readFile(path, 'utf8'));
  } catch (error) {
    throw new Error(`could not read ${path}: ${error instanceof Error ? error.message : error}`, {
      cause: error,
    });
  }
  const problems = manifestProblems(manifest);
  if (problems.length > 0) {
    throw new Error(`${path}:\n  - ${problems.join('\n  - ')}`);
  }
  return manifest;
};

/**
 * esbuild options: one self-contained script, since the host evaluates the entry as a plain
 * script with no module loader. `@raygun/sdk` is this CLI's own, so an extension builds against
 * the contracts it's checked with, even before its packages are installed.
 * @param {string} dir
 * @param {Manifest} manifest
 * @returns {esbuild.BuildOptions & { write: false }}
 */
const buildOptions = (dir, manifest) => ({
  absWorkingDir: resolve(dir),
  entryPoints: [SOURCE_ENTRY],
  outfile: manifest.entry,
  alias: { '@raygun/sdk': SDK_INDEX },
  bundle: true,
  write: false,
  format: 'iife',
  platform: 'neutral',
  mainFields: ['module', 'main'],
  target: 'es2022',
  charset: 'utf8',
  legalComments: 'inline',
  logLevel: 'silent',
  banner: {
    js: `// ${manifest.id} ${manifest.version}. Built by raygun-ext from ${SOURCE_ENTRY}; do not edit.`,
  },
});

/** Whether an extension is site scripts (runtime "site"), which run in pages, not in Raygun. */
export const isSite = (/** @type {Manifest} */ manifest) => manifest.runtime === 'site';

/**
 * The scripts of a site-scripts extension that are built, each from its source in `src/` (a
 * script `dist/<name>.js` from `src/<name>.ts`, or `.js`); scripts without one are taken as they
 * are, like its styles.
 * @param {string} dir
 * @param {Manifest} manifest
 * @returns {{ source: string, out: string }[]}
 */
export const siteBuilds = (dir, manifest) =>
  siteFiles(/** @type {Record<string, unknown>} */ (manifest)).js.flatMap((out) => {
    const name = basename(out, '.js');
    const source = ['ts', 'js']
      .map((type) => `src/${name}.${type}`)
      .find((candidate) => existsSync(join(dir, candidate)));
    return source ? [{ source, out }] : [];
  });

/**
 * esbuild options for site scripts: each built script is one self-contained script for pages.
 * @param {string} dir
 * @param {Manifest} manifest
 * @param {{ source: string, out: string }[]} builds
 * @returns {esbuild.BuildOptions & { write: false }}
 */
const siteBuildOptions = (dir, manifest, builds) => ({
  absWorkingDir: resolve(dir),
  entryPoints: builds.map(({ source, out }) => ({ in: source, out: out.replace(/\.js$/, '') })),
  outdir: resolve(dir),
  alias: { '@raygun/sdk/site': SDK_SITE },
  bundle: true,
  write: false,
  format: 'iife',
  platform: 'browser',
  target: 'es2022',
  charset: 'utf8',
  legalComments: 'inline',
  logLevel: 'silent',
  banner: {
    js: `// ${manifest.id} ${manifest.version}. Built by raygun-ext from src/; do not edit.`,
  },
});

/**
 * The files a built extension's package holds besides its manifest, by path: the entry, or a
 * site-scripts extension's scripts and styles.
 * @param {string} dir
 * @param {Manifest} manifest
 * @param {Record<string, string>} built
 * @returns {Promise<Record<string, string>>}
 */
const packageFiles = async (dir, manifest, built) => {
  if (!isSite(manifest)) return built;
  const { js, css } = siteFiles(/** @type {Record<string, unknown>} */ (manifest));
  /** @type {Record<string, string>} */
  const files = {};
  for (const path of [...js, ...css]) {
    files[path] = built[path] ?? (await readFile(join(dir, path), 'utf8'));
  }
  return files;
};

/**
 * Bundles an extension without writing it: `code` is its entry, `files` everything it builds
 * (a site-scripts extension builds each of its scripts that has a source).
 * @param {string} dir
 * @returns {Promise<{ manifest: Manifest, code: string, files: Record<string, string> }>}
 */
export const bundle = async (dir) => {
  const manifest = await readManifest(dir);
  if (isSite(manifest)) {
    const builds = siteBuilds(dir, manifest);
    /** @type {Record<string, string>} */
    const files = {};
    if (builds.length > 0) {
      const result = await esbuild.build(siteBuildOptions(dir, manifest, builds));
      for (const output of result.outputFiles) {
        files[relative(resolve(dir), output.path).split(sep).join('/')] = output.text;
      }
    }
    const code =
      files[manifest.entry] ?? (await readFile(join(dir, manifest.entry), 'utf8').catch(() => ''));
    if (!code) throw new Error(`${manifest.entry} has no source in src/ and isn't there`);
    return { manifest, code, files };
  }
  const result = await esbuild.build(buildOptions(dir, manifest));
  const output = result.outputFiles[0];
  if (!output) throw new Error('esbuild produced no output');
  return { manifest, code: output.text, files: { [manifest.entry]: output.text } };
};

/**
 * Bundles an extension into its manifest's entry (and a site-scripts extension's other scripts).
 * @param {string} dir
 * @returns {Promise<{ manifest: Manifest, outfile: string, bytes: number }>}
 */
export const build = async (dir) => {
  const { manifest, files } = await bundle(dir);
  let bytes = 0;
  for (const [path, code] of Object.entries(files)) {
    const outfile = join(dir, path);
    await mkdir(dirname(outfile), { recursive: true });
    await writeFile(outfile, code);
    bytes += Buffer.byteLength(code);
  }
  return { manifest, outfile: join(dir, manifest.entry), bytes };
};

/**
 * Copies the files the app loads (manifest and entry) into `<extensionsDir>/<id>/`.
 * @param {string} dir
 * @param {Manifest} manifest
 * @param {string} extensionsDir
 */
export const installInto = async (dir, manifest, extensionsDir) => {
  const target = join(extensionsDir, manifest.id);
  // Stage under a dot-folder (the app ignores those) and swap, so it never sees half a copy.
  const staging = join(extensionsDir, `.staging-${manifest.id}`);
  await rm(staging, { recursive: true, force: true });
  await stage(dir, manifest, staging);
  await rm(target, { recursive: true, force: true });
  await cp(staging, target, { recursive: true });
  await rm(staging, { recursive: true, force: true });
  return target;
};

/**
 * @param {string} dir
 * @param {Manifest} manifest
 * @param {string} staging
 */
const stage = async (dir, manifest, staging) => {
  await mkdir(staging, { recursive: true });
  await cp(join(dir, MANIFEST_FILE), join(staging, MANIFEST_FILE));
  const paths = isSite(manifest)
    ? Object.values(siteFiles(/** @type {Record<string, unknown>} */ (manifest))).flat()
    : [manifest.entry];
  for (const path of paths) {
    await mkdir(dirname(join(staging, path)), { recursive: true });
    await cp(join(dir, path), join(staging, path));
  }
};

/** The adb executable: $ADB, else the SDK's platform-tools, else `adb` on PATH. */
export const adbPath = () => {
  if (process.env.ADB) return process.env.ADB;
  const sdk =
    process.env.ANDROID_HOME ??
    process.env.ANDROID_SDK_ROOT ??
    (process.env.LOCALAPPDATA ? join(process.env.LOCALAPPDATA, 'Android', 'Sdk') : undefined);
  return sdk ? join(sdk, 'platform-tools', 'adb') : 'adb';
};

/**
 * Runs adb with stdin closed: with an open stdin, `adb shell` waits on it and never returns.
 * @param {string[]} args
 * @param {string | undefined} serial
 */
const adb = (args, serial) =>
  new Promise((resolvePromise, reject) => {
    const fullArgs = serial ? ['-s', serial, ...args] : args;
    const child = spawn(adbPath(), fullArgs, { stdio: ['ignore', 'pipe', 'pipe'] });
    let output = '';
    child.stdout.on('data', (chunk) => (output += chunk));
    child.stderr.on('data', (chunk) => (output += chunk));
    child.on('error', reject);
    child.on('close', (code) =>
      code === 0
        ? resolvePromise(output)
        : reject(new Error(`adb ${args[0]} failed (${code}): ${output.trim()}`)),
    );
  });

/**
 * Installs into the debug app's sandbox on a device or emulator. `adb push` can't write there
 * directly, so the files go through /data/local/tmp and `run-as` copies them in.
 * @param {string} dir
 * @param {Manifest} manifest
 * @param {{ serial?: string }} [options]
 */
export const installAndroid = async (dir, manifest, { serial } = {}) => {
  const local = await mkdtemp(join(tmpdir(), 'raygun-ext-'));
  try {
    const staging = join(local, manifest.id);
    await stage(dir, manifest, staging);
    const remote = `/data/local/tmp/raygun-ext/${manifest.id}`;
    await adb(['shell', `rm -rf ${remote} && mkdir -p /data/local/tmp/raygun-ext`], serial);
    await adb(['push', staging, remote], serial);
    const swap = [
      'mkdir -p extensions',
      `rm -rf extensions/.staging-${manifest.id}`,
      `cp -r ${remote} extensions/.staging-${manifest.id}`,
      `rm -rf extensions/${manifest.id}`,
      `mv extensions/.staging-${manifest.id} extensions/${manifest.id}`,
    ].join(' && ');
    await adb(['shell', `run-as ${ANDROID_PACKAGE} sh -c '${swap}'`], serial);
    await adb(['shell', `rm -rf ${remote}`], serial);
  } finally {
    await rm(local, { recursive: true, force: true });
  }
};

/**
 * @typedef {{ into: string } | { android: true, serial?: string }} InstallTarget
 * @typedef {{ kind: 'installed', manifest: Manifest, bytes: number, where: string }
 *   | { kind: 'failed', message: string }} DevEvent
 */

/**
 * Rebuilds on every change to the sources or the manifest and installs each good build.
 * @param {string} dir
 * @param {InstallTarget} target
 * @param {(event: DevEvent) => void} onEvent
 * @returns {Promise<{ stop: () => Promise<void> }>}
 */
export const dev = async (dir, target, onEvent) => {
  let manifest = await readManifest(dir);
  // Builds can finish faster than installs; installs run one at a time, in order.
  let installing = Promise.resolve();

  /** @type {esbuild.Plugin} */
  const install = {
    name: 'raygun-install',
    setup(pluginBuild) {
      pluginBuild.onEnd((result) => {
        const outputs = result.outputFiles ?? [];
        if (result.errors.length > 0 || outputs.length === 0) {
          const message = result.errors.map((error) => error.text).join('\n') || 'no output';
          onEvent({ kind: 'failed', message });
          return;
        }
        const current = manifest;
        installing = installing.then(async () => {
          try {
            let bytes = 0;
            for (const output of outputs) {
              // A site-scripts extension's scripts land where esbuild names them.
              const outfile = isSite(current) ? output.path : join(dir, current.entry);
              await mkdir(dirname(outfile), { recursive: true });
              await writeFile(outfile, output.text);
              bytes += output.contents.length;
            }
            const where =
              'into' in target
                ? await installInto(dir, current, target.into)
                : (await installAndroid(dir, current, { serial: target.serial }),
                  `${ANDROID_PACKAGE}:extensions/${current.id}`);
            onEvent({ kind: 'installed', manifest: current, bytes, where });
          } catch (error) {
            onEvent({
              kind: 'failed',
              message: error instanceof Error ? error.message : String(error),
            });
          }
        });
      });
    },
  };

  const builds = isSite(manifest) ? siteBuilds(dir, manifest) : [];
  if (isSite(manifest) && builds.length === 0) {
    throw new Error(`${manifest.id} builds nothing from src/: install it with raygun-ext pack`);
  }
  const options = isSite(manifest)
    ? siteBuildOptions(dir, manifest, builds)
    : buildOptions(dir, manifest);
  const context = await esbuild.context({ ...options, plugins: [install] });
  await context.watch();
  // esbuild only watches what it bundles; the manifest (permissions, version) matters too.
  const manifestWatcher = watch(join(dir, MANIFEST_FILE), async () => {
    try {
      manifest = await readManifest(dir);
      await context.rebuild();
    } catch (error) {
      onEvent({ kind: 'failed', message: error instanceof Error ? error.message : String(error) });
    }
  });
  return {
    stop: async () => {
      manifestWatcher.close();
      await context.dispose();
      await installing;
    },
  };
};

/** The template folder of a provider kind: `manga.source` → `templates/manga-source`. */
const templateDir = (/** @type {string} */ kind) => join(TEMPLATES_DIR, kind.replace(/\./g, '-'));

/**
 * How a new extension in `dir` depends on the SDK: through the workspace inside the Raygun
 * checkout's `extensions/` folder, else by a link to this checkout's SDK (it isn't published).
 * @param {string} dir
 */
export const sdkDependency = (dir) => {
  const inWorkspace = relative(join(RAYGUN_ROOT, 'extensions'), resolve(dir));
  if (inWorkspace !== '' && !inWorkspace.startsWith('..') && !inWorkspace.includes(sep)) {
    return 'workspace:*';
  }
  return `link:${relative(resolve(dir), SDK_DIR).split(sep).join('/')}`;
};

/**
 * Creates a new extension from the template of `kind`: a working source for a legal site, with
 * its manifest, sources and package files.
 * @param {string} dir
 * @param {{ id: string, name: string, kind?: string }} options
 */
export const init = async (dir, { id, name, kind: asked = DEFAULT_KIND }) => {
  // A template folder's name names its kind too: `site-scripts` is `site.scripts`.
  const kind = asked in KINDS ? asked : asked.replace(/-/g, '.');
  if (!(kind in KINDS)) {
    throw new Error(
      `there's no template for ${kind}; pick one of ${Object.keys(KINDS).join(', ')}`,
    );
  }
  const existing = await readdir(dir).catch(() => []);
  if (existing.length > 0) {
    throw new Error(`${dir} is not empty`);
  }
  await cp(join(TEMPLATES_DIR, 'common'), dir, { recursive: true });
  await cp(templateDir(kind), dir, { recursive: true });
  const values = { __ID__: id, __NAME__: name, __SDK__: sdkDependency(dir) };
  for (const file of [MANIFEST_FILE, 'package.json', SOURCE_ENTRY]) {
    const path = join(dir, file);
    await writeFile(path, fillTemplate(file, await readFile(path, 'utf8'), values));
  }
  const manifest = await readManifest(dir);
  return manifest;
};

/**
 * A template file with its placeholders filled: JSON-escaped in JSON files; in the source, the
 * name as a string literal where it is one (`'__NAME__'`) and as words in comments.
 * @param {string} file
 * @param {string} text
 * @param {{ __ID__: string, __NAME__: string, __SDK__: string }} values
 */
const fillTemplate = (file, text, values) => {
  if (file.endsWith('.json')) {
    return Object.entries(values).reduce(
      (filled, [placeholder, value]) =>
        filled.replaceAll(placeholder, JSON.stringify(value).slice(1, -1)),
      text,
    );
  }
  const literal = `'${values.__NAME__.replace(/\\/g, '\\\\').replace(/'/g, "\\'")}'`;
  return text
    .replaceAll("'__NAME__'", literal)
    .replaceAll('__NAME__', values.__NAME__.replace(/\s+/g, ' '))
    .replaceAll('__ID__', values.__ID__);
};

/**
 * @typedef {{ manifest: Manifest | null, errors: string[], warnings: string[],
 *   providers: import('./check.mjs').FoundProvider[], bytes: number, sites?: string[] }} Validation
 */

/**
 * Checks an extension as Raygun would load it, without the app: the manifest (every rule the app
 * applies), a build of the sources, their types against the provider contracts (`types`), the
 * providers the script registers and the calls each has, the hosts the code fetches from against
 * `permissions.net`, and browser or Node APIs QuickJS lacks. `errors` make Raygun refuse the
 * extension or a call; `warnings` are likely mistakes.
 * @param {string} dir
 * @param {{ types?: boolean }} [options]
 * @returns {Promise<Validation>}
 */
export const validate = async (dir, { types = true } = {}) => {
  const path = join(dir, MANIFEST_FILE);
  let raw;
  try {
    raw = JSON.parse(await readFile(path, 'utf8'));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return {
      manifest: null,
      errors: [`can't read ${path}: ${message}`],
      warnings: [],
      providers: [],
      bytes: 0,
    };
  }
  const { errors, warnings } = checkManifest(raw);
  if (errors.length > 0) {
    return { manifest: null, errors, warnings, providers: [], bytes: 0 };
  }
  /** @type {Manifest} */
  const manifest = raw;
  if (isSite(manifest)) return validateSite(dir, manifest, { errors, warnings, types });
  let code;
  try {
    code = (await bundle(dir)).code;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { manifest, errors: [`the build fails: ${message}`], warnings, providers: [], bytes: 0 };
  }
  const bytes = Buffer.byteLength(code);
  if (bytes > MAX_PACKAGE_BYTES) {
    errors.push(`the bundle is ${bytes} bytes; Raygun takes packages of 10 MB at most`);
  }
  if (types) {
    const problems = await typeProblems(dir);
    if (problems === null) {
      warnings.push("TypeScript isn't installed in this Raygun checkout: types weren't checked");
    }
    errors.push(...(problems ?? []).map((problem) => `type error: ${problem}`));
  }
  const checked = await checkBundle(manifest, code);
  return {
    manifest,
    errors: [...errors, ...checked.errors],
    warnings: [...warnings, ...checked.warnings],
    providers: checked.providers,
    bytes,
  };
};

/**
 * A site-scripts extension checked as Raygun would install it: the manifest, a build of its
 * scripts, its files there, their types, and scripts that reach for the host API pages don't
 * have.
 * @param {string} dir
 * @param {Manifest} manifest
 * @param {{ errors: string[], warnings: string[], types: boolean }} checked
 * @returns {Promise<Validation>}
 */
const validateSite = async (dir, manifest, { errors, warnings, types }) => {
  const sites = sitePatterns(/** @type {Record<string, unknown>} */ (manifest));
  let files;
  try {
    files = await packageFiles(dir, manifest, (await bundle(dir)).files);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return {
      manifest,
      errors: [`the build fails: ${message}`],
      warnings,
      providers: [],
      bytes: 0,
      sites,
    };
  }
  const bytes = Object.values(files).reduce((sum, code) => sum + Buffer.byteLength(code), 0);
  if (bytes > MAX_PACKAGE_BYTES) {
    errors.push(
      `the scripts and styles are ${bytes} bytes; Raygun takes packages of 10 MB at most`,
    );
  }
  if (types) {
    const problems = await typeProblems(dir);
    if (problems === null) {
      warnings.push("TypeScript isn't installed in this Raygun checkout: types weren't checked");
    }
    errors.push(...(problems ?? []).map((problem) => `type error: ${problem}`));
  }
  for (const [path, code] of Object.entries(files)) {
    if (path.endsWith('.js') && /\braygun\.(http|providers|storage|settings|log)\b/.test(code)) {
      warnings.push(
        `${path} reaches for raygun.*, the host API of extensions that run in Raygun; site scripts run in pages and get raygunPage`,
      );
    }
  }
  if (sites.some((pattern) => /^(\*|https?):\/\/\*\//.test(pattern))) {
    warnings.push('its scripts run on every site; people are told so before they install it');
  }
  return { manifest, errors, warnings, providers: [], bytes, sites };
};

/**
 * Arguments for a provider call from `key=value` words, over the ones the app passes that call
 * (`kinds.json`). A value is read as JSON when it can be (`page=2`, `artist=null`), else as text.
 * @param {string} kind
 * @param {string} method
 * @param {string[]} words
 */
export const callArgs = (kind, method, words) => {
  if (words.length === 1 && /^\s*[{[]/.test(words[0] ?? '')) {
    return JSON.parse(words[0] ?? '');
  }
  /** @type {Record<string, unknown>} */
  const args = { ...(KINDS[kind]?.args[method] ?? {}) };
  for (const word of words) {
    const equals = word.indexOf('=');
    if (equals < 1) {
      throw new Error(`"${word}": give arguments as key=value, or as one JSON object`);
    }
    const value = word.slice(equals + 1);
    try {
      args[word.slice(0, equals)] = JSON.parse(value);
    } catch {
      args[word.slice(0, equals)] = value;
    }
  }
  return args;
};

/**
 * @typedef {{ provider?: string, method?: string, args?: unknown, settings?: string[],
 *   nsfw?: boolean, data?: string, address?: string, bundle?: string }} TestOptions
 */

/**
 * The command that loads an extension folder into Raygun's own extension host and calls a
 * provider (`crates/raygun-ext-host/examples/call.rs`), with cargo in this checkout.
 * @param {string} dir
 * @param {TestOptions} [options]
 */
export const testCommand = (
  dir,
  { provider, method, args, settings = [], nsfw = false, data, address, bundle: bundleOut } = {},
) => ({
  command: 'cargo',
  args: [
    'run',
    '--quiet',
    '--manifest-path',
    join(RAYGUN_ROOT, 'Cargo.toml'),
    '-p',
    'raygun-ext-host',
    '--example',
    'call',
    '--',
    resolve(dir),
    // A site-scripts extension is tried on a page's address: what runs there.
    ...(address ? [address] : []),
    ...(address && bundleOut ? ['--bundle', resolve(bundleOut)] : []),
    ...(provider && method ? [provider, method] : []),
    ...(provider && method && args !== undefined ? [JSON.stringify(args)] : []),
    ...settings.flatMap((setting) => ['--set', setting]),
    ...(nsfw ? ['--nsfw'] : []),
    ...(data ? ['--data', resolve(data)] : []),
  ],
});

/**
 * Builds an extension, then runs one provider call in Raygun's extension host: the answer as
 * JSON, what Raygun keeps of it, and how long it took against the time the app gives the call.
 * Without a provider it lists the providers the extension registers. Resolves to the exit code.
 * @param {string} dir
 * @param {Omit<TestOptions, 'args'> & { words?: string[] }} [options]
 * @returns {Promise<number>}
 */
export const runTest = async (dir, { words = [], ...options } = {}) => {
  if (!existsSync(join(RAYGUN_ROOT, 'Cargo.toml'))) {
    throw new Error(
      `raygun-ext test runs Raygun's extension host from a Raygun checkout; ${RAYGUN_ROOT} isn't one`,
    );
  }
  await build(dir);
  let args;
  if (options.provider && options.method) {
    // The provider's kind says what the app passes the call; types don't matter here.
    const found = (await validate(dir, { types: false })).providers.find(
      (provider) => provider.id === options.provider?.split('/').pop(),
    );
    args = callArgs(found?.kind ?? '', options.method, words);
  }
  const { command, args: commandArgs } = testCommand(dir, { ...options, args });
  return new Promise((resolvePromise, reject) => {
    const child = spawn(command, commandArgs, { stdio: 'inherit' });
    child.on('error', (error) =>
      reject(
        /** @type {NodeJS.ErrnoException} */ (error).code === 'ENOENT'
          ? new Error(
              "raygun-ext test needs cargo, Rust's build tool: install it from https://rustup.rs",
            )
          : error,
      ),
    );
    child.on('close', (code) => resolvePromise(code ?? 1));
  });
};

// ---- Repositories (M13) ----

export const INDEX_FILE = 'index.json';
// Zip timestamps are local DOS times; a local-time date gives the same bytes on every machine.
const PACKAGE_TIME = new Date(2000, 0, 1);

/**
 * Semver ordering (major.minor.patch, then a pre-release sorts before its release).
 * @param {string} a
 * @param {string} b
 */
export const compareVersions = (a, b) => {
  /** @param {string} version */
  const parse = (version) => {
    const [core = '', pre] = version.split('+')[0]?.split(/-(.*)/s) ?? [];
    return { parts: core.split('.').map(Number), pre };
  };
  const left = parse(a);
  const right = parse(b);
  for (let index = 0; index < 3; index += 1) {
    const difference = (left.parts[index] ?? 0) - (right.parts[index] ?? 0);
    if (difference !== 0) return Math.sign(difference);
  }
  if (left.pre === right.pre) return 0;
  if (left.pre === undefined) return 1;
  if (right.pre === undefined) return -1;
  return left.pre < right.pre ? -1 : 1;
};

/**
 * Whether a published package holds the same files as a new one: the same code, and a manifest
 * with the same contents (its whitespace aside).
 * @param {Uint8Array} published
 * @param {Record<string, Uint8Array>} files
 */
const samePackage = (published, files) => {
  let unpacked;
  try {
    unpacked = unzipSync(published);
  } catch {
    return false;
  }
  const names = Object.keys(files);
  if (Object.keys(unpacked).sort().join('\n') !== [...names].sort().join('\n')) return false;
  return names.every((name) => {
    const before = unpacked[name];
    const after = files[name];
    if (!before || !after) return false;
    if (name !== MANIFEST_FILE) return Buffer.from(before).equals(Buffer.from(after));
    try {
      const decode = (/** @type {Uint8Array} */ bytes) =>
        JSON.parse(new TextDecoder().decode(bytes));
      return isDeepStrictEqual(decode(before), decode(after));
    } catch {
      return false;
    }
  });
};

/**
 * Builds an extension and adds its package to a repository folder, leaving the index alone.
 * `kept` is set when this version was already published with other contents.
 * @param {string} dir
 * @param {string} repoDir
 */
const packPackage = async (dir, repoDir) => {
  // A package Raygun would refuse, or whose calls would fail, is never published.
  const { errors } = await validate(dir);
  if (errors.length > 0) {
    throw new Error(`${dir} isn't ready to publish:\n  - ${errors.join('\n  - ')}`);
  }
  const { manifest, outfile } = await build(dir);
  /** @type {Record<string, Uint8Array>} */
  const files = { [MANIFEST_FILE]: await readFile(join(dir, MANIFEST_FILE)) };
  if (isSite(manifest)) {
    // Its scripts and styles, in the order the manifest names them.
    for (const path of Object.values(
      siteFiles(/** @type {Record<string, unknown>} */ (manifest)),
    ).flat()) {
      files[path] = await readFile(join(dir, path));
    }
  } else {
    files[manifest.entry] = await readFile(outfile);
  }
  /** @type {import('fflate').Zippable} */
  const zippable = Object.fromEntries(
    Object.entries(files).map(([name, bytes]) => [name, [bytes, { mtime: PACKAGE_TIME }]]),
  );
  const archive = zipSync(zippable, { level: 9, mtime: PACKAGE_TIME });
  const name = `${manifest.id}-${manifest.version}.zip`;
  const target = join(repoDir, 'packages', name);
  await mkdir(join(repoDir, 'packages'), { recursive: true });
  // A published version never changes: installs check its checksum. Changed code needs a new
  // version, so an existing package with other contents is kept and the author warned. One that
  // differs only in how its manifest is laid out is the same package.
  const published = await readFile(target).catch(() => null);
  const kept =
    published !== null &&
    !Buffer.from(published).equals(Buffer.from(archive)) &&
    !samePackage(published, files);
  if (kept) {
    console.warn(
      `${manifest.id} ${manifest.version} is already published with other contents; ` +
        'kept it. Bump the version to publish the changes.',
    );
  } else if (!published) {
    await writeFile(target, archive);
  }
  await copyIcon(dir, repoDir, manifest.id);
  return { manifest, package: `packages/${name}`, bytes: archive.length, kept };
};

/** Picture types an extension's icon may be, in the order they're looked for. */
const ICON_TYPES = ['png', 'webp', 'jpg'];

/**
 * Copies an extension's `icon.png` (or `.webp`, `.jpg`) to the repository's `icons/<id>.<type>`,
 * which the index then names. Icons aren't part of packages, so one can change without a version.
 * @param {string} dir
 * @param {string} repoDir
 * @param {string} id
 */
const copyIcon = async (dir, repoDir, id) => {
  for (const type of ICON_TYPES) {
    const icon = join(dir, `icon.${type}`);
    if (!existsSync(icon)) continue;
    await mkdir(join(repoDir, 'icons'), { recursive: true });
    // One icon per extension: another type left from before would be listed instead.
    for (const other of ICON_TYPES)
      await rm(join(repoDir, 'icons', `${id}.${other}`), { force: true });
    await cp(icon, join(repoDir, 'icons', `${id}.${type}`));
    return;
  }
};

/** The icon the repository holds for an extension, relative to its index. */
const iconOf = (/** @type {string} */ repoDir, /** @type {string} */ id) =>
  ICON_TYPES.map((type) => `icons/${id}.${type}`).find((path) => existsSync(join(repoDir, path)));

/** @typedef {{ name?: string }} IndexOptions */

/**
 * Builds an extension and adds its package to a repository folder, then rewrites the index.
 * Packages are reproducible: the same sources give the same bytes (and checksum).
 * @param {string} dir
 * @param {string} repoDir
 * @param {IndexOptions} [options]
 */
export const pack = async (dir, repoDir, options = {}) => {
  const packed = await packPackage(dir, repoDir);
  const index = await writeIndex(repoDir, options);
  return { ...packed, index };
};

/**
 * Packs every extension in a folder (each subfolder with a manifest) into a repository folder,
 * one at a time, then rewrites the index once. A new extension is published with the rest; it
 * needs no script of its own.
 * @param {string} parentDir
 * @param {string} repoDir
 * @param {IndexOptions} [options]
 */
export const packAll = async (parentDir, repoDir, options = {}) => {
  const folders = (await readdir(parentDir, { withFileTypes: true }))
    .filter((entry) => entry.isDirectory() && !entry.name.startsWith('.'))
    .map((entry) => join(parentDir, entry.name))
    .sort();
  const packed = [];
  for (const dir of folders) {
    const hasManifest = await readFile(join(dir, MANIFEST_FILE)).then(
      () => true,
      () => false,
    );
    if (hasManifest) packed.push(await packPackage(dir, repoDir));
  }
  if (packed.length === 0) throw new Error(`${parentDir} has no extensions`);
  const index = await writeIndex(repoDir, options);
  return { packed, index };
};

/**
 * Rewrites `index.json` from the packages in a repository folder, listing the newest version of
 * each extension. Older packages stay on disk for anyone who linked them. The index is named
 * `options.name` when given; otherwise it keeps its name, and a new one is named after its
 * folder rather than as someone else's repository.
 * @param {string} repoDir
 * @param {IndexOptions} [options]
 */
export const writeIndex = async (repoDir, options = {}) => {
  const indexPath = join(repoDir, INDEX_FILE);
  const existing = await readFile(indexPath, 'utf8')
    .then((text) => JSON.parse(text))
    .catch(() => null);
  const packagesDir = join(repoDir, 'packages');
  const names = (await readdir(packagesDir).catch(() => [])).filter((name) =>
    name.endsWith('.zip'),
  );

  /** @type {Map<string, Record<string, unknown> & { id: string, version: string }>} */
  const newest = new Map();
  for (const name of names.sort()) {
    const bytes = await readFile(join(packagesDir, name));
    const manifestBytes = unzipSync(bytes, { filter: (file) => file.name === MANIFEST_FILE })[
      MANIFEST_FILE
    ];
    if (!manifestBytes) throw new Error(`${name} has no ${MANIFEST_FILE}`);
    /** @type {Manifest} */
    const manifest = JSON.parse(new TextDecoder().decode(manifestBytes));
    const icon = iconOf(repoDir, manifest.id);
    // Where a site-scripts extension's scripts run, checked against the package at install.
    const sites = sitePatterns(/** @type {Record<string, unknown>} */ (manifest));
    const entry = {
      id: manifest.id,
      name: manifest.name,
      version: manifest.version,
      apiVersion: manifest.apiVersion,
      description: manifest.description ?? null,
      author: manifest.author ?? null,
      provides: manifest.provides,
      permissions: {
        net: manifest.permissions?.net ?? [],
        webview: manifest.permissions?.webview ?? false,
      },
      contentRating: manifest.contentRating ?? 'safe',
      ...(sites.length > 0 ? { sites } : {}),
      package: `packages/${name}`,
      ...(icon ? { icon } : {}),
      sha256: createHash('sha256').update(bytes).digest('hex'),
      size: bytes.length,
    };
    const current = newest.get(manifest.id);
    if (!current || compareVersions(manifest.version, current.version) > 0) {
      newest.set(manifest.id, entry);
    }
  }

  const index = {
    formatVersion: 1,
    name: options.name ?? existing?.name ?? basename(resolve(repoDir)),
    extensions: [...newest.values()].sort((a, b) => a.id.localeCompare(b.id)),
  };
  await writeFile(
    indexPath,
    `${JSON.stringify(index, null, 2)}
`,
  );
  return index;
};
