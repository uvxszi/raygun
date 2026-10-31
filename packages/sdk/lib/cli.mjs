// raygun-ext: builds Raygun extensions and installs them into a running app (M12).
// Plain ESM with JSDoc types, so it runs on Node without a build step; tsc checks it (checkJs).

import { spawn } from 'node:child_process';
import { watch } from 'node:fs';
import { cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import * as esbuild from 'esbuild';

export const MANIFEST_FILE = 'raygun-extension.json';
export const SOURCE_ENTRY = 'src/index.ts';
export const ANDROID_PACKAGE = 'app.raygun.player';
const TEMPLATE_DIR = fileURLToPath(new URL('../template', import.meta.url));

/** @typedef {import('../src/types').Manifest} Manifest */

const ID_PATTERN = /^[a-z0-9][a-z0-9-]*(\.[a-z0-9][a-z0-9-]*)+$/;
const SEMVER_PATTERN = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/;

/**
 * Problems with a manifest, mirroring the host's checks (crates/raygun-ext-host/src/manifest.rs)
 * closely enough to catch mistakes before the app does.
 * @param {Record<string, unknown>} manifest
 * @returns {string[]}
 */
export const manifestProblems = (manifest) => {
  const problems = [];
  const { id, name, version, apiVersion, runtime, entry, provides } = manifest;
  if (typeof id !== 'string' || !ID_PATTERN.test(id) || id.length > 100) {
    problems.push('id must be reverse-DNS in lower case, like dev.example.my-source');
  }
  if (typeof name !== 'string' || name.trim() === '' || name.length > 80) {
    problems.push('name must be 1-80 characters');
  }
  if (typeof version !== 'string' || !SEMVER_PATTERN.test(version)) {
    problems.push('version must be semver, like 1.0.0');
  }
  if (apiVersion !== 1) problems.push('apiVersion must be 1');
  if (runtime !== 'js') problems.push('runtime must be "js"');
  if (
    typeof entry !== 'string' ||
    !entry.endsWith('.js') ||
    entry.includes('\\') ||
    entry.split('/').some((part) => part === '' || part === '.' || part === '..')
  ) {
    problems.push('entry must be a relative path to a .js file, like dist/index.js');
  }
  if (!Array.isArray(provides) || provides.length === 0) {
    problems.push('provides must list at least one provider kind, like "music.metadata"');
  }
  return problems;
};

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
 * script with no module loader.
 * @param {string} dir
 * @param {Manifest} manifest
 * @returns {esbuild.BuildOptions & { write: false }}
 */
const buildOptions = (dir, manifest) => ({
  absWorkingDir: resolve(dir),
  entryPoints: [SOURCE_ENTRY],
  outfile: manifest.entry,
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

/**
 * Bundles an extension without writing it.
 * @param {string} dir
 * @returns {Promise<{ manifest: Manifest, code: string }>}
 */
export const bundle = async (dir) => {
  const manifest = await readManifest(dir);
  const result = await esbuild.build(buildOptions(dir, manifest));
  const output = result.outputFiles[0];
  if (!output) throw new Error('esbuild produced no output');
  return { manifest, code: output.text };
};

/**
 * Bundles an extension into its manifest's entry.
 * @param {string} dir
 * @returns {Promise<{ manifest: Manifest, outfile: string, bytes: number }>}
 */
export const build = async (dir) => {
  const { manifest, code } = await bundle(dir);
  const outfile = join(dir, manifest.entry);
  await mkdir(dirname(outfile), { recursive: true });
  await writeFile(outfile, code);
  return { manifest, outfile, bytes: Buffer.byteLength(code) };
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
  await mkdir(dirname(join(staging, manifest.entry)), { recursive: true });
  await cp(join(dir, MANIFEST_FILE), join(staging, MANIFEST_FILE));
  await cp(join(dir, manifest.entry), join(staging, manifest.entry));
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
        const output = result.outputFiles?.[0];
        if (result.errors.length > 0 || !output) {
          const message = result.errors.map((error) => error.text).join('\n') || 'no output';
          onEvent({ kind: 'failed', message });
          return;
        }
        const current = manifest;
        installing = installing.then(async () => {
          try {
            const outfile = join(dir, current.entry);
            await mkdir(dirname(outfile), { recursive: true });
            await writeFile(outfile, output.text);
            const where =
              'into' in target
                ? await installInto(dir, current, target.into)
                : (await installAndroid(dir, current, { serial: target.serial }),
                  `${ANDROID_PACKAGE}:extensions/${current.id}`);
            onEvent({ kind: 'installed', manifest: current, bytes: output.contents.length, where });
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

  const context = await esbuild.context({ ...buildOptions(dir, manifest), plugins: [install] });
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

/**
 * Creates a new extension from the template.
 * @param {string} dir
 * @param {{ id: string, name: string }} options
 */
export const init = async (dir, { id, name }) => {
  const existing = await readdir(dir).catch(() => []);
  if (existing.length > 0) {
    throw new Error(`${dir} is not empty`);
  }
  await cp(TEMPLATE_DIR, dir, { recursive: true });
  for (const file of [MANIFEST_FILE, 'package.json', SOURCE_ENTRY]) {
    const path = join(dir, file);
    const text = await readFile(path, 'utf8');
    await writeFile(path, text.replaceAll('__ID__', id).replaceAll('__NAME__', name));
  }
  const manifest = await readManifest(dir);
  return manifest;
};
