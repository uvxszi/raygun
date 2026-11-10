import { commands } from '@raygun/bindings';
import type { PodcastPicks } from '@raygun/bindings';
import {
  BookMinus,
  CheckCheck,
  CircleDashed,
  Download,
  ListEnd,
  ListPlus,
  Pin,
  PinOff,
  Trash2,
  UserMinus,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { ConfirmButton } from '../shell/ConfirmButton';
import { showToast } from '../shell/toastStore';
import { useDragScroll } from '../shell/useDragScroll';
import { useRail } from '../shell/useRail';
import { usePodcastPicksAction } from './usePodcasts';

const BUTTON =
  'hover:bg-background grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-50';

/**
 * The bar of a selection of shows and episodes, or of books and chapters (`books`): play them
 * next or last, mark them played (finished), download them or delete their downloads, and for
 * picked shows and books unfollow (take out of the library) and pin to quick access. Rust's
 * podcast service works out what each does to the shows and books picked whole.
 */
export const PodcastPickActions: FC<{
  picks: PodcastPicks;
  books?: boolean;
  /** The action is done: picking ends. */
  onDone: () => void;
}> = ({ picks, books = false, onDone }) => {
  const { t } = useTranslation();
  const rail = useRail();
  const action = usePodcastPicksAction();
  const bar = useDragScroll<HTMLDivElement>();
  const shows = picks.shows;
  const page = books ? '/audiobook' : '/podcast';
  const allPinned =
    shows.length > 0 && shows.every((id) => rail.isPinned({ to: page, search: { id } }));

  const run = (work: () => Promise<string | null>) =>
    action.mutate(work, {
      onSuccess: (message) => {
        if (message) showToast(message);
        onDone();
      },
      onError: (error) => showToast(error.message),
    });

  const queue = (next: boolean) =>
    run(async () => {
      const count = await unwrapResult(commands.podcastQueuePicks(picks, next));
      return count > 0
        ? t('podcasts.select.queued', { count })
        : t('podcasts.select.nothingToQueue');
    });
  const mark = (played: boolean) =>
    run(async () => {
      await unwrapResult(commands.podcastSetPlayedPicks(picks, played));
      return null;
    });
  const download = () =>
    run(async () => {
      const count = await unwrapResult(commands.podcastDownloadPicks(picks));
      return count > 0 ? t('search.select.queued', { count }) : t('search.select.nothingQueued');
    });
  const removeDownloads = () =>
    run(async () => {
      const count = await unwrapResult(commands.podcastRemoveDownloadsPicks(picks));
      return t('podcasts.select.removedDownloads', { count });
    });
  const unfollow = () =>
    run(async () => {
      const count = await unwrapResult(commands.podcastSetSubscribedMany(shows, false));
      return t(books ? 'audiobooks.select.removed' : 'podcasts.select.unfollowed', { count });
    });
  const pin = () =>
    run(async () => {
      const count = await unwrapResult(commands.podcastPinMany(shows, !allPinned));
      return t(allPinned ? 'podcasts.select.unpinned' : 'podcasts.select.pinned', { count });
    });

  const button = (label: string, Icon: LucideIcon, onClick: () => void) => (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={action.isPending}
      onClick={onClick}
      className={BUTTON}
    >
      <Icon aria-hidden size={20} />
    </button>
  );

  return (
    <div
      ref={bar}
      role="toolbar"
      aria-label={t(books ? 'audiobooks.select.actions' : 'podcasts.select.actions')}
      data-testid="podcast-pick-actions"
      className="bg-surface sticky bottom-2 z-30 mt-auto flex w-full justify-around no-scrollbar overflow-x-auto rounded-2xl p-1 shadow-xl"
    >
      {button(t('menu.playNext'), ListPlus, () => queue(true))}
      {button(t('menu.addToQueue'), ListEnd, () => queue(false))}
      {button(t(books ? 'audiobooks.markFinished' : 'podcasts.markPlayed'), CheckCheck, () =>
        mark(true),
      )}
      {button(t(books ? 'audiobooks.markUnfinished' : 'podcasts.markUnplayed'), CircleDashed, () =>
        mark(false),
      )}
      {button(t('library.select.download'), Download, download)}
      <ConfirmButton
        label={t('podcasts.removeDownloads')}
        confirm={t('podcasts.removeDownloadConfirm')}
        icon={Trash2}
        iconSize={20}
        disabled={action.isPending}
        onConfirm={removeDownloads}
        idleClassName="hover:bg-background size-9"
      />
      {shows.length > 0 && (
        <>
          {button(
            t(books ? 'audiobooks.removeFromLibrary' : 'podcasts.menu.unfollow'),
            books ? BookMinus : UserMinus,
            unfollow,
          )}
          {button(t(allPinned ? 'rail.unpin' : 'rail.pin'), allPinned ? PinOff : Pin, pin)}
        </>
      )}
    </div>
  );
};
