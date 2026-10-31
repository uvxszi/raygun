// Builds the Nuclear converter Raygun embeds: first the shim (one IIFE defining `__nuclearShim`),
// then the converter with that shim inlined as text. Output is committed under the ext-host crate
// so `cargo build` never needs Node; a test fails when it's stale.
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import * as esbuild from 'esbuild';

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
export const OUTPUT = join(
  ROOT,
  '..',
  '..',
  'crates',
  'raygun-ext-host',
  'assets',
  'nuclear-converter.js',
);

/** @type {{ bundle: true, write: false, platform: 'neutral', target: string, logLevel: 'silent' }} */
const common = {
  bundle: true,
  write: false,
  platform: 'neutral',
  target: 'es2022',
  logLevel: 'silent',
};

/** @param {esbuild.BuildResult<{ write: false }>} result */
const textOf = (result) => {
  const text = result.outputFiles?.[0]?.text;
  if (text === undefined) throw new Error('esbuild produced no output');
  return text;
};

/** The shim, as the text that goes in front of every converted plugin. */
export const buildShim = async () => {
  const result = await esbuild.build({
    ...common,
    entryPoints: [join(ROOT, 'src/shim/index.ts')],
    format: 'iife',
    globalName: '__nuclearShim',
    mainFields: ['module', 'main'],
    legalComments: 'none',
  });
  return textOf(result);
};

/** The converter script, with the shim inlined. */
export const buildConverter = async () => {
  const shim = await buildShim();
  const result = await esbuild.build({
    ...common,
    entryPoints: [join(ROOT, 'src/entry.ts')],
    format: 'iife',
    mainFields: ['module', 'main'],
    minify: true,
    legalComments: 'inline',
    banner: {
      js: '// Raygun Nuclear plugin converter. Built by packages/nuclear-compat (pnpm build); do not edit.',
    },
    plugins: [
      {
        name: 'virtual-shim',
        setup(build) {
          build.onResolve({ filter: /^virtual:shim$/ }, () => ({
            path: 'shim',
            namespace: 'virtual',
          }));
          build.onLoad({ filter: /.*/, namespace: 'virtual' }, () => ({
            contents: `export default ${JSON.stringify(shim)};`,
            loader: 'js',
          }));
        },
      },
    ],
  });
  return textOf(result);
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const code = await buildConverter();
  await mkdir(dirname(OUTPUT), { recursive: true });
  await writeFile(OUTPUT, code);
  console.log(`Built ${OUTPUT} (${code.length} bytes)`);
}
