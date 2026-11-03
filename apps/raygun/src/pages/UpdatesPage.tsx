import { commands } from '@raygun/bindings';
import type { ArtistReleaseNews, MangaUpdateItem, VideoUpdateItem } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useSearch } from '@tanstack/react-router';
import { BellDot, RefreshCw } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { ChapterUpdateRow, ChapterUpdates } from '../manga/ChapterUpdates';
import { groupByDay, useDayLabel } from '../manga/groupByDay';
import {
  useMangaLiveEvents,
  useMangaUpdateStatus,
  useMangaUpdates,
  useUpdateLibrary,
} from '../manga/useManga';
import { useReleaseNews } from '../music/follows';
import { EmptyState, Loading } from '../shell/EmptyState';
import { LoadMore } from '../shell/LoadMore';
import { SourceIssue } from '../shell/SourceIssue';
import { Tabs } from '../shell/Tabs';
import { TopBarAction } from '../shell/TopBar';
import { EpisodeUpdateRow, EpisodeUpdates, UpcomingStrip } from '../watch/EpisodeUpdates';
import { useEpisodePlayer } from '../watch/useEpisodePlayer';
import { useVideoUpcoming, useVideoUpdates } from '../watch/useVideoLibrary';
import { ReleaseNews, ReleaseRow } from '../music/ReleaseNews';
import { PageHeader } from './PageHeader';

export type UpdatesTab = 'all' | 'chapters' | 'episodes' | 'releases';

/** Rows the All tab shows at first, and how many more each time its end comes near. */
const PAGE = 80;

type Entry =
  | { kind: 'chapter'; key: string; at: number; item: MangaUpdateItem }
  | { kind: 'episode'; key: string; at: number; item: VideoUpdateItem }
  | { kind: 'release'; key: string; at: number; item: ArtistReleaseNews };

/** Followed artists checked now, beside the library update. */
const useCheckReleases = () => {
  const client = useQueryClient();
  return useMutation({
    mutationFn: () => unwrapResult(commands.artistReleasesCheck()),
    onSuccess: (news) => client.setQueryData(['music', 'releaseNews'], news),
  });
};

/**
 * Updates → All: new chapters (comics and books), episodes and releases as one list by day,
 * newest first, with what's coming up for followed series above it.
 */
const AllUpdates: FC = () => {
  const { t } = useTranslation();
  const chapters = useMangaUpdates();
  const episodes = useVideoUpdates();
  const upcoming = useVideoUpcoming();
  const news = useReleaseNews();
  const player = useEpisodePlayer();
  const dayLabel = useDayLabel();
  const [limit, setLimit] = useState(PAGE);
  const entries = useMemo<Entry[]>(
    () =>
      [
        ...(chapters.data ?? []).map((item): Entry => ({
          kind: 'chapter',
          key: `c-${item.chapter.id}`,
          at: item.foundAt ?? 0,
          item,
        })),
        ...(episodes.data ?? []).map((item): Entry => ({
          kind: 'episode',
          key: `e-${item.episodeId}`,
          at: item.foundAt ?? 0,
          item,
        })),
        ...(news.data ?? []).map((item): Entry => ({
          kind: 'release',
          key: `r-${item.album.provider.id}:${item.album.id}`,
          at: item.foundAt ?? 0,
          item,
        })),
      ].sort((a, b) => b.at - a.at),
    [chapters.data, episodes.data, news.data],
  );
  const queries = [chapters, episodes, upcoming, news];
  const failed = queries.find((query) => query.error)?.error;
  if (!queries.every((query) => query.data !== undefined || query.isError)) {
    return <Loading label={t('manga.loading')} />;
  }
  if (entries.length === 0 && (upcoming.data?.length ?? 0) === 0) {
    return (
      <>
        {failed && (
          <SourceIssue compact title={t('manga.updatesUnreadable')}>
            {failed.message}
          </SourceIssue>
        )}
        <EmptyState icon={BellDot} title={t('updates.noneTitle')} />
      </>
    );
  }
  return (
    <div data-testid="updates-all" className="flex flex-col gap-5">
      {failed && (
        <SourceIssue compact title={t('manga.updatesUnreadable')}>
          {failed.message}
        </SourceIssue>
      )}
      <UpcomingStrip />
      {groupByDay(entries.slice(0, limit), (entry) => entry.at).map((group) => (
        <section key={group.day} className="flex flex-col gap-1">
          <h2 className="text-muted text-sm font-medium">{dayLabel(group.day)}</h2>
          <ul className="flex flex-col">
            {group.items.map((entry) =>
              entry.kind === 'chapter' ? (
                <ChapterUpdateRow key={entry.key} item={entry.item} />
              ) : entry.kind === 'episode' ? (
                <EpisodeUpdateRow key={entry.key} item={entry.item} player={player} />
              ) : (
                <ReleaseRow key={entry.key} item={entry.item} />
              ),
            )}
          </ul>
        </section>
      ))}
      <LoadMore
        hasNextPage={entries.length > limit}
        fetching={false}
        onLoad={() => setLimit((shown) => shown + PAGE)}
      />
    </div>
  );
};

/**
 * Updates (More → Updates): what library updates and followed artists brought, all together and
 * by kind. One check covers everything, so its progress and the refresh action sit above the
 * tabs.
 */
export const UpdatesPage: FC = () => {
  const { t, i18n } = useTranslation();
  // All first (M46.37): chapters, episodes and releases together.
  const { tab = 'all' } = useSearch({ from: '/updates' });
  useMangaLiveEvents();
  const status = useMangaUpdateStatus();
  const update = useUpdateLibrary();
  const releases = useCheckReleases();
  const running = status.data?.running ?? false;
  const lastRun = status.data?.lastRunAt;
  const finished = status.data && !status.data.running && lastRun ? status.data : null;
  const check = () => {
    if (!running) update.mutate();
    releases.mutate();
  };

  return (
    <section data-testid="updates-page" className="flex flex-col gap-4">
      <PageHeader
        back
        fallback="/more"
        title={t('manga.updates')}
        actions={
          <TopBarAction
            label={t('updates.check')}
            icon={RefreshCw}
            disabled={running || update.isPending || releases.isPending}
            onClick={check}
          />
        }
      />
      <Tabs
        label={t('manga.updates')}
        active={tab}
        items={[
          { id: 'all', label: t('library.tabs.all'), to: '/updates', search: {} },
          {
            id: 'chapters',
            label: t('updates.chapters'),
            to: '/updates',
            search: { tab: 'chapters' },
          },
          {
            id: 'episodes',
            label: t('updates.episodes'),
            to: '/updates',
            search: { tab: 'episodes' },
          },
          {
            id: 'releases',
            label: t('updates.releases'),
            to: '/updates',
            search: { tab: 'releases' },
          },
        ]}
      />
      {(running || releases.isPending || finished?.lastRunAt) && (
        <div className="flex flex-wrap items-center gap-3">
          {running && status.data && (
            <p role="status" className="text-muted flex items-center gap-2 text-sm">
              <RefreshCw aria-hidden size={14} className="text-primary animate-spin" />
              {t('manga.updating', { checked: status.data.checked, total: status.data.total })}
            </p>
          )}
          {!running && releases.isPending && (
            <p role="status" className="text-muted flex items-center gap-2 text-sm">
              <RefreshCw aria-hidden size={14} className="text-primary animate-spin" />
              {t('updates.checkingReleases')}
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
      )}
      {(update.error ?? releases.error) && (
        <SourceIssue compact title={t('updates.checkFailed')}>
          {(update.error ?? releases.error)?.message}
        </SourceIssue>
      )}
      {tab === 'all' && <AllUpdates />}
      {tab === 'chapters' && <ChapterUpdates />}
      {tab === 'episodes' && <EpisodeUpdates />}
      {tab === 'releases' && <ReleaseNews />}
    </section>
  );
};
