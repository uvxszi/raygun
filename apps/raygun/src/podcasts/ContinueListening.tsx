import { commands } from '@raygun/bindings';
import type { ListeningItem } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { BookmarkMinus, CheckCheck, Download, EyeOff, Pin, Play, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ContinuePickBar } from '../home/ContinuePickBar';
import type { PickAction } from '../home/ContinuePickBar';
import { unwrapResult } from '../lib/commandResult';
import { PickTarget } from '../select/PickTarget';
import { usePickList } from '../select/usePickList';
import type { PickState } from '../select/usePickPress';

import { usePlayAudiobook } from '../audiobooks/useAudiobooks';
import { Carousel } from '../shell/Carousel';
import { SourceIssue } from '../shell/SourceIssue';
import { formatLength } from './formatLength';
import { PodcastArt } from './PodcastArt';
import {
  useContinueListening,
  useForgetPosition,
  usePlayEpisode,
  usePodcastEvents,
} from './usePodcasts';
import type { MenuItem } from '../shell/contextMenuStore';
import { MenuButton } from '../shell/MenuButton';

/** Entries shown on Home. */
const SHOWN = 12;

const itemKey = (item: ListeningItem) => `${item.podcastId}:${item.episodeId}`;

const Card: FC<{ item: ListeningItem; pick: PickState }> = ({ item, pick }) => {
  const { t } = useTranslation();
  const playEpisode = usePlayEpisode();
  const playBook = usePlayAudiobook();
  const forget = useForgetPosition();
  const book = item.kind === 'audiobook';
  const duration = item.durationMs ?? 0;
  const position = item.positionMs ?? 0;
  const left = duration > 0 ? Math.max(0, duration - position) : 0;
  const play = () =>
    book
      ? playBook.mutate({ id: item.podcastId, chapterId: null })
      : playEpisode.mutate(item.episodeId);
  const error = playEpisode.error ?? playBook.error ?? forget.error;
  return (
    <li aria-label={`${item.show}: ${item.title}`} className="relative w-36 shrink-0 sm:w-40">
      <PickTarget
        pick={pick}
        label={t('bulk.pick', { name: `${item.show}: ${item.title}` })}
        testId="listening-pick"
      >
        <button
          type="button"
          onClick={play}
          data-testid="listening-card"
          className="flex w-full flex-col gap-1.5 text-left"
        >
          <span className="relative overflow-hidden rounded-xl">
            <PodcastArt url={item.artworkUrl} className="w-full" />
            {duration > 0 && (
              <span aria-hidden className="absolute inset-x-0 bottom-0 h-1 bg-black/40">
                <span
                  className="bg-primary block h-full origin-left"
                  style={{ transform: `scaleX(${Math.min(1, position / duration)})` }}
                />
              </span>
            )}
            <span className="absolute inset-0 grid place-items-center">
              <span className="grid size-11 place-items-center rounded-full bg-black/55 text-white">
                <Play aria-hidden size={20} fill="currentColor" />
              </span>
            </span>
          </span>
          <span className="line-clamp-2 text-sm font-medium">{item.show}</span>
          <span className="text-muted line-clamp-1 text-xs">
            {[
              book && item.chapter && item.chapters
                ? t('audiobooks.chapterOf', { chapter: item.chapter, count: item.chapters })
                : item.title,
              left > 0 ? t('podcasts.left', { time: formatLength(left, t) }) : null,
            ]
              .filter(Boolean)
              .join(' · ')}
          </span>
        </button>
      </PickTarget>
      {!book && !pick.selecting && (
        <button
          type="button"
          aria-label={t('podcasts.removeContinue', { title: item.title })}
          onClick={() => forget.mutate(item.episodeId)}
          className="absolute top-1.5 right-1.5 grid size-8 place-items-center rounded-full bg-black/55 text-white"
        >
          <X aria-hidden size={15} />
        </button>
      )}
      {error && <SourceIssue compact>{error.message}</SourceIssue>}
    </li>
  );
};

/**
 * Home's Continue listening (M46.48): podcast episodes and audiobooks started and not finished,
 * the last listened first; nothing when there are none. `menu` puts a ⋯ beside its heading.
 */
export const ContinueListening: FC<{ menu?: () => MenuItem[] }> = ({ menu }) => {
  const { t } = useTranslation();
  usePodcastEvents();
  const client = useQueryClient();
  const listening = useContinueListening(SHOWN);
  const items = listening.data ?? [];
  const picks = usePickList(items.map(itemKey));
  if (!items.length) return null;

  const picked = items.filter((item) => picks.has(itemKey(item)));
  const count = picked.length;
  // A book stands for itself (its chapters); an episode for itself.
  const books = picked.filter((item) => item.kind === 'audiobook').map((item) => item.podcastId);
  const episodes = picked.filter((item) => item.kind !== 'audiobook').map((item) => item.episodeId);
  const shows = [...new Set(picked.map((item) => item.podcastId))];
  const done = async <T,>(work: Promise<T>) => {
    try {
      return await work;
    } finally {
      await Promise.all([
        client.invalidateQueries({ queryKey: ['podcasts'] }),
        client.invalidateQueries({ queryKey: ['audiobooks'] }),
      ]);
    }
  };
  const actions: PickAction[] = [
    {
      label: t('home.continue.remove'),
      icon: EyeOff,
      run: async () => {
        await done(
          unwrapResult(
            commands.continueHide(
              picked.map(
                (item) =>
                  `listening:${item.kind === 'audiobook' ? item.podcastId : item.episodeId}`,
              ),
            ),
          ),
        );
        return t('home.continue.removed', { count });
      },
    },
    {
      label: t('home.continue.markPlayed'),
      icon: CheckCheck,
      run: async () => {
        await done(unwrapResult(commands.podcastSetPlayedPicks({ shows: books, episodes }, true)));
        return t('home.continue.markedPlayed', { count });
      },
    },
    {
      label: t('library.select.download'),
      icon: Download,
      run: async () => {
        const queued = await done(
          unwrapResult(commands.podcastDownloadPicks({ shows: books, episodes })),
        );
        return queued > 0
          ? t('search.select.queued', { count: queued })
          : t('search.select.nothingQueued');
      },
    },
    {
      label: t('rail.pin'),
      icon: Pin,
      run: async () => {
        await done(unwrapResult(commands.podcastPinMany(shows, true)));
        return t('home.continue.pinned', { count: shows.length });
      },
    },
    {
      label: t('menu.removeFromLibrary'),
      icon: BookmarkMinus,
      confirm: t('home.continue.removeShowsAsk', { count: shows.length }),
      run: async () => {
        await done(unwrapResult(commands.podcastSetSubscribedMany(shows, false)));
        return t('home.continue.removedFromLibrary', { count: shows.length });
      },
    },
  ];
  return (
    <section
      aria-labelledby="continue-listening"
      data-testid="continue-listening"
      className="flex flex-col gap-2"
    >
      <div className="flex min-h-9 items-center gap-3">
        <h2 id="continue-listening" className="text-lg font-semibold">
          {t('podcasts.continueListening')}
        </h2>
        {menu && (
          <MenuButton
            horizontal
            label={t('menu.more', { name: t('podcasts.continueListening') })}
            items={menu}
            className="text-muted hover:text-foreground hover:bg-surface ml-auto grid size-8 place-items-center rounded-full"
          />
        )}
      </div>
      {picks.selecting && (
        <ContinuePickBar
          label={t('home.continue.actions', { name: t('podcasts.continueListening') })}
          count={count}
          allPicked={count === items.length}
          onSelectAll={picks.selectAll}
          onClose={picks.clear}
          actions={actions}
        />
      )}
      <Carousel className="-mx-4 gap-3 px-4 lg:mx-0 lg:px-0">
        {items.map((item) => (
          <Card key={itemKey(item)} item={item} pick={picks.state(itemKey(item))} />
        ))}
      </Carousel>
    </section>
  );
};
