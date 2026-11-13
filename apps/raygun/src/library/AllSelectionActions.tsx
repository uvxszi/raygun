import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Check,
  CheckCheck,
  CircleDashed,
  Download,
  Heart,
  ListEnd,
  ListPlus,
  ListStart,
  Merge,
  Pin,
  PinOff,
  Trash2,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useCollectionSheets } from '../collections/collectionSheets';
import { DownloadSheet } from '../manga/LibrarySelection';
import { useMenuSheetsStore } from '../music/menuSheetsStore';
import { Sheet } from '../shell/Sheet';
import { showToast } from '../shell/toastStore';
import { useDragScroll } from '../shell/useDragScroll';
import { useRail } from '../shell/useRail';
import {
  downloadAll,
  favoriteAll,
  markAll,
  playlistSongs,
  queueAll,
  refreshAll,
  removeAll,
} from './allActions';
import { allItemPin, sharedActions } from './allItems';
import type { AllItem } from './allItems';

const BUTTON =
  'hover:bg-background grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-50';

/** Asks before the picked items leave the library, saying what goes with them. */
const RemoveSheet: FC<{
  items: AllItem[];
  onRemove: (deleteDownloads: boolean) => void;
  onClose: () => void;
}> = ({ items, onRemove, onClose }) => {
  const { t } = useTranslation();
  const [downloads, setDownloads] = useState(false);
  const playlists = items.filter((item) => item.kind === 'playlist').length;
  const sites = items.filter((item) => item.kind === 'website').length;
  const downloadable = items.some((item) => item.kind === 'title' && !item.entry.local);
  return (
    <Sheet title={t('menu.removeFromLibrary')} onClose={onClose}>
      <div className="flex flex-col gap-3 text-sm">
        <p>{t('library.all.removeQuestion', { count: items.length })}</p>
        {playlists > 0 && (
          <p className="text-muted">{t('library.all.removePlaylists', { count: playlists })}</p>
        )}
        {sites > 0 && (
          <p className="text-muted">{t('library.all.removeSites', { count: sites })}</p>
        )}
        {downloadable && (
          <button
            type="button"
            role="checkbox"
            aria-checked={downloads}
            onClick={() => setDownloads(!downloads)}
            className="hover:bg-background flex items-center gap-3 rounded-xl px-3 py-3 text-left"
          >
            <span
              aria-hidden
              className={`grid size-5 shrink-0 place-items-center rounded border-2 ${
                downloads ? 'border-primary bg-primary text-background' : 'border-muted'
              }`}
            >
              {downloads && <Check size={14} strokeWidth={3} />}
            </span>
            {t('library.all.removeDownloads')}
          </button>
        )}
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="hover:bg-background rounded-full px-4 py-2.5 font-medium"
          >
            {t('collections.cancel')}
          </button>
          <button
            type="button"
            onClick={() => onRemove(downloads)}
            className="bg-primary text-background rounded-full px-5 py-2.5 font-semibold"
          >
            {t('library.select.removeConfirm')}
          </button>
        </div>
      </div>
    </Sheet>
  );
};

type Panel = 'download' | 'remove' | null;

/**
 * The bar of Library → All's selection: what every picked item can have done (a mix gets what
 * its kinds share). Playlists, shows and books play next or last; titles, shows and books are
 * marked read, watched, played or finished; what can be downloaded is (titles by how many
 * chapters or episodes); titles are favorited; playlists are added to another or merged; and
 * everything is pinned to quick access or taken out of the library, after a question.
 */
export const AllSelectionActions: FC<{
  items: AllItem[];
  /** The action is done: picking ends. */
  onDone: () => void;
}> = ({ items, onDone }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const rail = useRail();
  const bar = useDragScroll<HTMLDivElement>();
  const openMerge = useCollectionSheets((state) => state.openMerge);
  const [panel, setPanel] = useState<Panel>(null);
  const actions = sharedActions(items);
  const pins = items.map(allItemPin);
  const allPinned = pins.length > 0 && pins.every((pin) => rail.isPinned(pin));
  const allFavorite = items.every((item) => item.kind === 'title' && item.entry.favorite);
  const titles = items.filter((item) => item.kind === 'title');
  const playlists = items.flatMap((item) => (item.kind === 'playlist' ? [item.playlist] : []));

  const run = useMutation({
    mutationFn: (work: () => Promise<string | null>) => work(),
    onSettled: () => refreshAll(client, items),
    onSuccess: (message) => {
      if (message) showToast(message);
      onDone();
    },
    onError: (error) => showToast(error.message),
  });

  const queue = (next: boolean) =>
    run.mutate(async () => {
      const count = await queueAll(items, next);
      return count > 0
        ? t('podcasts.select.queued', { count })
        : t('podcasts.select.nothingToQueue');
    });
  const mark = (done: boolean) =>
    run.mutate(async () => {
      await markAll(items, done);
      return null;
    });
  const download = (amount: number | null) =>
    run.mutate(async () => {
      const count = await downloadAll(items, amount);
      return count > 0 ? t('search.select.queued', { count }) : t('search.select.nothingQueued');
    });
  const favorite = () =>
    run.mutate(async () => {
      await favoriteAll(items, !allFavorite);
      return null;
    });
  const remove = (deleteDownloads: boolean) => {
    setPanel(null);
    run.mutate(async () => {
      const { removed, failed } = await removeAll(items, deleteDownloads);
      return [
        removed > 0 ? t('library.all.removed', { count: removed }) : null,
        failed > 0 ? t('library.all.removeFailed', { count: failed }) : null,
      ]
        .filter(Boolean)
        .join(' · ');
    });
  };
  const pin = () => {
    rail.setPinned(pins, !allPinned);
    showToast(
      t(allPinned ? 'podcasts.select.unpinned' : 'podcasts.select.pinned', {
        count: pins.length,
      }),
    );
    onDone();
  };
  const addToPlaylist = () => {
    useMenuSheetsStore.getState().addToPlaylist({
      title: playlists.map((playlist) => playlist.name).join(', '),
      load: () => playlistSongs(items),
    });
    onDone();
  };
  const merge = () => {
    openMerge(playlists.map((playlist) => playlist.id));
    onDone();
  };

  const button = (label: string, Icon: LucideIcon, onClick: () => void, pressed?: boolean) => (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-pressed={pressed}
      disabled={run.isPending}
      onClick={onClick}
      className={`${BUTTON} ${pressed ? 'text-primary' : ''}`}
    >
      <Icon aria-hidden size={20} />
    </button>
  );

  return (
    <>
      <div
        ref={bar}
        role="toolbar"
        aria-label={t('library.all.actions')}
        data-testid="all-selection-actions"
        className="bg-surface motion-rise sticky bottom-2 z-30 mt-auto flex w-full justify-around no-scrollbar overflow-x-auto rounded-2xl p-1 shadow-xl"
      >
        {actions.queue && (
          <>
            {button(t('menu.playNext'), ListStart, () => queue(true))}
            {button(t('menu.addToQueue'), ListEnd, () => queue(false))}
          </>
        )}
        {actions.done && (
          <>
            {button(t(actions.done.done), CheckCheck, () => mark(true))}
            {button(t(actions.done.notDone), CircleDashed, () => mark(false))}
          </>
        )}
        {actions.download &&
          button(t('library.select.download'), Download, () =>
            titles.length > 0 ? setPanel('download') : download(null),
          )}
        {actions.favorite &&
          button(
            t(allFavorite ? 'library.select.unfavorite' : 'library.select.favorite'),
            Heart,
            favorite,
            allFavorite,
          )}
        {actions.playlists && (
          <>
            {button(t('collections.addToPlaylist'), ListPlus, addToPlaylist)}
            {playlists.length > 1 && button(t('library.all.merge'), Merge, merge)}
          </>
        )}
        {button(t(allPinned ? 'rail.unpin' : 'rail.pin'), allPinned ? PinOff : Pin, pin)}
        {button(t('menu.removeFromLibrary'), Trash2, () => setPanel('remove'))}
      </div>
      {panel === 'download' && (
        <DownloadSheet
          video={titles.every((item) => item.kind === 'title' && item.medium === 'video')}
          onPick={(amount) => {
            setPanel(null);
            download(amount);
          }}
          onClose={() => setPanel(null)}
        />
      )}
      {panel === 'remove' && (
        <RemoveSheet items={items} onRemove={remove} onClose={() => setPanel(null)} />
      )}
    </>
  );
};
