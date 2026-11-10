import { useState } from 'react';
import type { FC, ReactNode } from 'react';

import { ReaderAiSheet } from '../../readerAi/ReaderAi';
import type { ReaderMedium } from '../../readerAi/useReaderAi';
import { PrefAction } from '../prefs';

/** A row that opens a reader's AI layers, as the reader's own AI button does. */
export const ReaderLayersRow: FC<{ medium: ReaderMedium; title: string; summary?: ReactNode }> = ({
  medium,
  title,
  summary,
}) => {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  return (
    <>
      <PrefAction title={title} summary={summary} dialog onClick={setAnchor} />
      {anchor && (
        <ReaderAiSheet
          medium={medium}
          anchor={anchor}
          error={null}
          onClose={() => setAnchor(null)}
        />
      )}
    </>
  );
};
