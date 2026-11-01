import { Link } from '@tanstack/react-router';
import { RefreshCw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { DownloadButton } from '../manga/DownloadButton';
import { DAY_MS, dayStart, groupByDay } from '../manga/groupByDay';
import { MangaCover } from '../manga/MangaCover';
import {
  useMangaLiveEvents,
  useMangaUpdateStatus,
  useMangaUpdates,
  useUpdateLibrary,
} from '../manga/useManga';
import { TopBarAction } from '../shell/TopBar';
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
  const update = useUpdateLibrary();
  const running = status.data?.running ?? false;
  const lastRun = status.data?.lastRunAt;
  const finished = status.data && !status.data.running && lastRun ? status.data : null;

  return (
    <section data-testid="manga-updates-page" className="flex flex-col gap-4">
      <PageHeader
        back
        fallback="/more"
        title={t('manga.updates')}
        actions={
          <TopBarAction
            label={t('manga.updateLibrary')}
            icon={RefreshCw}
            disabled={running || update.isPending}
            onClick={() => update.mutate()}
          />
        }
      />
      <div className="flex flex-wrap items-center gap-3">
        {running && status.data && (
          <p role="status" className="text-muted flex items-center gap-2 text-sm">
            <RefreshCw aria-hidden size={14} className="text-primary animate-spin" />
            {t('manga.updating', { checked: status.data.checked, total: status.data.total })}
          </p>
        )}
        {finished?.lastRunAt && (
          <p className="text-muted text-sm italic">
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
