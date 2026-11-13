import { FileDown } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import type { Medium } from '../manga/medium';
import { useNeedsApp } from '../shell/useNeedsApp';
import { TopBarAction } from '../shell/TopBar';
import { ExportSheet } from './ExportSheet';
import type { ExportChapter, ExportEpisode } from './ExportSheet';

/** A title page's Export action (M46.1). */
export const ExportButton: FC<{
  medium: Medium;
  titleIds: string[];
  chapters?: ExportChapter[];
  /** A video title's episodes, for seasons, ranges and picking episodes. */
  episodes?: ExportEpisode[];
}> = ({ medium, titleIds, chapters, episodes }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  // Exports write files through the app (docs/WEB.md).
  const needsApp = useNeedsApp();
  if (needsApp) return null;
  return (
    <>
      <TopBarAction label={t('export.title')} icon={FileDown} onClick={() => setOpen(true)} />
      {open && (
        <ExportSheet
          medium={medium}
          titleIds={titleIds}
          chapters={chapters}
          episodes={episodes}
          onClose={() => setOpen(false)}
        />
      )}
    </>
  );
};
