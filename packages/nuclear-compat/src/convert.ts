// Converts a Nuclear plugin (package.json + TS/JS sources) into one plain script for Raygun's `js`
// runtime, plus a generated raygun-extension.json and a compatibility report (ARCHITECTURE §7.4).
import { transform } from 'sucrase';

/** Nuclear plugin categories Raygun runs, and the Raygun provider kinds they become. */
export const CATEGORY_KINDS: Record<string, string> = {
  metadata: 'music.metadata',
  streaming: 'music.stream',
  lyrics: 'lyrics',
};

/** API domains the shim stubs (keep in step with shim/api.ts). */
const STUBBED_DOMAINS = [
  'Queue',
  'Playback',
  'Favorites',
  'Playlists',
  'Dashboard',
  'Discovery',
  'Events',
  'Shell',
  'Navigation',
  'Ytdlp',
  'Metadata',
  'Streaming',
  'Lyrics',
];

const ENTRY_CANDIDATES = [
  'index.ts',
  'index.tsx',
  'index.js',
  'src/index.ts',
  'src/index.tsx',
  'src/index.js',
  'dist/index.js',
];

export type CompatReport = {
  /** Raygun provider kinds the plugin provides. */
  supported: string[];
  /** Nuclear categories Raygun can't run (dashboard, playlists, ...). */
  unsupportedCategories: string[];
  /** Nuclear API domains the plugin calls that are stubs in Raygun. */
  stubbedApis: string[];
  /** UI modules the plugin imports; they're inert here. */
  uiImports: string[];
  /** Hosts found in the source; they become the extension's network permissions. */
  hosts: string[];
  modules: number;
};

export type ConvertResult = {
  manifest: Record<string, unknown>;
  bundle: string;
  report: CompatReport;
};

type PackageJson = {
  name?: string;
  version?: string;
  description?: string;
  author?: string | { name?: string };
  main?: string;
  nuclear?: { displayName?: string; category?: string; categories?: string[] };
};

const isModule = (path: string) =>
  /\.(tsx?|jsx?|mjs|cjs|json)$/.test(path) &&
  !path.endsWith('.d.ts') &&
  !/\.(test|spec)\.[cm]?[jt]sx?$/.test(path) &&
  !/(^|\/)(test|tests|__tests__|__mocks__|node_modules)\//.test(path) &&
  !/(^|\/)(package-lock|tsconfig[^/]*|package)\.json$/.test(path);

/** `@scope/My Plugin` → `scope-my-plugin`, a valid reverse-DNS label. */
const slug = (name: string) =>
  name
    .toLowerCase()
    .replace(/^@/, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 80) || 'plugin';

const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/;

const IGNORED_HOSTS = new Set(['localhost', 'example.com', 'example.org', 'www.w3.org']);

const findHosts = (sources: string[]) => {
  const hosts = new Set<string>();
  for (const source of sources) {
    for (const [, host] of source.matchAll(/https?:\/\/([a-zA-Z0-9-]+(?:\.[a-zA-Z0-9-]+)+)/g)) {
      const name = (host ?? '').toLowerCase();
      if (!IGNORED_HOSTS.has(name)) hosts.add(name);
    }
  }
  return [...hosts].sort();
};

const compile = (path: string, source: string) => {
  if (path.endsWith('.json')) return `module.exports = ${source.trim() || 'null'};`;
  const jsx = /\.[jt]sx$/.test(path);
  return transform(source, {
    transforms: [
      ...(/\.tsx?$/.test(path) ? (['typescript'] as const) : []),
      ...(jsx ? (['jsx'] as const) : []),
      'imports',
    ],
    jsxRuntime: 'automatic',
    production: true,
    filePath: path,
  }).code;
};

export const convertPlugin = (files: Record<string, string>, shimSource: string): ConvertResult => {
  // Zips sometimes wrap everything in one folder; the shallowest package.json marks the root.
  const packagePath = Object.keys(files)
    .filter((path) => path === 'package.json' || path.endsWith('/package.json'))
    .filter((path) => !path.includes('node_modules/'))
    .sort((a, b) => a.split('/').length - b.split('/').length)[0];
  if (!packagePath) throw new Error('This is not a Nuclear plugin: there is no package.json');
  const root = packagePath.slice(0, -'package.json'.length);
  const pkg = JSON.parse(files[packagePath] as string) as PackageJson;
  if (!pkg.name) throw new Error('package.json has no name');

  const sources: Record<string, string> = {};
  for (const [path, source] of Object.entries(files)) {
    if (!path.startsWith(root)) continue;
    const relative = path.slice(root.length);
    if (isModule(relative)) sources[relative] = source;
  }

  const entry = [pkg.main?.replace(/^\.\//, ''), ...ENTRY_CANDIDATES].find(
    (candidate): candidate is string => !!candidate && candidate in sources,
  );
  if (!entry) throw new Error(`Can't find the plugin's entry file (${pkg.main ?? 'index.ts'})`);

  const categories =
    pkg.nuclear?.categories ?? (pkg.nuclear?.category ? [pkg.nuclear.category] : []);
  const code = Object.values(sources);
  // Plugins also register providers their categories don't name (OmniSource's metadata, M46.8).
  const registered = [
    ...new Set(
      code.flatMap((source) =>
        [...source.matchAll(/\bkind\s*:\s*['"](metadata|streaming|lyrics)['"]/g)].map(
          ([, kind]) => kind as string,
        ),
      ),
    ),
  ];
  const report: CompatReport = {
    supported: [
      ...new Set(
        [...categories, ...registered].flatMap((category) => CATEGORY_KINDS[category] ?? []),
      ),
    ],
    unsupportedCategories: categories.filter((category) => !(category in CATEGORY_KINDS)),
    stubbedApis: STUBBED_DOMAINS.filter((domain) =>
      code.some((source) => new RegExp(`\\.${domain}\\s*\\.`).test(source)),
    ),
    uiImports: [
      ...new Set(
        code.flatMap((source) =>
          [...source.matchAll(/from\s+['"](react(?:\/[\w-]+)?|@nuclearplayer\/ui)['"]/g)].map(
            ([, module]) => module as string,
          ),
        ),
      ),
    ].sort(),
    hosts: findHosts(code),
    modules: Object.keys(sources).length,
  };
  if (report.supported.length === 0) {
    const what = categories.length > 0 ? categories.join(', ') : 'no provider categories';
    throw new Error(`Raygun can't run this Nuclear plugin yet: it provides ${what}`);
  }

  const definitions = Object.entries(sources)
    .map(
      ([path, source]) =>
        `${JSON.stringify(path)}: function (exports, require, module) {\n${compile(path, source)}\n}`,
    )
    .join(',\n');
  const plugin = {
    id: pkg.name,
    name: pkg.nuclear?.displayName ?? pkg.name,
    version: pkg.version ?? '0.0.0',
    categories,
  };
  const bundle = [
    `// Nuclear plugin ${pkg.name} ${pkg.version ?? ''}, converted for Raygun by @raygun/nuclear-compat.`,
    shimSource,
    '(function () {',
    `var definitions = {\n${definitions}\n};`,
    `__nuclearShim.run({ entry: ${JSON.stringify(entry)}, definitions: definitions, plugin: ${JSON.stringify(plugin)} });`,
    '})();',
    '',
  ].join('\n');

  const author = typeof pkg.author === 'string' ? pkg.author : pkg.author?.name;
  const manifest = {
    id: `nuclear.${slug(pkg.name)}`,
    name: (pkg.nuclear?.displayName ?? pkg.name).slice(0, 80),
    version: pkg.version && SEMVER.test(pkg.version) ? pkg.version : '0.0.0',
    apiVersion: 1,
    runtime: 'js',
    entry: 'dist/index.js',
    description: pkg.description ?? null,
    author: author ?? null,
    provides: report.supported,
    permissions: { net: report.hosts },
    contentRating: 'safe',
    nuclear: { package: pkg.name, report },
  };
  return { manifest, bundle, report };
};
