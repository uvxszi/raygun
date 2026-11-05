import type { ListeningItem } from '@raygun/bindings';
import { Play, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

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

const Card: FC<{ item: ListeningItem }> = ({ item }) => {
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
      {!book && (
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
  const listening = useContinueListening(SHOWN);
  if (!listening.data?.length) return null;
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
      <Carousel className="-mx-4 gap-3 px-4 lg:mx-0 lg:px-0">
        {listening.data.map((item) => (
          <Card key={`${item.podcastId}:${item.episodeId}`} item={item} />
        ))}
      </Carousel>
    </section>
  );
};
