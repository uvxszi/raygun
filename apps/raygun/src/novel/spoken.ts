import type { TableCell, TextBlock, TextPart } from '@raygun/bindings';

import { codeReadsAsWords } from './code';

/** A table block of a chapter. */
export type TableBlock = Extract<TextBlock, { kind: 'table' }>;

/** What a part says aloud: a formula its words, a picture nothing, the rest its text. */
const spokenOf = (part: TextPart) => {
  switch (part.kind) {
    case 'math':
      return ` ${part.speech} `;
    case 'image':
      return '';
    default:
      return part.text;
  }
};

/** A paragraph's parts as reading aloud says them: formulas in words. */
export const spokenParts = (parts: TextPart[]) =>
  parts.map(spokenOf).join('').replace(/\s+/g, ' ').trim();

/**
 * Where each part's characters are in the text `spokenParts` makes of them (UTF-16, as reading
 * aloud counts): a place for each character of the part's spoken form, or -1 for whitespace
 * that went (a run of it is one space, none at the start).
 */
export const spokenPlaces = (parts: TextPart[]): number[][] => {
  let length = 0;
  let afterSpace = true;
  return parts.map((part) => {
    const text = spokenOf(part);
    const places: number[] = [];
    for (let index = 0; index < text.length; index += 1) {
      const space = /\s/.test(text.charAt(index));
      if (space && afterSpace) {
        places.push(-1);
        continue;
      }
      afterSpace = space;
      places.push(length);
      length += 1;
    }
    return places;
  });
};

/** The characters of a part (from, to) whose places lie in `range`, or `null` for none. */
export const markedIn = (
  places: number[],
  range: { start: number; end: number },
): { from: number; to: number } | null => {
  let from = -1;
  let to = -1;
  places.forEach((place, index) => {
    if (place < range.start || place >= range.end) return;
    if (from < 0) from = index;
    to = index + 1;
  });
  return from < 0 ? null : { from, to };
};

/** Ends a sentence read aloud with a full stop unless it ends with its own. */
const sentence = (text: string) => (/[.!?…:;]$/.test(text) ? text : `${text}.`);

/**
 * A table as reading aloud says it: its caption, then each row as a sentence, a cell after the
 * header of its column ("Level: 0") when the table's first rows name the columns.
 */
export const tableSpeech = (block: TableBlock) => {
  const said = (cell: TableCell) => spokenParts(cell.parts);
  const firstBody = block.rows.findIndex((row) => row.some((cell) => !cell.header));
  const heads = firstBody < 0 ? [] : block.rows.slice(0, firstBody);
  const body = firstBody < 0 ? block.rows : block.rows.slice(firstBody);
  // The last header row names the columns: a header across two columns names both.
  const labels: string[] = [];
  let column = 0;
  for (const cell of heads.at(-1) ?? []) {
    const label = said(cell);
    for (let span = 0; span < (cell.colspan ?? 1); span += 1) labels[column + span] = label;
    column += cell.colspan ?? 1;
  }
  const sentences: string[] = [];
  const caption = block.caption ? spokenParts(block.caption) : '';
  if (caption) sentences.push(sentence(caption));
  // Cells that span rows take their column in the rows below too.
  const taken: number[] = [];
  for (const row of body) {
    const words: string[] = [];
    column = 0;
    for (const cell of row) {
      while ((taken[column] ?? 0) > 0) column += 1;
      const text = said(cell);
      const label = labels[column];
      if (text) words.push(!cell.header && label && label !== text ? `${label}: ${text}` : text);
      for (let span = 0; span < (cell.colspan ?? 1); span += 1) {
        taken[column + span] = cell.rowspan ?? 1;
      }
      column += cell.colspan ?? 1;
    }
    taken.forEach((rows, index) => {
      taken[index] = Math.max(0, rows - 1);
    });
    if (words.length > 0) sentences.push(sentence(words.join(', ')));
  }
  return sentences.join(' ');
};

/**
 * What reading aloud says for a block, or `null` for one it skips (pictures, rules, code). A
 * list's marks aren't said, so a sentence's place in the text is its place on screen.
 */
export const spokenText = (block: TextBlock): string | null => {
  switch (block.kind) {
    case 'paragraph':
      return block.parts ? spokenParts(block.parts) : block.text;
    case 'heading':
      return block.text;
    case 'math':
      return block.speech;
    case 'code':
      // Verse set out line by line is read; code would be read sign by sign.
      return codeReadsAsWords(block) ? block.text : null;
    case 'table':
      return tableSpeech(block) || null;
    default:
      return null;
  }
};
