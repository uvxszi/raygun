import type { VideoUpdateItem } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { CalendarClock, CircleCheck, Play, Tv } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { groupByDay, useDayLabel } from '../manga/groupByDay';
import { MangaCover } from '../manga/MangaCover';
import { EmptyState } from '../shell/EmptyState';
import { episodeLabel } from './episodeLabel';
import { useEpisodePlayer } from './useEpisodePlayer';
import type { EpisodePlayer } from './useEpisodePlayer';
import { useVideoUpcoming, useVideoUpdates, videoTitleLink } from './useVideoLibrary';
import { Carousel } from '../shell/Carousel';
import { SourceIssue } from '../shell/SourceIssue';

/** A new episode: tap for its title, Play to watch it now. */
export const EpisodeUpdateRow: FC<{ item: VideoUpdateItem; player: EpisodePlayer }> = ({
  item,
  player,
}) => {
  const { t } = useTranslation();
  const link = videoTitleLink(item.sourceId, item.remoteId);
  const pending = player.pending(item);
  const error = player.error(item);
  const label = `${item.title} · ${episodeLabel(item)}`;
  return (
    <li className="flex flex-col">
      <div className="flex items-center gap-3">
        <Link
          to={link.to}
          search={link.search}
          data-testid="episode-update-row"
          className={`flex min-w-0 flex-1 items-center gap-3 py-2 ${item.watched ? 'opacity-50' : ''}`}
        >
          <MangaCover url={item.coverUrl} video className="w-10 shrink-0 rounded" />
          <span className="flex min-w-0 flex-1 flex-col">
            <span className="truncate text-sm font-medium">{item.title}</span>
            <span className="text-muted truncate text-xs">{episodeLabel(item)}</span>
          </span>
          {item.downloaded && (
            <CircleCheck
              role="img"
              aria-label={t('watch.downloaded')}
              size={18}
              className="text-primary shrink-0"
            />
          )}
        </Link>
        <button
          type="button"
          aria-label={t('watch.playEpisode', { name: label })}
          disabled={pending}
          onClick={() => player.play(item)}
          className="text-primary hover:bg-surface grid size-10 shrink-0 place-items-center rounded-full disabled:opacity-50"
        >
          <Play aria-hidden size={18} fill="currentColor" />
        </button>
      </div>
      {error && (
        <SourceIssue compact title={t('watch.playFailed')}>
          {error.message}
        </SourceIssue>
      )}
    </li>
  );
};

/** A card in the Upcoming strip: when it airs, the series and the episode. */
const UpcomingCard: FC<{ item: VideoUpdateItem; when: string }> = ({ item, when }) => {
  const link = videoTitleLink(item.sourceId, item.remoteId);
  return (
    <li aria-label={`${item.title} · ${episodeLabel(item)}`} className="w-28 shrink-0">
      <Link to={link.to} search={link.search} className="flex flex-col gap-1.5">
        <span className="relative overflow-hidden rounded-xl">
          <MangaCover url={item.coverUrl} video className="w-full" />
          <span className="bg-primary text-background absolute top-1.5 left-1.5 rounded-md px-1.5 py-0.5 text-[10px] font-bold">
            {when}
          </span>
        </span>
        <span className="line-clamp-1 text-xs font-semibold">{item.title}</span>
        <span className="text-muted line-clamp-1 text-xs">{episodeLabel(item)}</span>
      </Link>
    </li>
  );
};

/** What's coming up for followed series (CloudStream's schedule); nothing when nothing is. */
export const UpcomingStrip: FC = () => {
  const { t } = useTranslation();
  const upcoming = useVideoUpcoming();
  const dayLabel = useDayLabel();
  const soon = upcoming.data ?? [];
  if (soon.length === 0) return null;
  return (
    <section aria-labelledby="upcoming-heading" className="flex flex-col gap-2">
      <h2 id="upcoming-heading" className="flex items-center gap-2 font-semibold">
        <CalendarClock aria-hidden size={18} className="text-primary" />
        {t('watch.upcoming')}
      </h2>
      <Carousel className="-mx-4 gap-3 px-4 pb-1 lg:mx-0 lg:px-0">
        {soon.map((item) => (
          <UpcomingCard
            key={item.episodeId}
            item={item}
            when={dayLabel(new Date(item.date ?? 0).setHours(0, 0, 0, 0))}
          />
        ))}
      </Carousel>
    </section>
  );
};

/**
 * Updates → Episodes: what's coming up for followed series, then new episodes that library
 * updates found, by day (Aniyomi's updates).
 */
export const EpisodeUpdates: FC = () => {
  const { t } = useTranslation();
  const updates = useVideoUpdates();
  const upcoming = useVideoUpcoming();
  const player = useEpisodePlayer();
  const dayLabel = useDayLabel();
  const soon = upcoming.data ?? [];
  const error = updates.error ?? upcoming.error;

  return (
    <div data-testid="episode-updates" className="flex flex-col gap-5">
      {error && (
        <SourceIssue compact title={t('watch.updatesUnreadable')}>
          {error.message}
        </SourceIssue>
      )}
      <UpcomingStrip />
      {updates.data?.length === 0 && soon.length === 0 && (
        <EmptyState icon={Tv} title={t('watch.noEpisodeUpdates')} />
      )}
      {updates.data &&
        groupByDay(updates.data).map((group) => (
          <section key={group.day} className="flex flex-col gap-1">
            <h2 className="text-muted text-sm font-medium">{dayLabel(group.day)}</h2>
            <ul className="flex flex-col">
              {group.items.map((item) => (
                <EpisodeUpdateRow key={item.episodeId} item={item} player={player} />
              ))}
            </ul>
          </section>
        ))}
    </div>
  );
};
