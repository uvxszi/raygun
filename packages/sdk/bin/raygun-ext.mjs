#!/usr/bin/env node
// raygun-ext — build Raygun extensions and install them into a running app.
import { parseArgs } from 'node:util';

import { build, dev, init } from '../lib/cli.mjs';

const USAGE = `Usage:
  raygun-ext init <dir> --id <reverse.dns.id> --name <name>
  raygun-ext build [dir]
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
    help: { type: 'boolean', short: 'h' },
  },
});
const [command, dir = '.'] = positionals;
const time = () => new Date().toLocaleTimeString();

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
