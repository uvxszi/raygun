import { Link } from '@tanstack/react-router';
import { Bell, Compass, Download } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { MangaGrid } from './MangaGrid';
import { ROUTES } from './medium';
import type { Medium } from './medium';
import { UpdateLibraryButton } from './UpdateLibraryButton';
import { useMangaLibrary, useMangaLiveEvents } from './useManga';

const CHIP = 'bg-surface flex items-center gap-2 rounded-full px-4 py-2 text-sm';

/** Library → Manga or Novels: the titles you added, with unread counts. */
export const MangaLibraryView: FC<{ medium?: Medium }> = ({ medium = 'manga' }) => {
  const { t } = useTranslation();
  useMangaLiveEvents();
  const library = useMangaLibrary(medium);

  return (
    <div data-testid={`${medium}-library`} className="flex flex-col gap-4">
      <nav aria-label={t('manga.libraryActions')} className="flex flex-wrap gap-2">
        <Link to={ROUTES[medium].browse} className={CHIP}>
          <Compass aria-hidden size={16} className="text-primary" />
          {t('manga.browseSources')}
        </Link>
        {medium === 'manga' && (
          <>
            <Link to="/manga/updates" className={CHIP}>
              <Bell aria-hidden size={16} className="text-primary" />
              {t('manga.updates')}
            </Link>
            <Link to="/manga/downloads" className={CHIP}>
              <Download aria-hidden size={16} className="text-primary" />
              {t('manga.downloads')}
            </Link>
            <UpdateLibraryButton />
          </>
        )}
      </nav>
      {library.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {library.error.message}
        </p>
      )}
      {library.data?.length === 0 && (
        <p className="text-muted text-sm">
          {t(medium === 'novel' ? 'novel.emptyLibrary' : 'manga.emptyLibrary')}
        </p>
      )}
      {library.data && library.data.length > 0 && (
        <MangaGrid
          medium={medium}
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
