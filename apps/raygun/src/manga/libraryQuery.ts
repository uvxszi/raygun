import type { MangaLibraryEntry } from '@raygun/bindings';

/**
 * The library's search language (M46), after Mihon's `LibrarySearchParser`:
 *
 *   or    := and ( ("|" | "||") and )*
 *   and   := term ( ["&&"] term )*          words next to each other must all match
 *   term  := "-"term | "(" or ")" | field ":" value | field op value | value
 *   value := "quoted text" | 'quoted text' | word        commas and spaces separate terms
 *
 * A word matches the title, authors, artists, genres or source; `local` also finds imported
 * titles. Fields: title, author, artist, genre (tag), source (src), status, list (with
 * `favorite`); comparisons: unread, read, total, downloaded, added (a date, YYYY-MM-DD).
 * An unknown field is plain text, and a missing ")" is forgiven.
 */

type Op = ':' | '=' | '>' | '>=' | '<' | '<=';

type Token =
  | { kind: '(' }
  | { kind: ')' }
  | { kind: 'or' }
  | { kind: 'and' }
  | { kind: 'not' }
  | { kind: 'word'; text: string; quoted: boolean };

export type QueryNode =
  | { kind: 'or'; nodes: QueryNode[] }
  | { kind: 'and'; nodes: QueryNode[] }
  | { kind: 'not'; node: QueryNode }
  | { kind: 'text'; value: string }
  | { kind: 'field'; field: TextField; value: string }
  | { kind: 'compare'; field: NumberField; op: Op; value: string };

type TextField = 'title' | 'author' | 'artist' | 'genre' | 'source' | 'status' | 'list';
type NumberField = 'unread' | 'read' | 'total' | 'downloaded' | 'added';

const TEXT_FIELDS: Record<string, TextField> = {
  title: 'title',
  author: 'author',
  artist: 'artist',
  genre: 'genre',
  tag: 'genre',
  source: 'source',
  src: 'source',
  status: 'status',
  list: 'list',
};
const NUMBER_FIELDS: Record<string, NumberField> = {
  unread: 'unread',
  unseen: 'unread',
  read: 'read',
  seen: 'read',
  total: 'total',
  downloaded: 'downloaded',
  added: 'added',
};

const isSeparator = (char: string) => /[\s,]/.test(char);

const tokenize = (input: string): Token[] => {
  const tokens: Token[] = [];
  let index = 0;
  const quoted = (quote: string) => {
    const end = input.indexOf(quote, index + 1);
    const text = input.slice(index + 1, end === -1 ? input.length : end);
    index = end === -1 ? input.length : end + 1;
    return text;
  };
  while (index < input.length) {
    const char = input.charAt(index);
    if (isSeparator(char)) {
      index += 1;
    } else if (char === '(' || char === ')') {
      tokens.push({ kind: char });
      index += 1;
    } else if (char === '|') {
      tokens.push({ kind: 'or' });
      index += input.charAt(index + 1) === '|' ? 2 : 1;
    } else if (char === '&' && input.charAt(index + 1) === '&') {
      tokens.push({ kind: 'and' });
      index += 2;
    } else if (char === '-' && index + 1 < input.length && !isSeparator(input.charAt(index + 1))) {
      tokens.push({ kind: 'not' });
      index += 1;
    } else if (char === '"' || char === "'") {
      tokens.push({ kind: 'word', text: quoted(char), quoted: true });
    } else {
      let text = '';
      while (index < input.length && !/[\s,()|]/.test(input.charAt(index))) {
        if (input.charAt(index) === '&' && input.charAt(index + 1) === '&') break;
        // field:"quoted value"
        if (/[:=<>]/.test(input.charAt(index)) && /["']/.test(input.charAt(index + 1))) {
          text += input.charAt(index);
          index += 1;
          text += `\u0000${quoted(input.charAt(index))}`;
          break;
        }
        text += input.charAt(index);
        index += 1;
      }
      tokens.push({ kind: 'word', text, quoted: false });
    }
  }
  return tokens;
};

const term = (token: Extract<Token, { kind: 'word' }>): QueryNode => {
  if (token.quoted) return { kind: 'text', value: token.text.toLocaleLowerCase() };
  const match = /^([a-z_]+)(>=|<=|:|=|>|<)(.*)$/is.exec(token.text);
  if (match) {
    const name = match[1] ?? '';
    const op = match[2] ?? ':';
    const value = (match[3] ?? '').replace('\u0000', '');
    const textField = TEXT_FIELDS[name.toLowerCase()];
    const numberField = NUMBER_FIELDS[name.toLowerCase()];
    if (textField && op === ':') {
      return { kind: 'field', field: textField, value: value.toLocaleLowerCase() };
    }
    if (numberField) return { kind: 'compare', field: numberField, op: op as Op, value };
  }
  return { kind: 'text', value: token.text.replace('\u0000', '').toLocaleLowerCase() };
};

/** Parses a query; an empty one is null (everything matches). */
export const parseLibraryQuery = (input: string): QueryNode | null => {
  const tokens = tokenize(input);
  let position = 0;
  const peek = () => tokens[position];

  const parseOr = (): QueryNode | null => {
    const nodes: QueryNode[] = [];
    let node = parseAnd();
    if (node) nodes.push(node);
    while (peek()?.kind === 'or') {
      position += 1;
      node = parseAnd();
      if (node) nodes.push(node);
    }
    if (nodes.length === 0) return null;
    return nodes.length === 1 ? (nodes[0] ?? null) : { kind: 'or', nodes };
  };
  const parseAnd = (): QueryNode | null => {
    const nodes: QueryNode[] = [];
    for (;;) {
      const next = peek();
      if (!next || next.kind === 'or' || next.kind === ')') break;
      if (next.kind === 'and') {
        position += 1;
        continue;
      }
      const node = parseTerm();
      if (node) nodes.push(node);
    }
    if (nodes.length === 0) return null;
    return nodes.length === 1 ? (nodes[0] ?? null) : { kind: 'and', nodes };
  };
  const parseTerm = (): QueryNode | null => {
    const token = tokens[position];
    position += 1;
    if (!token) return null;
    switch (token.kind) {
      case 'not': {
        const inner = peek() ? parseTerm() : null;
        return inner ? { kind: 'not', node: inner } : null;
      }
      case '(': {
        const inner = parseOr();
        if (peek()?.kind === ')') position += 1;
        return inner;
      }
      case 'word':
        return term(token);
      default:
        return null;
    }
  };

  const root = parseOr();
  // Stray ")" at the top level: keep reading what follows.
  while (position < tokens.length) {
    position += 1;
    const more = parseOr();
    if (more) return root ? { kind: 'and', nodes: [root, more] } : more;
  }
  return root;
};

const has = (values: string[], value: string) =>
  values.some((item) => item.toLocaleLowerCase().includes(value));

/** A field's values; `""` asks for titles where it's empty. */
const fieldValues = (entry: MangaLibraryEntry, field: TextField): string[] => {
  switch (field) {
    case 'title':
      return [entry.title];
    case 'author':
      return entry.authors;
    case 'artist':
      return entry.artists;
    case 'genre':
      return entry.genres;
    case 'source':
      return [entry.sourceName];
    case 'status':
      return [entry.status];
    case 'list':
      return [entry.list ?? '', entry.favorite ? 'favorite' : ''].filter(Boolean);
  }
};

const localDay = (ms: number) => {
  const date = new Date(ms);
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
};

const compare = (left: number | string, op: Op, right: number | string) => {
  switch (op) {
    case '>':
      return left > right;
    case '>=':
      return left >= right;
    case '<':
      return left < right;
    case '<=':
      return left <= right;
    default:
      return left === right;
  }
};

/** Whether an entry matches a parsed query. */
export const matchesQuery = (entry: MangaLibraryEntry, node: QueryNode | null): boolean => {
  if (!node) return true;
  switch (node.kind) {
    case 'or':
      return node.nodes.some((child) => matchesQuery(entry, child));
    case 'and':
      return node.nodes.every((child) => matchesQuery(entry, child));
    case 'not':
      return !matchesQuery(entry, node.node);
    case 'text':
      if (node.value === 'local' && entry.local) return true;
      return has(
        [entry.title, ...entry.authors, ...entry.artists, ...entry.genres, entry.sourceName],
        node.value,
      );
    case 'field': {
      const values = fieldValues(entry, node.field).filter((value) => value.trim() !== '');
      return node.value === '' ? values.length === 0 : has(values, node.value);
    }
    case 'compare': {
      if (node.field === 'added') {
        if (!/^\d{4}-\d{2}-\d{2}$/.test(node.value) || entry.addedAt === null) return false;
        return compare(localDay(entry.addedAt), node.op === ':' ? '=' : node.op, node.value);
      }
      const value = Number(node.value);
      if (node.value === '' || !Number.isFinite(value)) return false;
      const actual = {
        unread: entry.unread,
        read: entry.chapters - entry.unread,
        total: entry.chapters,
        downloaded: entry.downloaded,
      }[node.field];
      return compare(actual, node.op === ':' ? '=' : node.op, value);
    }
  }
};
