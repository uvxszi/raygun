import { RefreshCw } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useMangaUpdateStatus, useUpdateLibrary } from './useManga';

/** The button that starts a library update, showing progress while one runs. */
export const UpdateLibraryButton: FC = () => {
  const { t } = useTranslation();
  const status = useMangaUpdateStatus();
  const update = useUpdateLibrary();
  const running = status.data?.running ?? false;
  return (
    <button
      type="button"
      disabled={running || update.isPending}
      onClick={() => update.mutate()}
      className="bg-surface flex items-center gap-2 rounded-full px-4 py-2 text-sm disabled:opacity-80"
    >
      <RefreshCw
        aria-hidden
        size={16}
        className={`text-primary ${running ? 'animate-spin' : ''}`}
      />
      {running && status.data
        ? t('manga.updating', { checked: status.data.checked, total: status.data.total })
        : t('manga.updateLibrary')}
    </button>
  );
};
