import { Link } from '@tanstack/react-router';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { DownloadButton } from '../manga/DownloadButton';
import { DAY_MS, dayStart, groupByDay } from '../manga/groupByDay';
import { MangaCover } from '../manga/MangaCover';
import { UpdateLibraryButton } from '../manga/UpdateLibraryButton';
import { useMangaLiveEvents, useMangaUpdateStatus, useMangaUpdates } from '../manga/useManga';
import { PageHeader } from './PageHeader';

/** Chapters that library updates found, by day. */
export const MangaUpdatesPage: FC = () => {
  const { t, i18n } = useTranslation();
  useMangaLiveEvents();
  const updates = useMangaUpdates();
  const status = useMangaUpdateStatus();
  const [today] = useState(() => dayStart(Date.now()));
  const dayLabel = (day: number) => {
    if (day === today) return t('manga.today');
    if (day === today - DAY_MS) return t('manga.yesterday');
    return new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium' }).format(day);
  };
  const lastRun = status.data?.lastRunAt;
  const finished = status.data && !status.data.running && lastRun ? status.data : null;

  return (
    <section data-testid="manga-updates-page" className="flex flex-col gap-4">
      <PageHeader title={t('manga.updates')} />
      <div className="flex flex-wrap items-center gap-3">
        <UpdateLibraryButton />
        {finished?.lastRunAt && (
          <p className="text-muted text-xs">
            {t('manga.lastUpdate', {
              when: new Intl.DateTimeFormat(i18n.language, {
                dateStyle: 'medium',
                timeStyle: 'short',
              }).format(finished.lastRunAt),
            })}
            {finished.failed > 0 && ` · ${t('manga.updateFailures', { count: finished.failed })}`}
          </p>
        )}
      </div>
      {updates.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {updates.error.message}
        </p>
      )}
      {updates.data?.length === 0 && <p className="text-muted text-sm">{t('manga.noUpdates')}</p>}
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
    </section>
  );
};
