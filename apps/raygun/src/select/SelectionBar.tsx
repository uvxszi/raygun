import { useMutation, useQueryClient } from '@tanstack/react-query';
import { BookmarkPlus, Download, FolderPlus, Heart, ListPlus, X } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { fromOnlineTrack } from '../collections/useCollections';
import { groupWords } from '../manga/groupWords';
import { DownloadSheet } from '../manga/LibrarySelection';
import { useMenuSheetsStore } from '../music/menuSheetsStore';
import { showToast } from '../shell/toastStore';
import { useDragScroll } from '../shell/useDragScroll';
import {
  addTitles,
  downloadAll,
  favoriteAll,
  refreshLibraries,
  resultMessage,
} from './bulkActions';
import { GroupPickSheet } from './GroupPickSheet';
import { pickedKinds, useSelectionStore } from './selectionStore';

type Panel = 'group' | 'download' | null;

/**
 * What can be done to the picked results, for the kinds they are: songs can be favorited, put in
 * a playlist and downloaded; comics, books and videos favorited, added to the library or a group
 * or folder (one medium at a time) and downloaded. A mix gets what its kinds share.
 */
export const SelectionBar: FC = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const items = useSelectionStore((state) => state.items);
  const clear = useSelectionStore((state) => state.clear);
  const { songs, titles, medium } = useMemo(() => pickedKinds(items), [items]);
  const [panel, setPanel] = useState<Panel>(null);
  const media = useMemo(() => [...new Set(titles.map((title) => title.medium))], [titles]);
  const onlySongs = titles.length === 0;
  const onlyTitles = songs.length === 0;
  const actions = useDragScroll<HTMLDivElement>();

  const run = useMutation({
    mutationFn: async (action: () => Promise<string>) => action(),
    onSettled: () => (media.length > 0 ? refreshLibraries(client, media) : undefined),
    onSuccess: (message) => {
      if (message) showToast(message);
      clear();
    },
    onError: (error) => showToast(error.message),
  });

  const favorite = () =>
    run.mutate(async () => {
      const result = await favoriteAll(client, songs, titles);
      const done = result.titleIds.length + songs.length;
      return resultMessage(
        t,
        result,
        done > 0 ? t('search.select.favorited', { count: done }) : null,
      );
    });
  const addToLibrary = () =>
    run.mutate(async () => {
      const result = await addTitles(titles);
      const done = result.titleIds.length;
      return resultMessage(
        t,
        result,
        done > 0 ? t('search.select.addedToLibrary', { count: done }) : null,
      );
    });
  const download = (amount: number | null) =>
    run.mutate(async () => {
      const result = await downloadAll(songs, titles, amount);
      return resultMessage(
        t,
        result,
        result.queued > 0
          ? t('search.select.queued', { count: result.queued })
          : result.failed.length === 0
            ? t('search.select.nothingQueued')
            : null,
      );
    });
  const addToPlaylist = () => {
    const tracks = songs.map((song) => fromOnlineTrack(song.track));
    useMenuSheetsStore.getState().addToPlaylist({
      title: t('search.select.playlistTitle', { count: tracks.length }),
      load: () => Promise.resolve(tracks),
    });
    clear();
  };

  const action = (label: string, Icon: LucideIcon, onClick: () => void) => (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={run.isPending}
      onClick={onClick}
      className="hover:bg-background grid size-10 shrink-0 place-items-center rounded-full disabled:opacity-40"
    >
      <Icon aria-hidden size={20} />
    </button>
  );

  return (
    <>
      <div
        role="toolbar"
        aria-label={t('search.select.actions')}
        data-testid="search-selection-bar"
        className="bg-surface motion-rise sticky bottom-2 z-30 mt-2 flex w-full items-center gap-1 rounded-2xl p-1 shadow-xl"
      >
        {action(t('search.select.clear'), X, clear)}
        <span aria-live="polite" className="min-w-0 flex-1 truncate px-1 text-sm font-semibold">
          {run.isPending
            ? t('search.select.working', { count: items.length })
            : t('search.select.count', { count: items.length })}
        </span>
        <div ref={actions} className="flex no-scrollbar overflow-x-auto">
          {action(t('search.select.favorite'), Heart, favorite)}
          {onlySongs && action(t('search.select.addToPlaylist'), ListPlus, addToPlaylist)}
          {onlyTitles && action(t('search.select.addToLibrary'), BookmarkPlus, addToLibrary)}
          {onlyTitles &&
            medium &&
            action(t('library.categories.addTo', groupWords(medium)), FolderPlus, () =>
              setPanel('group'),
            )}
          {action(t('search.select.download'), Download, () =>
            onlySongs ? download(null) : setPanel('download'),
          )}
        </div>
      </div>
      {panel === 'group' && medium && (
        <GroupPickSheet
          medium={medium}
          titles={titles}
          onDone={() => {
            setPanel(null);
            clear();
          }}
          onClose={() => setPanel(null)}
        />
      )}
      {panel === 'download' && (
        <DownloadSheet
          video={medium === 'video'}
          onPick={(amount) => {
            setPanel(null);
            download(amount);
          }}
          onClose={() => setPanel(null)}
        />
      )}
    </>
  );
};
