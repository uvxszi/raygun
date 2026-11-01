import { Link } from '@tanstack/react-router';
import { Compass } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { MangaGrid } from './MangaGrid';
import { useMangaLibrary } from './useManga';

/** Library → Manga: the titles you added, with unread counts. */
export const MangaLibraryView: FC = () => {
  const { t } = useTranslation();
  const library = useMangaLibrary();

  return (
    <div data-testid="manga-library" className="flex flex-col gap-4">
      <Link
        to="/manga/browse"
        className="bg-surface flex items-center gap-2 self-start rounded-full px-4 py-2 text-sm"
      >
        <Compass aria-hidden size={16} className="text-primary" />
        {t('manga.browseSources')}
      </Link>
      {library.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {library.error.message}
        </p>
      )}
      {library.data?.length === 0 && (
        <p className="text-muted text-sm">{t('manga.emptyLibrary')}</p>
      )}
      {library.data && library.data.length > 0 && (
        <MangaGrid
          items={library.data.map((entry) => ({
            sourceId: entry.sourceId,
            id: entry.remoteId,
            title: entry.title,
            coverUrl: entry.coverUrl,
            unread: entry.unread,
          }))}
        />
      )}
    </div>
  );
};
