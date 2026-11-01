import { Link } from '@tanstack/react-router';
import { BellDot } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { EmptyState } from '../shell/EmptyState';
import { DownloadButton } from './DownloadButton';
import { groupByDay, useDayLabel } from './groupByDay';
import { MangaCover } from './MangaCover';
import { useMangaUpdates } from './useManga';

/** Updates → Chapters: chapters that library updates found, by day. */
export const ChapterUpdates: FC = () => {
  const { t } = useTranslation();
  const updates = useMangaUpdates();
  const dayLabel = useDayLabel();

  return (
    <div data-testid="chapter-updates" className="flex flex-col gap-4">
      {updates.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {updates.error.message}
        </p>
      )}
      {updates.data?.length === 0 && (
        <EmptyState icon={BellDot} title={t('manga.noUpdates')} body={t('manga.noUpdatesHint')} />
      )}
      {updates.data &&
        groupByDay(updates.data).map((group) => (
          <section key={group.day} className="flex flex-col gap-1">
            <h2 className="text-muted text-sm font-medium">{dayLabel(group.day)}</h2>
            <ul className="flex flex-col">
              {group.items.map((item) => (
                <li key={item.chapter.id} className="flex items-center gap-3">
                  <Link
                    to="/manga/read"
                    search={{ title: item.titleId, chapter: item.chapter.id }}
                    data-testid="update-row"
                    className={`flex min-w-0 flex-1 items-center gap-3 py-2 ${item.chapter.read ? 'opacity-50' : ''}`}
                  >
                    <MangaCover url={item.coverUrl} className="w-10 shrink-0 rounded" />
                    <span className="flex min-w-0 flex-col">
                      <span className="truncate text-sm font-medium">{item.title}</span>
                      <span className="text-muted truncate text-xs">{item.chapter.name}</span>
                    </span>
                  </Link>
                  <DownloadButton chapter={item.chapter} />
                </li>
              ))}
            </ul>
          </section>
        ))}
    </div>
  );
};
