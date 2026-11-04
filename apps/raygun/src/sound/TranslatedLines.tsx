import type { Lyrics, TextAlign } from '@raygun/bindings';
import { memo } from 'react';
import type { FC } from 'react';

import { lineTexts } from './lyricsTranslation';
import type { ShownTranslation } from './lyricsTranslation';

/** The romanized and translated words under one lyrics line, smaller than the line. */
export const TranslatedLine: FC<{ shown: ShownTranslation; index: number }> = ({
  shown,
  index,
}) => (
  <SubLines
    romanized={shown.romanization?.lines[index] ?? ''}
    translated={shown.translation?.lines[index] ?? ''}
  />
);

const SubLines = memo(({ romanized, translated }: { romanized: string; translated: string }) => (
  <>
    {romanized && (
      <span data-testid="lyrics-romanized" className="block text-[0.62em] leading-snug font-medium">
        {romanized}
      </span>
    )}
    {translated && (
      <span
        data-testid="lyrics-translation"
        className="block text-[0.62em] leading-snug font-normal opacity-80"
      >
        {translated}
      </span>
    )}
  </>
));
SubLines.displayName = 'SubLines';

/** Plain lyrics line by line, each with its translation under it. */
export const TranslatedPlain: FC<{
  lyrics: Lyrics;
  shown: ShownTranslation;
  size: number;
  align: TextAlign;
  compact: boolean;
}> = ({ lyrics, shown, size, align, compact }) => (
  <ol
    style={size > 0 ? { fontSize: Math.round(size * 0.75) } : undefined}
    className={`flex flex-col gap-2 leading-relaxed ${align === 'center' ? 'text-center' : 'text-left'} ${compact ? 'text-base' : 'text-lg md:text-xl'}`}
  >
    {lineTexts(lyrics).map((line, index) => (
      <li key={index} className={line.trim() ? '' : 'h-3'}>
        {line}
        <TranslatedLine shown={shown} index={index} />
      </li>
    ))}
  </ol>
);
