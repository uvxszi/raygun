// The Europeana APIs (Search and Record) with the person's own key, and what the films and
// recordings share: which licences count as open, how a licence and an institution are named, and
// which texts to show.
import { needsSetup, query } from '@raygun/sdk';

const API = 'https://api.europeana.eu/record/v2';
const AGENT = 'Raygun/1.0 ( https://github.com/uvxszi/raygun )';

/** The person's key; without one the source needs setup. */
const apiKey = (): string => {
  const key = raygun.settings.get<string>('apiKey')?.trim();
  if (!key) {
    return needsSetup(
      "Europeana needs your own free API key: create one in your europeana.eu account and enter it in this source's settings",
    );
  }
  return key;
};

const answers = new Map<string, unknown>();
const remember = (address: string, answer: unknown) => {
  answers.delete(address);
  answers.set(address, answer);
  if (answers.size > 60) answers.delete(answers.keys().next().value!);
};

/**
 * A Europeana API answer, kept in memory. The key goes in a header (`X-Api-Key`), never in the
 * address, so it stays out of errors and logs.
 */
export const europeana = async <T>(
  path: string,
  params: Record<string, string | number | boolean | readonly string[]>,
): Promise<T> => {
  const key = apiKey();
  const address = `${API}/${path}?${query(params)}`;
  const kept = answers.get(address);
  if (kept !== undefined) return kept as T;
  const response = await raygun.http.fetch(address, {
    headers: { Accept: 'application/json', 'X-Api-Key': key, 'User-Agent': AGENT },
  });
  if (response.status === 401 || response.status === 403) {
    needsSetup("Europeana didn't accept the API key: check it in this source's settings");
  }
  if (response.status === 404) throw new Error('This item is no longer on Europeana');
  if (response.status === 429) {
    throw new Error('Europeana is limiting the requests of this API key; try again later');
  }
  if (!response.ok) throw new Error(`Europeana answered HTTP ${response.status}`);
  const answer = await response.json<T & { success?: boolean; error?: string }>();
  if (answer.success === false) {
    throw new Error(`Europeana couldn't answer${answer.error ? `: ${answer.error}` : ''}`);
  }
  remember(address, answer);
  return answer;
};

/** Words for a search, without the characters Europeana's search language reads as operators. */
export const searchWords = (typed: string) =>
  typed
    .replace(/[+\-&|!(){}[\]^"~*?:\\/]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();

// ---- Licences ----

/**
 * Open licences only: the Public Domain Mark, CC0, CC BY and CC BY-SA (any version or port). What
 * `reusability=open` asks Europeana for; checked again here for every item shown.
 */
const OPEN =
  /^https?:\/\/creativecommons\.org\/(publicdomain\/(mark|zero)\/1\.0|licenses\/by(-sa)?\/\d\.\d(\/[a-z]{2,3})?)\/?$/i;

export const isOpen = (rights: string | undefined | null): rights is string =>
  Boolean(rights && OPEN.test(rights.trim()));

/** "CC BY-SA 4.0", "Public Domain Mark 1.0". */
export const licenceName = (rights: string): string => {
  const marked = /publicdomain\/(mark|zero)\/(\d\.\d)/i.exec(rights);
  if (marked) return marked[1]!.toLowerCase() === 'mark' ? 'Public Domain Mark 1.0' : 'CC0 1.0';
  const licence = /licenses\/(by(?:-sa)?)\/(\d\.\d)(?:\/([a-z]{2,3}))?/i.exec(rights);
  if (!licence) return rights;
  const port = licence[3] ? ` ${licence[3].toUpperCase()}` : '';
  return `CC ${licence[1]!.toUpperCase()} ${licence[2]}${port}`;
};

// ---- Texts ----

/** Language maps (`{ en: [...], def: [...] }`) and plain lists alike. */
export type LangMap = Record<string, string[] | undefined>;

/** The first value of a language map, in the order of `languages` (then any). */
export const pick = (
  values: LangMap | string[] | undefined | null,
  languages: string[],
): string | null => {
  if (!values) return null;
  if (Array.isArray(values)) return values.find((value) => value.trim())?.trim() ?? null;
  for (const language of [...languages, 'def', ...Object.keys(values)]) {
    const found = values[language]?.find((value) => value.trim());
    if (found) return found.trim();
  }
  return null;
};

/** Creators, without links to authority records ("https://…#ent2"). */
export const people = (values: LangMap | string[] | undefined | null, languages: string[]) => {
  if (!values) return [];
  const lists = Array.isArray(values)
    ? [values]
    : [...languages, 'def', ...Object.keys(values)].map((language) => values[language] ?? []);
  const first = lists.find((list) => list.some((value) => !/^https?:\/\//i.test(value))) ?? [];
  return [
    ...new Set(
      first
        .filter((value) => !/^https?:\/\//i.test(value))
        .map((value) => value.replace(/\s+/g, ' ').trim())
        .filter(Boolean),
    ),
  ];
};

export const firstYear = (texts: (string | undefined | null)[]): number | null => {
  for (const text of texts) {
    const year = /\b(1[5-9]\d\d|20\d\d)\b/.exec(text ?? '');
    if (year) return Number(year[1]);
  }
  return null;
};

/** Hosts whose links are pages with a player, not files. */
const PAGES = /^https?:\/\/([a-z0-9-]+\.)*(vimeo\.com|youtube\.com|youtu\.be|dailymotion\.com)\//i;

/** The file extension of an address's path, lower case, or `''`. */
export const extensionOf = (address: string) =>
  (/\.([a-z0-9]{2,5})$/i.exec(address.split(/[?#]/)[0] ?? '')?.[1] ?? '').toLowerCase();

/** A web page with a player, never the file itself. */
export const isPage = (address: string) =>
  PAGES.test(address) || /\.(html?|php|aspx?)$/i.test(address.split(/[?#]/)[0] ?? '');

export const isWeb = (address: string | undefined | null): address is string =>
  typeof address === 'string' && /^https?:\/\//i.test(address.trim());

/** Europeana ids start with a slash ("/2051906/…"); Raygun's don't. */
export const plainId = (id: string) => id.replace(/^\/+/, '');
