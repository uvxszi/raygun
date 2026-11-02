import type { LibraryDuplicate } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { useState } from 'react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { videoTitleLink } from '../watch/useVideoLibrary';
import { MangaCover } from './MangaCover';
import { ROUTES } from './medium';
import type { Medium } from './medium';
import { fetchDuplicates } from './useLibraryActions';

const duplicateLink = (medium: Medium, duplicate: LibraryDuplicate) =>
  medium === 'video'
    ? videoTitleLink(duplicate.sourceId, duplicate.remoteId)
    : { to: ROUTES[medium].title, search: { source: duplicate.sourceId, id: duplicate.remoteId } };

/**
 * Adding to the library asks first when a title with a similar name is already there (Mihon's
 * duplicate check): open that one instead, or add this one anyway.
 */
export const useDuplicateCheck = (medium: Medium) => {
  const { t } = useTranslation();
  const [found, setFound] = useState<{ duplicates: LibraryDuplicate[]; add: () => void } | null>(
    null,
  );
  const [checking, setChecking] = useState(false);

  /** Runs `add` straight away, or after the user saw the duplicates. */
  const addChecked = async (titleId: string, title: string, add: () => void) => {
    setChecking(true);
    try {
      const duplicates = await fetchDuplicates(medium, titleId, title);
      if (duplicates.length === 0) add();
      else setFound({ duplicates, add });
    } catch {
      // A failed check shouldn't stop adding.
      add();
    } finally {
      setChecking(false);
    }
  };

  const dialog: ReactNode = found && (
    <Sheet title={t('library.duplicates.title')} onClose={() => setFound(null)}>
      <div className="flex flex-col gap-3 text-sm">
        <p className="text-muted">{t('library.duplicates.body')}</p>
        <ul className="flex flex-col gap-2">
          {found.duplicates.map((duplicate) => (
            <li key={duplicate.titleId}>
              <Link
                {...duplicateLink(medium, duplicate)}
                onClick={() => setFound(null)}
                aria-label={t('library.duplicates.open', {
                  title: duplicate.title,
                  source: duplicate.sourceName,
                })}
                className="bg-background flex items-center gap-3 rounded-xl p-2"
              >
                <span className="w-10 shrink-0 overflow-hidden rounded-md">
                  <MangaCover
                    url={duplicate.coverUrl}
                    video={medium === 'video'}
                    className="w-full"
                  />
                </span>
                <span className="flex min-w-0 flex-col">
                  <span className="line-clamp-2 font-medium">{duplicate.title}</span>
                  <span className="text-muted text-xs">{duplicate.sourceName}</span>
                </span>
              </Link>
            </li>
          ))}
        </ul>
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={() => setFound(null)}
            className="rounded-full px-4 py-2.5 font-medium"
          >
            {t('library.duplicates.cancel')}
          </button>
          <button
            type="button"
            onClick={() => {
              found.add();
              setFound(null);
            }}
            className="bg-primary text-background rounded-full px-5 py-2.5 font-semibold"
          >
            {t('library.duplicates.addAnyway')}
          </button>
        </div>
      </div>
    </Sheet>
  );

  return { addChecked, dialog, checking };
};
