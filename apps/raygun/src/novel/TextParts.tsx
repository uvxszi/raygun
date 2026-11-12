import type { TextPart } from '@raygun/bindings';
import { Fragment } from 'react';
import type { FC, ReactNode } from 'react';

import { MathFormula } from './MathFormula';
import { markedIn, spokenPlaces } from './spoken';
import './bookText.css';

/** The sentence being read aloud: UTF-16 offsets into its paragraph's spoken text. */
export type SpokenRange = { start: number; end: number };

const SpokenMark: FC<{ children: ReactNode }> = ({ children }) => (
  <mark
    data-testid="spoken-sentence"
    data-spoken=""
    className="bg-primary/25 rounded-sm text-current"
  >
    {children}
  </mark>
);

/** Text with the characters `from`–`to` marked as being read aloud. */
const withMark = (text: string, mark: { from: number; to: number } | null): ReactNode =>
  mark ? (
    <>
      {text.slice(0, mark.from)}
      <SpokenMark>{text.slice(mark.from, mark.to)}</SpokenMark>
      {text.slice(mark.to)}
    </>
  ) : (
    text
  );

/** A plain paragraph's text with the sentence being read aloud marked. */
export const SpokenText: FC<{ text: string; sentence?: SpokenRange }> = ({ text, sentence }) => {
  if (!sentence || sentence.end <= sentence.start || sentence.end > text.length) return text;
  return withMark(text, { from: sentence.start, to: sentence.end });
};

const PartView: FC<{ part: TextPart; mark: { from: number; to: number } | null }> = ({
  part,
  mark,
}) => {
  switch (part.kind) {
    case 'sup':
      return <sup>{withMark(part.text, mark)}</sup>;
    case 'sub':
      return <sub>{withMark(part.text, mark)}</sub>;
    case 'math': {
      const formula = <MathFormula mathml={part.mathml} text={part.text} speech={part.speech} />;
      return mark ? <SpokenMark>{formula}</SpokenMark> : formula;
    }
    case 'styled': {
      let content = withMark(part.text, mark);
      if (part.code) content = <code className="book-inline-code">{content}</code>;
      if (part.italic) content = <em>{content}</em>;
      if (part.bold) content = <strong>{content}</strong>;
      return content;
    }
    case 'image':
      return <img src={part.url} alt="" loading="lazy" className="book-part-image" />;
    default:
      return withMark(part.text, mark);
  }
};

/**
 * Text in parts: raised and lowered text, formulas in the line, text in italics, bold or code,
 * a table cell's pictures (its line breaks kept by its style). With `sentence`, the part of it
 * being read aloud is marked, in places of the parts' spoken text.
 */
export const TextParts: FC<{ parts: TextPart[]; sentence?: SpokenRange }> = ({
  parts,
  sentence,
}) => {
  const places = sentence && sentence.end > sentence.start ? spokenPlaces(parts) : null;
  return (
    <>
      {parts.map((part, index) => (
        <Fragment key={index}>
          <PartView
            part={part}
            mark={places && sentence ? markedIn(places[index] ?? [], sentence) : null}
          />
        </Fragment>
      ))}
    </>
  );
};
