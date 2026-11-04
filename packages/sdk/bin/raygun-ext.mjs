#!/usr/bin/env node
// raygun-ext — create, check, try and build Raygun extensions, install them into a running app
// and pack them into repositories. The guide is docs/extensions in the Raygun repository.
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { parseArgs } from 'node:util';

import {
  DEFAULT_KIND,
  KINDS,
  MANIFEST_FILE,
  build,
  dev,
  init,
  pack,
  packAll,
  runTest,
  validate,
} from '../lib/cli.mjs';

const KIND_LIST = Object.entries(KINDS)
  .map(([kind, info]) => `  ${kind.padEnd(20)} ${info.label}`)
  .join('\n');

const USAGE = `Usage:
  raygun-ext init <dir> --id <reverse.dns.id> --name <name> [--kind <kind>]
  raygun-ext build [dir]
  raygun-ext validate [dir]
  raygun-ext test [dir] [<provider> <method> [key=value... | <JSON>]] [--set key=value]... [--nsfw] [--data <dir>]
  raygun-ext dev [dir] --android [--serial <device>]
  raygun-ext dev [dir] --into <extensions folder>
  raygun-ext pack [dir] --repo <repository folder> [--name <repository name>]
  raygun-ext pack-all <folder of extensions> --repo <repository folder> [--name <repository name>]

Kinds (init --kind, ${DEFAULT_KIND} without one):
${KIND_LIST}`;

const { positionals, values } = parseArgs({
  allowPositionals: true,
  options: {
    id: { type: 'string' },
    name: { type: 'string' },
    kind: { type: 'string' },
    android: { type: 'boolean' },
    serial: { type: 'string' },
    into: { type: 'string' },
    repo: { type: 'string' },
    set: { type: 'string', multiple: true },
    nsfw: { type: 'boolean' },
    data: { type: 'string' },
    help: { type: 'boolean', short: 'h' },
  },
});
const [command, dir = '.'] = positionals;
const time = () => new Date().toLocaleTimeString();
/** @param {number} count */
const extensions = (count) => `${count} ${count === 1 ? 'extension' : 'extensions'}`;

/**
 * `test`'s folder is optional: `raygun-ext test books popular` runs in the current folder, and a
 * first word naming a folder with a manifest is the extension's.
 * @param {string[]} words
 */
const testWords = (words) => {
  const [first, ...rest] = words;
  return first !== undefined && existsSync(join(first, MANIFEST_FILE))
    ? { dir: first, rest }
    : { dir: '.', rest: words };
};

const main = async () => {
  switch (command) {
    case 'init': {
      if (!values.id || !values.name || positionals.length < 2) throw new Error(USAGE);
      const manifest = await init(dir, { id: values.id, name: values.name, kind: values.kind });
      console.log(
        `Created ${manifest.id} (${manifest.provides.join(', ')}) in ${dir}. Next: pnpm install, then pnpm build.`,
      );
      return;
    }
    case 'build': {
      const { manifest, outfile, bytes } = await build(dir);
      console.log(`Built ${manifest.id} ${manifest.version} → ${outfile} (${bytes} bytes)`);
      return;
    }
    case 'validate': {
      const { manifest, errors, warnings, providers } = await validate(dir);
      for (const provider of providers) {
        console.log(`${provider.kind} ${provider.id}: ${provider.methods.join(', ')}`);
      }
      for (const warning of warnings) console.warn(`warning: ${warning}`);
      for (const error of errors) console.error(`error: ${error}`);
      if (errors.length > 0) {
        process.exitCode = 1;
      } else if (manifest) {
        console.log(
          `${manifest.id} ${manifest.version} is ready${warnings.length > 0 ? `, with ${warnings.length} warning(s)` : ''}.`,
        );
      }
      return;
    }
    case 'test': {
      const { dir: folder, rest } = testWords(positionals.slice(1));
      const [provider, method, ...words] = rest;
      if (provider && !method) throw new Error(`test needs a method after the provider.\n${USAGE}`);
      process.exitCode = await runTest(folder, {
        provider,
        method,
        words,
        settings: values.set,
        nsfw: values.nsfw,
        data: values.data,
      });
      return;
    }
    case 'pack': {
      if (!values.repo)
        throw new Error(`pack needs --repo.
${USAGE}`);
      const result = await pack(dir, values.repo, { name: values.name });
      console.log(
        `Packed ${result.manifest.id} ${result.manifest.version} → ${result.package} (${result.bytes} bytes); index lists ${extensions(result.index.extensions.length)}`,
      );
      return;
    }
    case 'pack-all': {
      if (!values.repo || positionals.length < 2)
        throw new Error(`pack-all needs a folder of extensions and --repo.
${USAGE}`);
      const { packed, index } = await packAll(dir, values.repo, { name: values.name });
      for (const result of packed) {
        console.log(
          `Packed ${result.manifest.id} ${result.manifest.version} → ${result.package} (${result.bytes} bytes)`,
        );
      }
      const kept = packed.filter((result) => result.kept);
      if (kept.length > 0) {
        console.warn(
          `Kept the published copies of ${kept.map((result) => `${result.manifest.id} ${result.manifest.version}`).join(', ')}: bump their versions to publish their changes.`,
        );
      }
      console.log(`Index lists ${extensions(index.extensions.length)}`);
      return;
    }
    case 'dev': {
      const target = values.into
        ? { into: values.into }
        : values.android
          ? { android: /** @type {const} */ (true), serial: values.serial }
          : null;
      if (!target) throw new Error(`dev needs --android or --into.\n${USAGE}`);
      const session = await dev(dir, target, (event) => {
        if (event.kind === 'installed') {
          console.log(
            `[${time()}] installed ${event.manifest.id} ${event.manifest.version} → ${event.where} (${event.bytes} bytes)`,
          );
        } else {
          console.error(`[${time()}] build failed:\n${event.message}`);
        }
      });
      console.log('Watching for changes; Ctrl-C to stop.');
      process.once('SIGINT', () => void session.stop().then(() => process.exit(0)));
      return;
    }
    default:
      console.log(USAGE);
      if (!values.help) process.exitCode = 1;
  }
};

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
});
