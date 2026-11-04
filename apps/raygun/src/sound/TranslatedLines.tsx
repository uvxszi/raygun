import type { Lyrics, TextAlign } from '@raygun/bindings';
import type { FC } from 'react';

import { lineTexts } from './lyricsTranslation';
import type { ShownTranslation } from './lyricsTranslation';

/**
 * Plain lyrics line by line, each with its romanized and translated words under it (M46.48),
 * sized like the player's plain lyrics (`.lyric-plain`).
 */
export const TranslatedPlain: FC<{
  lyrics: Lyrics;
  shown: ShownTranslation;
  variant: 'sheet' | 'panel';
  align: TextAlign;
}> = ({ lyrics, shown, variant, align }) => (
  <ol
    data-variant={variant}
    className={`lyric-plain flex flex-col gap-2 leading-relaxed ${
      align === 'center' ? 'text-center' : 'text-left'
    }`}
  >
    {lineTexts(lyrics).map((line, index) => {
      const romanized = shown.romanization?.lines[index];
      const translated = shown.translation?.lines[index];
      return (
        <li key={index} className={line.trim() ? '' : 'h-3'}>
          {line}
          {romanized && (
            <span
              data-testid="lyrics-romanized"
              className="block text-[0.75em] leading-snug font-medium"
            >
              {romanized}
            </span>
          )}
          {translated && (
            <span
              data-testid="lyrics-translation"
              className="block text-[0.75em] leading-snug opacity-80"
            >
              {translated}
            </span>
          )}
        </li>
      );
    })}
  </ol>
);
