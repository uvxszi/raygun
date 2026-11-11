// The guide in docs/extensions stays true: each kind's example is what `raygun-ext init` writes,
// every TypeScript snippet compiles against the SDK, every manifest snippet is one Raygun loads,
// and links between the pages lead somewhere.
import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import * as prettier from 'prettier';
import ts from 'typescript';

import { KINDS, MANIFEST_FILE, checkManifest, init } from '../lib/cli.mjs';

const SDK_DIR = dirname(dirname(fileURLToPath(import.meta.url)));
const DOCS = join(SDK_DIR, '..', '..', 'docs', 'extensions');

type Block = { lang: string; code: string; line: number };
type Page = { file: string; text: string; blocks: Block[] };

/** A page's fenced code blocks, list indentation removed. */
const fenced = (text: string): Block[] => {
  const found: Block[] = [];
  const lines = text.split('\n');
  for (let index = 0; index < lines.length; index += 1) {
    const open = /^(\s*)```(\w*)\s*$/.exec(lines[index] ?? '');
    if (!open) continue;
    const [, indent = '', lang = ''] = open;
    const body: string[] = [];
    let end = index + 1;
    while (end < lines.length && !/^\s*```\s*$/.test(lines[end] ?? '')) {
      body.push((lines[end] ?? '').slice(indent.length));
      end += 1;
    }
    found.push({ lang, code: body.join('\n'), line: index + 1 });
    index = end;
  }
  return found;
};

const pages = async (): Promise<Page[]> => {
  const files = (await readdir(DOCS)).filter((file) => file.endsWith('.md')).sort();
  return Promise.all(
    files.map(async (file) => {
      const text = (await readFile(join(DOCS, file), 'utf8')).replaceAll('\r\n', '\n');
      return { file, text, blocks: fenced(text) };
    }),
  );
};

/** As the repository's Prettier writes it, so formatting the docs never breaks the comparison. */
const formatted = async (code: string, parser: 'typescript' | 'json') => {
  const options = (await prettier.resolveConfig(join(DOCS, 'README.md'))) ?? {};
  return prettier.format(code, { ...options, parser });
};

/** GitHub's anchor for a heading. */
const slug = (heading: string) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s-]/gu, '')
    .replace(/\s/g, '-');

let work: string;

beforeEach(async () => {
  work = await mkdtemp(join(tmpdir(), 'raygun-docs-test-'));
});

afterEach(async () => {
  await rm(work, { recursive: true, force: true });
});

describe('the extensions guide', () => {
  it('has a page with an example for every kind', async () => {
    const all = await pages();
    for (const kind of Object.keys(KINDS)) {
      const page = all.find((candidate) => candidate.file === `${kind.replace('.', '-')}.md`);
      expect(page, kind).toBeDefined();
      expect(page?.text, kind).toContain(`<!-- example: ${kind} `);
    }
  });

  it('shows each example as raygun-ext init writes it', async () => {
    const marker =
      /<!-- example: (\S+) (\S+) (.+?) -->\n\n```json\n([\s\S]*?)```\n\n```ts\n([\s\S]*?)```/g;
    let examples = 0;
    for (const page of await pages()) {
      for (const [
        ,
        kind = '',
        id = '',
        name = '',
        manifest = '',
        source = '',
      ] of page.text.matchAll(marker)) {
        examples += 1;
        const dir = join(work, id);
        await init(dir, { id, name, kind });
        const written = await readFile(join(dir, MANIFEST_FILE), 'utf8');
        expect(await formatted(manifest, 'json'), `${page.file} manifest`).toBe(
          await formatted(written, 'json'),
        );
        const code = await readFile(join(dir, 'src', 'index.ts'), 'utf8');
        expect(await formatted(source, 'typescript'), `${page.file} source`).toBe(
          await formatted(code, 'typescript'),
        );
        // Its init command makes the same extension.
        expect(page.text, page.file).toMatch(
          new RegExp(
            `raygun-ext init \\S+ --kind ${kind.replace('.', '\\.')} --id ${id} --name "${name}"`,
          ),
        );
      }
    }
    expect(examples).toBe(Object.keys(KINDS).length);
  });

  it('calls only providers and methods the examples have', async () => {
    for (const page of await pages()) {
      const example = /<!-- example: (\S+) /.exec(page.text);
      const kind = example?.[1];
      if (!kind || !example) continue;
      // The example's provider, not one of the snippets before it.
      const provider = new RegExp(`kind: '${kind.replace('.', '\\.')}',\\s*id: '([^']+)'`).exec(
        page.text.slice(example.index),
      )?.[1];
      const calls = [...(KINDS[kind]?.required ?? []), ...(KINDS[kind]?.optional ?? [])];
      for (const [, id, method] of page.text.matchAll(/raygun-ext test ([\w.-]+) (\w+)/g)) {
        expect(id, `${page.file}: raygun-ext test ${id} ${method}`).toBe(provider);
        expect(calls, `${page.file}: ${method}`).toContain(method);
      }
    }
  });

  it('has TypeScript snippets that compile against the SDK', async () => {
    // Site scripts run in pages: their snippets have the DOM, the others QuickJS's ES2022 alone.
    const files: string[] = [];
    const pageFiles: string[] = [];
    for (const page of await pages()) {
      for (const block of page.blocks.filter((candidate) => candidate.lang === 'ts')) {
        const file = join(work, 'snippets', `${page.file.replace('.md', '')}-${block.line}.ts`);
        await mkdir(dirname(file), { recursive: true });
        // Each snippet is a module of its own, so their names never meet.
        const module = /^\s*(import|export)\s/m.test(block.code) ? '' : '\nexport {};\n';
        await writeFile(file, `${block.code}${module}`);
        (page.file === 'site-scripts.md' ? pageFiles : files).push(file);
      }
    }
    expect(files.length).toBeGreaterThan(20);
    expect(pageFiles.length).toBeGreaterThan(1);
    const problemsOf = (sources: string[], lib: string[]) => {
      const program = ts.createProgram(sources, {
        target: ts.ScriptTarget.ES2022,
        lib,
        types: [],
        module: ts.ModuleKind.ESNext,
        moduleResolution: ts.ModuleResolutionKind.Bundler,
        strict: true,
        noUncheckedIndexedAccess: true,
        isolatedModules: true,
        verbatimModuleSyntax: true,
        skipLibCheck: true,
        noEmit: true,
        paths: {
          '@raygun/sdk': [join(SDK_DIR, 'src', 'index.ts')],
          '@raygun/sdk/site': [join(SDK_DIR, 'src', 'site.ts')],
        },
      });
      return ts.getPreEmitDiagnostics(program).map((diagnostic) => {
        const where = diagnostic.file
          ? `${diagnostic.file.fileName}:${diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start ?? 0).line + 1}`
          : '';
        return `${where} ${ts.flattenDiagnosticMessageText(diagnostic.messageText, ' ')}`;
      });
    };
    expect(problemsOf(files, ['lib.es2022.d.ts'])).toEqual([]);
    expect(
      problemsOf(pageFiles, ['lib.es2022.d.ts', 'lib.dom.d.ts', 'lib.dom.iterable.d.ts']),
    ).toEqual([]);
  });

  it('has JSON snippets Raygun reads', async () => {
    let manifests = 0;
    for (const page of await pages()) {
      for (const block of page.blocks.filter((candidate) => candidate.lang === 'json')) {
        const value = JSON.parse(block.code);
        if ('apiVersion' in value) {
          manifests += 1;
          expect(checkManifest(value).errors, `${page.file}:${block.line}`).toEqual([]);
        }
        if ('formatVersion' in value) {
          expect(value.formatVersion).toBe(1);
          for (const entry of value.extensions) {
            expect(entry.sha256, `${page.file}:${block.line}`).toMatch(/^[0-9a-f]{64}$/);
          }
        }
      }
    }
    expect(manifests).toBeGreaterThan(12);
  });

  it('links to pages and headings that exist', async () => {
    const all = await pages();
    for (const page of all) {
      for (const [, target = ''] of page.text.matchAll(/\]\(([^)\s]+)\)/g)) {
        if (/^[a-z]+:/.test(target)) continue;
        const [file = '', anchor] = target.split('#');
        // A page in a folder of its own (`formats/`) only has to exist.
        if (file.includes('/')) {
          expect(existsSync(join(DOCS, file)), `${page.file} → ${target}`).toBe(true);
          continue;
        }
        const linked = file === '' ? page : all.find((candidate) => candidate.file === file);
        expect(linked, `${page.file} → ${target}`).toBeDefined();
        expect(existsSync(join(DOCS, linked?.file ?? '')), target).toBe(true);
        if (anchor) {
          const headings = [...(linked?.text ?? '').matchAll(/^#+ (.+)$/gm)].map((match) =>
            slug(match[1] ?? ''),
          );
          expect(headings, `${page.file} → ${target}`).toContain(anchor);
        }
      }
    }
  });
});
