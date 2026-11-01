import en from './locales/en.json';
import { LANGUAGES, matchLanguage, nativeName, resolveLanguage } from './languages';

type Tree = { [key: string]: string | Tree };

const flatten = (tree: Tree, prefix = '', out = new Map<string, string>()) => {
  for (const [key, value] of Object.entries(tree)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (typeof value === 'string') out.set(path, value);
    else flatten(value, path, out);
  }
  return out;
};

const PLURAL = /_(zero|one|two|few|many|other)$/;
const placeholders = (text: string) =>
  [...text.matchAll(/\{\{\s*([\w.]+)\s*\}\}/g)].map((match) => match[1]).sort();

const english = flatten(en as Tree);
const locales = import.meta.glob<{ default: Tree }>(['./locales/*.json', '!./locales/en.json'], {
  eager: true,
});

describe('locale files', () => {
  it('are all offered', () => {
    expect(LANGUAGES[0]).toBe('en');
    expect(LANGUAGES.length).toBe(Object.keys(locales).length + 1);
  });

  it.each(Object.entries(locales))('%s only has keys and placeholders English has', (_, file) => {
    for (const [key, text] of flatten(file.default)) {
      const base = PLURAL.test(key) ? key.replace(PLURAL, '_other') : key;
      const source = english.get(key) ?? english.get(base);
      expect(source, `unknown key ${key}`).toBeDefined();
      expect(text.trim(), `empty ${key}`).not.toBe('');
      for (const name of placeholders(text)) {
        expect(placeholders(source!), `${key} uses {{${name}}}`).toContain(name);
      }
    }
  });
});

describe('language matching', () => {
  const available = ['en', 'de', 'pt', 'pt-BR', 'zh-CN', 'zh-TW'];

  it('prefers an exact tag, then the same base language, then English', () => {
    expect(matchLanguage(['pt-BR'], available)).toBe('pt-BR');
    expect(matchLanguage(['pt-PT'], available)).toBe('pt');
    expect(matchLanguage(['de-AT', 'fr'], available)).toBe('de');
    expect(matchLanguage(['zh-TW'], available)).toBe('zh-TW');
    expect(matchLanguage(['xx', 'yy'], available)).toBe('en');
  });

  it('follows the device for "system"', () => {
    expect(resolveLanguage('system', ['de-CH'])).toBe('de');
    expect(resolveLanguage('en', ['de-CH'])).toBe('en');
    expect(resolveLanguage(undefined, [])).toBe('en');
  });

  it('names languages in their own language', () => {
    expect(nativeName('de')).toBe('Deutsch');
    expect(nativeName('not a tag!')).toBe('not a tag!');
    expect(nativeName('qqq')).toBe('qqq');
  });
});
