/**
 * A small GitHub-flavoured Markdown parser for release notes: headings, paragraphs, bold,
 * italics, strikethrough, inline and fenced code, lists (nested, ordered, task boxes), quotes,
 * rules, tables and links. `Markdown.tsx` builds React elements only, so nothing in the text becomes HTML:
 * tags show as text (`<br>` is a line break). Only https links are links; they open outside.
 */

export type MarkdownBlock =
  | { kind: 'heading'; level: number; text: string }
  | { kind: 'paragraph'; text: string }
  | { kind: 'code'; text: string }
  | { kind: 'list'; ordered: boolean; start: number; items: MarkdownItem[] }
  | { kind: 'quote'; blocks: MarkdownBlock[] }
  | { kind: 'table'; head: string[]; rows: string[][] }
  | { kind: 'rule' };

export type MarkdownItem = { checked: boolean | null; blocks: MarkdownBlock[] };

const FENCE = /^ {0,3}(`{3,}|~{3,})/;
const HEADING = /^ {0,3}(#{1,6})(?:\s+(.*?))?(?:\s+#+)?\s*$/;
const RULE = /^ {0,3}([-*_])(?:\s*\1){2,}\s*$/;
const QUOTE = /^ {0,3}>\s?/;
const LIST_ITEM = /^( {0,3})([-*+]|\d{1,9}[.)])(\s+|$)(.*)$/;
const SETEXT = /^ {0,3}(=+|-+)\s*$/;
const TABLE_DIVIDER = /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/;

const blank = (line: string) => line.trim() === '';

const indentOf = (line: string) => line.length - line.trimStart().length;

/** Whether `line` starts a block other than a paragraph (so it ends one). */
const startsBlock = (line: string) =>
  FENCE.test(line) ||
  HEADING.test(line) ||
  RULE.test(line) ||
  QUOTE.test(line) ||
  LIST_ITEM.test(line);

const tableCells = (line: string) =>
  line
    .trim()
    .replace(/^\|/, '')
    .replace(/\|$/, '')
    .split(/(?<!\\)\|/)
    .map((cell) => cell.trim());

/** The blocks of `source`. */
export const parseMarkdown = (source: string): MarkdownBlock[] =>
  parseLines(source.replace(/\r\n?/g, '\n').replace(/\t/g, '    ').split('\n'));

const parseLines = (lines: string[]): MarkdownBlock[] => {
  const blocks: MarkdownBlock[] = [];
  let index = 0;
  while (index < lines.length) {
    const line = lines[index] ?? '';
    if (blank(line)) {
      index += 1;
      continue;
    }
    const fence = FENCE.exec(line);
    if (fence) {
      const marker = fence[1] ?? '';
      const body: string[] = [];
      index += 1;
      while (index < lines.length && !(lines[index] ?? '').trimStart().startsWith(marker)) {
        body.push(lines[index] ?? '');
        index += 1;
      }
      index += 1;
      blocks.push({ kind: 'code', text: body.join('\n') });
      continue;
    }
    const heading = HEADING.exec(line);
    if (heading) {
      blocks.push({ kind: 'heading', level: (heading[1] ?? '').length, text: heading[2] ?? '' });
      index += 1;
      continue;
    }
    if (RULE.test(line)) {
      blocks.push({ kind: 'rule' });
      index += 1;
      continue;
    }
    if (QUOTE.test(line)) {
      const body: string[] = [];
      while (index < lines.length && !blank(lines[index] ?? '')) {
        body.push((lines[index] ?? '').replace(QUOTE, ''));
        index += 1;
      }
      blocks.push({ kind: 'quote', blocks: parseLines(body) });
      continue;
    }
    if (LIST_ITEM.test(line)) {
      index = parseList(lines, index, blocks);
      continue;
    }
    if (
      line.includes('|') &&
      index + 1 < lines.length &&
      TABLE_DIVIDER.test(lines[index + 1] ?? '')
    ) {
      const head = tableCells(line);
      const rows: string[][] = [];
      index += 2;
      while (
        index < lines.length &&
        !blank(lines[index] ?? '') &&
        (lines[index] ?? '').includes('|')
      ) {
        rows.push(tableCells(lines[index] ?? ''));
        index += 1;
      }
      blocks.push({ kind: 'table', head, rows });
      continue;
    }
    const text: string[] = [line.trim()];
    index += 1;
    while (index < lines.length && !blank(lines[index] ?? '')) {
      const next = lines[index] ?? '';
      const setext = SETEXT.exec(next);
      if (setext) {
        blocks.push({
          kind: 'heading',
          level: (setext[1] ?? '').startsWith('=') ? 1 : 2,
          text: text.join(' '),
        });
        text.length = 0;
        index += 1;
        break;
      }
      if (startsBlock(next)) break;
      // Two trailing spaces or a backslash end a line; other line ends are kept, as GitHub does
      // in release notes.
      text.push(next.trim());
      index += 1;
    }
    if (text.length > 0) {
      blocks.push({
        kind: 'paragraph',
        text: text.map((part) => part.replace(/\\$/, '')).join('\n'),
      });
    }
  }
  return blocks;
};

/** Reads the list starting at `start` into `blocks`; returns the line after it. */
const parseList = (lines: string[], start: number, blocks: MarkdownBlock[]) => {
  const first = LIST_ITEM.exec(lines[start] ?? '');
  if (!first) return start + 1;
  const baseIndent = (first[1] ?? '').length;
  const ordered = /\d/.test(first[2] ?? '');
  const items: MarkdownItem[] = [];
  let index = start;
  while (index < lines.length) {
    const marker = LIST_ITEM.exec(lines[index] ?? '');
    if (
      !marker ||
      (marker[1] ?? '').length !== baseIndent ||
      /\d/.test(marker[2] ?? '') !== ordered
    )
      break;
    const contentIndent =
      baseIndent + (marker[2] ?? '').length + Math.max(1, Math.min((marker[3] ?? '').length, 4));
    const body: string[] = [marker[4] ?? ''];
    index += 1;
    while (index < lines.length) {
      const next = lines[index] ?? '';
      if (blank(next)) {
        // A blank line continues the item only when indented content follows.
        const after = lines[index + 1];
        if (after !== undefined && !blank(after) && indentOf(after) >= contentIndent) {
          body.push('');
          index += 1;
          continue;
        }
        break;
      }
      if (indentOf(next) >= contentIndent) {
        body.push(next.slice(contentIndent));
      } else if (LIST_ITEM.test(next) || startsBlock(next)) {
        break;
      } else {
        // A lazy continuation of the item's text.
        body.push(next.trim());
      }
      index += 1;
    }
    const task = /^\[([ xX])\]\s+/.exec(body[0] ?? '');
    if (task) body[0] = (body[0] ?? '').slice((task[0] ?? '').length);
    items.push({ checked: task ? (task[1] ?? '') !== ' ' : null, blocks: parseLines(body) });
    // Items separated by one blank line stay in the same list.
    if (index < lines.length && blank(lines[index] ?? '')) {
      const after = lines[index + 1];
      const again = after === undefined ? null : LIST_ITEM.exec(after);
      if (again && (again[1] ?? '').length === baseIndent) index += 1;
    }
  }
  blocks.push({
    kind: 'list',
    ordered,
    start: ordered ? Number.parseInt(first[2] ?? '', 10) : 1,
    items,
  });
  return index;
};

export type MarkdownInline =
  | { kind: 'text'; text: string }
  | { kind: 'code'; text: string }
  | { kind: 'strong' | 'em' | 'del'; children: MarkdownInline[] }
  | { kind: 'link'; url: string | null; children: MarkdownInline[] }
  | { kind: 'break' };

const BARE_URL = /^https?:\/\/[^\s<>]*[^\s<>.,:;"')\]!?*_~]/;
const ESCAPABLE = /[\\`*_{}[\]()#+\-.!|~<>]/;

/** Only https addresses become links. */
const safeUrl = (url: string) => (/^https:\/\/[^\s/@:]/.test(url) ? url : null);

/** The index of the `]` matching the `[` at `open`, or -1. */
const closingBracket = (text: string, open: number) => {
  let depth = 0;
  for (let at = open; at < text.length; at += 1) {
    if ((text[at] ?? '') === '\\') at += 1;
    else if ((text[at] ?? '') === '[') depth += 1;
    else if ((text[at] ?? '') === ']') {
      depth -= 1;
      if (depth === 0) return at;
    }
  }
  return -1;
};

const isWordChar = (char: string | undefined) => char !== undefined && /[\p{L}\p{N}]/u.test(char);

/** The inline pieces of a block's text. */
export const parseInline = (text: string): MarkdownInline[] => {
  const out: MarkdownInline[] = [];
  let plain = '';
  const flush = () => {
    if (plain) out.push({ kind: 'text', text: plain });
    plain = '';
  };
  let at = 0;
  while (at < text.length) {
    const char = text[at] ?? '';
    const rest = text.slice(at);
    if (char === '\\' && at + 1 < text.length && ESCAPABLE.test(text[at + 1] ?? '')) {
      plain += text[at + 1] ?? '';
      at += 2;
      continue;
    }
    if (char === '\n') {
      flush();
      out.push({ kind: 'break' });
      at += 1;
      continue;
    }
    if (char === '`') {
      const run = /^`+/.exec(rest)?.[0] ?? '`';
      const close = text.indexOf(run, at + run.length);
      if (close > 0) {
        flush();
        out.push({ kind: 'code', text: text.slice(at + run.length, close).trim() });
        at = close + run.length;
        continue;
      }
      plain += run;
      at += run.length;
      continue;
    }
    const breakTag = /^<br\s*\/?>/i.exec(rest);
    if (breakTag) {
      flush();
      out.push({ kind: 'break' });
      at += breakTag[0].length;
      continue;
    }
    const auto = /^<(https?:\/\/[^\s<>]+)>/.exec(rest);
    if (auto) {
      flush();
      out.push({
        kind: 'link',
        url: safeUrl(auto[1] ?? ''),
        children: [{ kind: 'text', text: auto[1] ?? '' }],
      });
      at += (auto[0] ?? '').length;
      continue;
    }
    if (char === 'h' && !isWordChar(text[at - 1] ?? '')) {
      const bare = BARE_URL.exec(rest);
      if (bare) {
        flush();
        out.push({
          kind: 'link',
          url: safeUrl(bare[0]),
          children: [{ kind: 'text', text: bare[0] }],
        });
        at += bare[0].length;
        continue;
      }
    }
    if (char === '[' || (char === '!' && (text[at + 1] ?? '') === '[')) {
      const open = char === '!' ? at + 1 : at;
      const close = closingBracket(text, open);
      const target =
        close > 0 ? /^\(\s*<?([^\s)>]*)>?(?:\s+"[^"]*")?\s*\)/.exec(text.slice(close + 1)) : null;
      if (target) {
        flush();
        // Pictures aren't fetched: their description links to them.
        out.push({
          kind: 'link',
          url: safeUrl(target[1] ?? ''),
          children: parseInline(text.slice(open + 1, close)),
        });
        at = close + 1 + (target[0] ?? '').length;
        continue;
      }
    }
    if (char === '*' || char === '_' || char === '~') {
      const double = (text[at + 1] ?? '') === char;
      const delimiter = double ? char + char : char;
      const kind = char === '~' ? 'del' : double ? 'strong' : 'em';
      const opensWord = !/\s/.test(text[at + delimiter.length] ?? ' ');
      const intraword = char === '_' && isWordChar(text[at - 1] ?? '');
      if ((char !== '~' || double) && opensWord && !intraword) {
        let close = text.indexOf(delimiter, at + delimiter.length + 1);
        while (
          close > 0 &&
          (/\s/.test(text[close - 1] ?? '') ||
            (char === '_' && isWordChar(text[close + delimiter.length] ?? '')) ||
            (!double && (text[close + 1] ?? '') === char))
        ) {
          close = text.indexOf(delimiter, close + (double ? 1 : 2));
        }
        if (close > 0) {
          flush();
          out.push({ kind, children: parseInline(text.slice(at + delimiter.length, close)) });
          at = close + delimiter.length;
          continue;
        }
      }
      plain += delimiter;
      at += delimiter.length;
      continue;
    }
    plain += char;
    at += 1;
  }
  flush();
  return out;
};
