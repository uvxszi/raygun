import { commands } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { BookOpen, ChevronRight, EyeOff } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { unwrapResult } from '../lib/commandResult';
import { useSettings } from '../settings/useSettings';
import { Sheet } from '../shell/Sheet';
import type { Medium } from './medium';

/** Library titles Rust leaves out while 18+ content is off (`library_hidden`); none while on. */
const useHiddenEntries =(medium: Medium) => {
  const showNsfw = useSettings().data?.showNsfw;
  return useQuery({
    queryKey: [medium, 'library', 'hidden', showNsfw],
    queryFn: () => unwrapResult(commands.libraryHidden(medium)),
    enabled: showNsfw === false,
  });
};

/**
 * A quiet "3 hidden entries" under a library's top bar while 18+ content is off and the library
 * holds titles it hides; a tap lists them and says why, with the way to the setting.
 */
export const HiddenEntries: FC<{ medium: Medium }> = ({ medium }) => {
  const { t } = useTranslation();
  const hidden = useHiddenEntries(medium);
  const [open, setOpen] = useState(false);
  const entries = hidden.data ?? [];
  if (entries.length === 0) return null;
  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        className="text-muted hover:text-foreground flex items-center gap-1.5 self-start text-xs"
      >
        <EyeOff aria-hidden size={14} />
        {t('library.hiddenEntries.note', { count: entries.length })}
      </button>
      {open && (
        <Sheet title={t('library.hiddenEntries.title')} onClose={() => setOpen(false)}>
          <div className="flex flex-col gap-3">
            <p className="text-muted text-sm">{t('library.hiddenEntries.body')}</p>
            <ul aria-label={t('library.hiddenEntries.title')} className="flex flex-col gap-2">
              {entries.map((entry) => (
                <li key={entry.titleId} className="flex items-center gap-3">
                  <Artwork
                    url={entry.coverUrl}
                    icon={BookOpen}
                    iconSize={18}
                    className="h-14 w-10 shrink-0 rounded-md"
                  />
                  <span className="flex min-w-0 flex-col">
                    <span className="truncate text-sm font-medium">{entry.title}</span>
                    <span className="text-muted truncate text-xs">{entry.sourceName}</span>
                  </span>
                </li>
              ))}
            </ul>
            <Link
              to="/settings/browse"
              onClick={() => setOpen(false)}
              className="bg-surface flex items-center justify-between rounded-xl px-4 py-3 text-sm"
            >
              {t('library.hiddenEntries.setting')}
              <ChevronRight aria-hidden size={16} className="text-muted" />
            </Link>
          </div>
        </Sheet>
      )}
    </>
  );
};
