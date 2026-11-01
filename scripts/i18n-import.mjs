#!/usr/bin/env node
// Imports translations from the reference apps in `reference/` (git-ignored clones) wherever
// their English text is exactly Raygun's. Nothing is machine-translated: a key gets a
// translation only when another app already ships the same English string translated.
//
// Sources (in priority order) and licences, all compatible with Raygun's AGPL-3.0:
//   Nuclear (AGPL-3.0, i18next JSON), Mihon and aniyomi (Apache-2.0, moko-resources),
//   CloudStream, Shosetsu, RiPlay (GPL-3.0, Android resources).
//
// Usage: node scripts/i18n-import.mjs [referenceDir]
// Writes apps/raygun/src/i18n/locales/<tag>.json and SOURCES.md. Keys already in a locale file
// (hand-written or imported earlier) are never overwritten; only missing ones are added.

import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { basename, join } from 'node:path';

const reference = process.argv[2] ?? 'reference';
const localesDir = 'apps/raygun/src/i18n/locales';
const english = JSON.parse(readFileSync(join(localesDir, 'en.json'), 'utf8'));
/** Locales with fewer imported strings are left out (a picker entry would be mostly English). */
const MIN_STRINGS = 40;
/** Private-use or joke locales some apps ship. */
const SKIPPED_LOCALES = new Set(['qt', 'en']);

// ---- Locale tags ----

/** Region kept only where variants differ enough to matter. */
const KEEP_REGION = new Set(['pt-BR', 'zh-CN', 'zh-TW', 'zh-HK']);
const RENAMED = {
  in: 'id',
  iw: 'he',
  ji: 'yi',
  no: 'nb',
  'nb-NO': 'nb',
  zh: 'zh-CN',
  'zh-Hans': 'zh-CN',
  'zh-Hant': 'zh-TW',
};

const canonical = (raw) => {
  let tag = raw
    .replace(/^values-?/, '')
    .replace(/^b\+/, '')
    .replace(/\+/g, '-')
    .replace(/_/g, '-')
    .replace(/-r([A-Z]{2})$/, '-$1');
  if (!tag || tag === 'base') return 'en';
  tag = RENAMED[tag] ?? tag;
  const [language, region] = tag.split('-');
  const full = region
    ? `${language.toLowerCase()}-${region.length === 2 ? region.toUpperCase() : region}`
    : language.toLowerCase();
  if (RENAMED[full]) return RENAMED[full];
  if (KEEP_REGION.has(full)) return full;
  if (full === 'pt-PT') return 'pt';
  return RENAMED[language.toLowerCase()] ?? language.toLowerCase();
};

// ---- Reading sources ----

/** One string from a source: its text, or plural forms by CLDR category. */
const unescapeAndroid = (text) => {
  let value = text.trim();
  if (value.startsWith('"') && value.endsWith('"') && value.length >= 2) value = value.slice(1, -1);
  return value
    .replace(/\\n/g, '\n')
    .replace(/\\t/g, '\t')
    .replace(/\\(['"@?\\])/g, '$1')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&#8230;/g, '…')
    .replace(/&amp;/g, '&');
};

const cdata = (text) => text.replace(/^<!\[CDATA\[([\s\S]*)\]\]>$/, '$1');

/** Parses `<string>` and `<plurals>` entries, skipping marked-up and untranslatable ones. */
const readAndroidXml = (path) => {
  const xml = readFileSync(path, 'utf8').replace(/<!--[\s\S]*?-->/g, '');
  const strings = new Map();
  for (const match of xml.matchAll(/<string\s+([^>]*?)>([\s\S]*?)<\/string>/g)) {
    const [, attrs, body] = match;
    if (/translatable\s*=\s*"false"/.test(attrs)) continue;
    const name = /name\s*=\s*"([^"]+)"/.exec(attrs)?.[1];
    const inner = cdata(body.trim());
    if (!name || /<[a-zA-Z/]/.test(inner)) continue;
    strings.set(name, unescapeAndroid(inner));
  }
  for (const match of xml.matchAll(/<plurals\s+([^>]*?)>([\s\S]*?)<\/plurals>/g)) {
    const name = /name\s*=\s*"([^"]+)"/.exec(match[1])?.[1];
    if (!name) continue;
    const forms = {};
    for (const item of match[2].matchAll(/<item\s+quantity\s*=\s*"(\w+)"\s*>([\s\S]*?)<\/item>/g)) {
      const inner = cdata(item[2].trim());
      if (!/<[a-zA-Z/]/.test(inner)) forms[item[1]] = unescapeAndroid(inner);
    }
    if (Object.keys(forms).length) strings.set(name, forms);
  }
  return strings;
};

const flatten = (object, prefix = '', out = new Map()) => {
  for (const [key, value] of Object.entries(object)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (value && typeof value === 'object') flatten(value, path, out);
    else if (typeof value === 'string') out.set(path, value);
  }
  return out;
};

/** A source: name, licence, and per-locale string maps sharing keys with its English map. */
const sources = [];

const addAndroidSource = (name, license, dirs, fileNames = ['strings.xml', 'plurals.xml']) => {
  const locales = new Map();
  for (const dir of dirs) {
    if (!existsSync(dir)) continue;
    for (const entry of readdirSync(dir)) {
      const folder = join(dir, entry);
      if (!statSync(folder).isDirectory()) continue;
      if (!(
        entry === 'base' ||
        entry.startsWith('values') ||
        /^[a-z]{2,3}(-r[A-Z]{2})?$/.test(entry)
      ))
        continue;
      if (entry.startsWith('values') && !/^values(-|$)/.test(entry)) continue;
      // Android qualifiers that aren't languages (values-night, values-v31, values-land...).
      if (
        /^values-(night|v\d+|land|port|sw\d+dp|w\d+dp|h\d+dp|television|watch|notnight|large|xlarge|small|normal)/.test(
          entry,
        )
      )
        continue;
      const tag = canonical(entry);
      const merged = locales.get(tag) ?? new Map();
      for (const file of fileNames) {
        const path = join(folder, file);
        if (existsSync(path))
          for (const [key, value] of readAndroidXml(path)) merged.set(key, value);
      }
      if (merged.size) locales.set(tag, merged);
    }
  }
  if (locales.has('en')) sources.push({ name, license, locales });
};

// Nuclear: i18next JSON, `{{name}}` placeholders like Raygun's.
const nuclearDir = join(reference, 'nuclear/packages/i18n/src/locales');
if (existsSync(nuclearDir)) {
  const locales = new Map();
  for (const file of readdirSync(nuclearDir).filter((name) => name.endsWith('.json'))) {
    const tag = canonical(basename(file, '.json'));
    locales.set(tag, flatten(JSON.parse(readFileSync(join(nuclearDir, file), 'utf8'))));
  }
  sources.push({ name: 'Nuclear', license: 'AGPL-3.0', locales });
}
addAndroidSource('Mihon', 'Apache-2.0', [
  join(reference, 'mihon/i18n/src/commonMain/moko-resources'),
]);
addAndroidSource('aniyomi', 'Apache-2.0', [
  join(reference, 'aniyomi/i18n/src/commonMain/moko-resources'),
  join(reference, 'aniyomi/i18n-aniyomi/src/commonMain/moko-resources'),
]);
addAndroidSource('CloudStream', 'GPL-3.0', [join(reference, 'cloudstream/app/src/main/res')]);
for (const root of ['shosetsu', 'RiPlay']) {
  const dirs = [];
  const walk = (dir, depth) => {
    if (depth > 8 || !existsSync(dir)) return;
    for (const entry of readdirSync(dir)) {
      const path = join(dir, entry);
      if (!statSync(path).isDirectory() || entry === 'build' || entry.startsWith('.')) continue;
      if (entry === 'res' && existsSync(join(path, 'values/strings.xml'))) dirs.push(path);
      else walk(path, depth + 1);
    }
  };
  walk(join(reference, root), 0);
  addAndroidSource(root === 'RiPlay' ? 'RiPlay' : 'Shosetsu', 'GPL-3.0', dirs);
}

// ---- Matching ----

const PLACEHOLDER_ANDROID = /%(?:(\d+)\$)?[sd]/g;
const PLACEHOLDER_I18NEXT = /\{\{\s*([\w.]+)\s*\}\}/g;
const normalize = (text) =>
  text
    .replace(/\.\.\./g, '…')
    .replace(/\s+/g, ' ')
    .trim();

/** Text with placeholders replaced by `\u0000<n>` in order, plus the placeholder list. */
const shape = (text, kind) => {
  const names = [];
  let index = 0;
  const pattern = kind === 'android' ? PLACEHOLDER_ANDROID : PLACEHOLDER_I18NEXT;
  const shaped = text.replace(pattern, (_, group) => {
    const name = kind === 'android' ? String(group ? Number(group) - 1 : index) : group;
    index += 1;
    names.push(name);
    return `\u0000${names.length - 1}`;
  });
  return { shaped: normalize(shaped), names };
};

/** English text → candidate entries ({source, key}) in priority order. */
const byEnglish = new Map();
for (const source of sources) {
  for (const [key, value] of source.locales.get('en')) {
    const forms = typeof value === 'string' ? { other: value } : value;
    const kind = source.name === 'Nuclear' ? 'i18next' : 'android';
    for (const [quantity, text] of Object.entries(forms)) {
      const { shaped, names } = shape(text, kind);
      const id = typeof value === 'string' ? shaped : `${quantity}\u0001${shaped}`;
      const list = byEnglish.get(id) ?? [];
      list.push({
        source,
        key,
        quantity: typeof value === 'string' ? null : quantity,
        names,
        kind,
      });
      byEnglish.set(id, list);
    }
  }
}

/** Converts a source translation to Raygun's placeholders, or null when they don't line up. */
const convert = (text, candidate, ourNames) => {
  if (typeof text !== 'string' || !text.trim()) return null;
  const { names } = candidate;
  const pattern = candidate.kind === 'android' ? PLACEHOLDER_ANDROID : PLACEHOLDER_I18NEXT;
  let index = 0;
  let ok = true;
  const used = new Set();
  const result = text.replace(pattern, (_, group) => {
    const name = candidate.kind === 'android' ? String(group ? Number(group) - 1 : index) : group;
    index += 1;
    const position = names.indexOf(name);
    if (position < 0 || ourNames[position] === undefined) {
      ok = false;
      return '';
    }
    used.add(position);
    return `{{${ourNames[position]}}}`;
  });
  return ok && used.size === names.length ? (normalize(result) === '' ? null : result) : null;
};

const ours = flatten(english);
/** Plural groups: base key → {one, other} English. */
const PLURAL = /_(zero|one|two|few|many|other)$/;
const translations = new Map(); // tag → Map(key → {text, source})

const put = (tag, key, text, source) => {
  const map = translations.get(tag) ?? new Map();
  if (!map.has(key)) map.set(key, { text, source });
  translations.set(tag, map);
};

for (const [key, value] of ours) {
  const plural = PLURAL.exec(key);
  if (plural && plural[1] !== 'other') continue; // handled with the `_other` key
  const { shaped, names } = shape(value, 'i18next');
  if (!plural) {
    for (const candidate of byEnglish.get(shaped) ?? []) {
      if (candidate.quantity !== null || candidate.names.length !== names.length) continue;
      for (const [tag, strings] of candidate.source.locales) {
        if (tag === 'en') continue;
        const text = convert(strings.get(candidate.key), candidate, names);
        if (text !== null) put(tag, key, text, candidate.source.name);
      }
    }
    continue;
  }
  // A plural group matches a source plural whose English `one` and `other` are both ours.
  const base = key.slice(0, -'_other'.length);
  const one = ours.get(`${base}_one`);
  for (const candidate of byEnglish.get(`other\u0001${shaped}`) ?? []) {
    const englishForms = candidate.source.locales.get('en').get(candidate.key);
    if (one !== undefined && englishForms.one !== undefined) {
      if (shape(englishForms.one, candidate.kind).shaped !== shape(one, 'i18next').shaped) continue;
    }
    for (const [tag, strings] of candidate.source.locales) {
      if (tag === 'en') continue;
      const forms = strings.get(candidate.key);
      if (!forms || typeof forms === 'string') continue;
      const converted = Object.entries(forms).map(([quantity, text]) => [
        quantity,
        convert(text, candidate, names),
      ]);
      if (converted.some(([, text]) => text === null) || !forms.other) continue;
      for (const [quantity, text] of converted)
        put(tag, `${base}_${quantity}`, text, candidate.source.name);
    }
  }
}

// ---- Writing ----

const nest = (entries) => {
  const root = {};
  for (const [key, text] of [...entries].sort(([a], [b]) => a.localeCompare(b))) {
    const parts = key.split('.');
    let node = root;
    for (const part of parts.slice(0, -1)) node = node[part] ??= {};
    node[parts.at(-1)] = text;
  }
  return root;
};

const report = [];
for (const [tag, map] of [...translations].sort(([a], [b]) => a.localeCompare(b))) {
  const path = join(localesDir, `${tag}.json`);
  if (SKIPPED_LOCALES.has(tag)) continue;
  const existing = existsSync(path) ? flatten(JSON.parse(readFileSync(path, 'utf8'))) : new Map();
  const merged = new Map(existing);
  const counts = {};
  for (const [key, { text, source }] of map) {
    if (merged.has(key) || (!ours.has(key.replace(PLURAL, '_other')) && !ours.has(key))) continue;
    merged.set(key, text);
    counts[source] = (counts[source] ?? 0) + 1;
  }
  const total = merged.size;
  if (total < MIN_STRINGS && !existing.size) continue;
  writeFileSync(path, `${JSON.stringify(nest(merged), null, 2)}\n`);
  report.push({ tag, total, counts, kept: existing.size });
}

const lines = [
  '# Where the translations come from',
  '',
  'Generated by `pnpm i18n:import` (`scripts/i18n-import.mjs`). Each string below was copied from an app',
  "whose English text is exactly Raygun's; nothing was machine-translated. Re-running the import only",
  'adds missing keys, so hand-written translations are never overwritten.',
  '',
  '| Source | Licence |',
  '|---|---|',
  ...sources.map((source) => `| ${source.name} | ${source.license} |`),
  '',
  `| Locale | Strings (of ${ours.size}) | From |`,
  '|---|---|---|',
  ...report.map(
    ({ tag, total, counts }) =>
      `| ${tag} | ${total} | ${Object.entries(counts)
        .map(([name, count]) => `${name} ${count}`)
        .join(', ')} |`,
  ),
  '',
];
writeFileSync(join(localesDir, 'SOURCES.md'), lines.join('\n'));
console.log(`${report.length} locales written; ${ours.size} English strings.`);
for (const { tag, total } of report) console.log(`  ${tag}: ${total}`);
