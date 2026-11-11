// What the Internet Archive's video providers share: its search and item records, read the same
// way for films and lectures.
import { fetchJson } from '@raygun/sdk';
import type { MangaPage } from '@raygun/sdk';

export const API = 'https://archive.org';
const ROWS = 30;

type SearchDoc = { identifier: string; title?: string | string[]; year?: number | string };
type SearchResponse = { response?: { numFound?: number; docs?: SearchDoc[] } };

export type ItemFile = {
  name: string;
  format?: string;
  source?: string;
  original?: string;
  height?: string;
  title?: string;
};
export type ItemMetadata = {
  files?: ItemFile[];
  metadata?: {
    title?: string | string[];
    description?: string | string[];
    date?: string;
    year?: string;
    creator?: string | string[];
    subject?: string | string[];
    licenseurl?: string;
    collection?: string | string[];
    uploader?: string;
  };
};

// Lucene syntax characters would turn a user's query into a malformed or different search.
export const escapeQuery = (query: string) =>
  query.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, ' ').trim();

export const firstString = (value: string | string[] | undefined): string | null =>
  (Array.isArray(value) ? value[0] : value) ?? null;

export const encodePath = (path: string) => path.split('/').map(encodeURIComponent).join('/');

export const cover = (identifier: string) =>
  `${API}/services/img/${encodeURIComponent(identifier)}`;

export const fileUrl = (identifier: string, name: string) =>
  `${API}/download/${encodeURIComponent(identifier)}/${encodePath(name)}`;

/** Descriptions are HTML; the app shows plain text. */
export const plainText = (html: string) =>
  html
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<\/p>/gi, '\n\n')
    .replace(/<[^>]*>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&')
    .replace(/\n{3,}/g, '\n\n')
    .trim();

/** The subjects an item lists, split where uploaders joined them with semicolons. */
export const subjectsOf = (item: ItemMetadata) => {
  const subject = item.metadata?.subject;
  const all = (Array.isArray(subject) ? subject : [subject ?? ''])
    .flatMap((each) => each.split(/\s*;\s*/))
    .map((each) => each.trim())
    .filter(Boolean);
  return [...new Set(all)];
};

/** A page of items the advanced search finds for `q`, in `sort` order. */
export const searchPage = async (q: string, sort: string, page: number): Promise<MangaPage> => {
  const params = [
    `q=${encodeURIComponent(q)}`,
    'fl[]=identifier',
    'fl[]=title',
    `rows=${ROWS}`,
    `page=${Math.max(1, page)}`,
    `sort[]=${encodeURIComponent(sort)}`,
    'output=json',
  ];
  const result = await fetchJson<SearchResponse>(`${API}/advancedsearch.php?${params.join('&')}`);
  const docs = result.response?.docs ?? [];
  return {
    items: docs.map((doc) => ({
      id: doc.identifier,
      title: firstString(doc.title) ?? doc.identifier,
      coverUrl: cover(doc.identifier),
    })),
    hasNextPage: page * ROWS < (result.response?.numFound ?? 0),
  };
};

// Item metadata is read for details, episodes and servers; keep the last few.
const items = new Map<string, Promise<ItemMetadata>>();
export const fetchItem = (identifier: string): Promise<ItemMetadata> => {
  let item = items.get(identifier);
  if (!item) {
    item = fetchJson<ItemMetadata>(`${API}/metadata/${encodeURIComponent(identifier)}`);
    item.catch(() => items.delete(identifier));
    items.set(identifier, item);
    if (items.size > 20) items.delete(items.keys().next().value!);
  }
  return item;
};
