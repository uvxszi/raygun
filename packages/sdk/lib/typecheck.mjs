// Type-checks an extension's sources against this SDK's provider contracts, with the TypeScript of
// the Raygun checkout the SDK sits in, so an extension outside the checkout needs no TypeScript of
// its own. The extension's own tsconfig.json is used when it has one.

import { join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const SDK_INDEX = fileURLToPath(new URL('../src/index.ts', import.meta.url));

/** What the templates' tsconfig.json sets: QuickJS's ES2022 and nothing of a browser or Node. */
const DEFAULTS = {
  target: 'ES2022',
  lib: ['ES2022'],
  types: [],
  module: 'ESNext',
  moduleResolution: 'bundler',
  strict: true,
  noUncheckedIndexedAccess: true,
  isolatedModules: true,
  verbatimModuleSyntax: true,
  skipLibCheck: true,
  noEmit: true,
};

/**
 * Type errors in an extension's sources, as `file:line:column message`; `null` when TypeScript
 * can't be loaded.
 * @param {string} dir
 * @returns {Promise<string[] | null>}
 */
export const typeProblems = async (dir) => {
  /** @type {typeof import('typescript')} */
  let ts;
  try {
    ts = (await import('typescript')).default;
  } catch {
    return null;
  }
  const root = resolve(dir);
  const configPath = join(root, 'tsconfig.json');
  const config = ts.sys.fileExists(configPath)
    ? ts.readConfigFile(configPath, ts.sys.readFile).config
    : { compilerOptions: DEFAULTS, include: ['src'] };
  const parsed = ts.parseJsonConfigFileContent(config ?? {}, ts.sys, root);
  const options = {
    ...parsed.options,
    noEmit: true,
    // The contracts this SDK checks against, wherever the extension's own copy comes from.
    paths: { ...parsed.options.paths, '@raygun/sdk': [SDK_INDEX] },
  };
  const program = ts.createProgram(
    parsed.fileNames.length > 0 ? parsed.fileNames : [join(root, 'src', 'index.ts')],
    options,
  );
  return ts.getPreEmitDiagnostics(program).map((diagnostic) => {
    // A chain of reasons reads one under the other.
    const message = ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n    ');
    if (!diagnostic.file || diagnostic.start === undefined) return message;
    const { line, character } = diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start);
    const file = relative(root, diagnostic.file.fileName).split('\\').join('/');
    return `${file.startsWith('..') ? diagnostic.file.fileName : file}:${line + 1}:${character + 1} ${message}`;
  });
};
