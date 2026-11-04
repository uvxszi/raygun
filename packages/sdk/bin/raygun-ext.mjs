#!/usr/bin/env node
// raygun-ext — build Raygun extensions and install them into a running app.
import { parseArgs } from 'node:util';

import { build, dev, init, pack, packAll } from '../lib/cli.mjs';

const USAGE = `Usage:
  raygun-ext init <dir> --id <reverse.dns.id> --name <name>
  raygun-ext build [dir]
  raygun-ext pack [dir] --repo <repository folder> [--name <repository name>]
  raygun-ext pack-all <folder of extensions> --repo <repository folder> [--name <repository name>]
  raygun-ext dev [dir] --android [--serial <device>]
  raygun-ext dev [dir] --into <extensions folder>`;

const { positionals, values } = parseArgs({
  allowPositionals: true,
  options: {
    id: { type: 'string' },
    name: { type: 'string' },
    android: { type: 'boolean' },
    serial: { type: 'string' },
    into: { type: 'string' },
    repo: { type: 'string' },
    help: { type: 'boolean', short: 'h' },
  },
});
const [command, dir = '.'] = positionals;
const time = () => new Date().toLocaleTimeString();
/** @param {number} count */
const extensions = (count) => `${count} ${count === 1 ? 'extension' : 'extensions'}`;

const main = async () => {
  switch (command) {
    case 'init': {
      if (!values.id || !values.name || positionals.length < 2) throw new Error(USAGE);
      const manifest = await init(dir, { id: values.id, name: values.name });
      console.log(`Created ${manifest.id} in ${dir}. Next: pnpm install, then raygun-ext build.`);
      return;
    }
    case 'build': {
      const { manifest, outfile, bytes } = await build(dir);
      console.log(`Built ${manifest.id} ${manifest.version} → ${outfile} (${bytes} bytes)`);
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
