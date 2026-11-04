import { Fragment, useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useOpenPodcastPage } from './usePodcasts';

/** Web addresses in notes, without the punctuation that ends a sentence after them. */
const ADDRESS = /https?:\/\/[^\s<>"'()[\]]+[^\s<>"'()[\].,;:!?]/g;

type Part = { text: string; url?: string };

const parts = (text: string): Part[] => {
  const found: Part[] = [];
  let last = 0;
  for (const match of text.matchAll(ADDRESS)) {
    const at = match.index ?? 0;
    if (at > last) found.push({ text: text.slice(last, at) });
    found.push({ text: match[0], url: match[0] });
    last = at + match[0].length;
  }
  if (last < text.length) found.push({ text: text.slice(last) });
  return found;
};

/**
 * A show's, episode's or book's notes (M46.48): selectable text whose web addresses open in
 * Raygun's browser. `clamp` shows the first lines until More.
 */
export const ShowNotes: FC<{ text: string; clamp?: boolean; className?: string }> = ({
  text,
  clamp = false,
  className = '',
}) => {
  const { t } = useTranslation();
  const open = useOpenPodcastPage();
  const [expanded, setExpanded] = useState(false);
  const pieces = useMemo(() => parts(text), [text]);
  const long = clamp && (text.length > 240 || text.split('\n').length > 3);
  return (
    <div className={`flex flex-col items-start gap-1 ${className}`}>
      <p
        className={`text-muted selectable text-sm whitespace-pre-line wrap-anywhere ${long && !expanded ? 'line-clamp-3' : ''}`}
      >
        {pieces.map((piece, index) =>
          piece.url ? (
            <a
              key={index}
              href={piece.url}
              onClick={(event) => {
                event.preventDefault();
                event.stopPropagation();
                if (piece.url) open.mutate(piece.url);
              }}
              className="text-primary underline"
            >
              {piece.text}
            </a>
          ) : (
            <Fragment key={index}>{piece.text}</Fragment>
          ),
        )}
      </p>
      {long && (
        <button
          type="button"
          aria-expanded={expanded}
          onClick={() => setExpanded(!expanded)}
          className="text-foreground text-xs font-semibold hover:underline"
        >
          {t(expanded ? 'podcasts.notesLess' : 'podcasts.notesMore')}
        </button>
      )}
    </div>
  );
};
