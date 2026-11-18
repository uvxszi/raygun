import { commands } from '@raygun/bindings';
import type { ContinueWatching } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { BookmarkMinus, Check, CheckCheck, Download, Eye, Pin, Trash2, X } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { WATCH_PROGRESS_KEYS } from '../shell/useLiveInvalidation';
import { SourceIssue } from '../shell/SourceIssue';
import { showToast } from '../shell/toastStore';
import { useDragScroll } from '../shell/useDragScroll';
import { useRail } from '../shell/useRail';
import { LOCAL_VIDEO, entryPage, linkDownloads, storedTitle, stremioParts } from './continueEntry';
import type { ContinueSelection } from './useContinueSelection';
import { STREMIO_SOURCE } from './useVideoLibrary';
import { useRemoveContinueMany } from './useWatch';

/**
 * What else picked entries can have done (M2.0.1): marked watched, downloaded, pinned to quick
 * access, their titles taken out of the library. Each returns what it did, in a line.
 */
const useContinueBulk = (items: ContinueWatching[]) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const rail = useRail();
  const refresh = () =>
    Promise.all(
      [...WATCH_PROGRESS_KEYS, ['video', 'downloads']].map((queryKey) =>
        client.invalidateQueries({ queryKey }),
      ),
    );
  const count = items.length;
  return {
    markWatched: async () => {
      for (const item of items.filter((each) => !each.watched)) {
        await unwrapResult(commands.videoMarkWatched(item.key));
      }
      return t('home.continue.markedWatched', { count });
    },
    download: async () => {
      let queued = 0;
      const byGroup = new Map<string, string[]>();
      for (const item of items) {
        if (item.link && linkDownloads(item)) {
          await unwrapResult(commands.videoLinkDownload(item.key, null));
          queued += 1;
        } else if (item.group && item.episodeId && item.sourceId !== LOCAL_VIDEO) {
          byGroup.set(item.group, [...(byGroup.get(item.group) ?? []), item.episodeId]);
        }
      }
      for (const [group, episodes] of byGroup) {
        queued += await unwrapResult(commands.videoDownloadEpisodes(group, episodes));
      }
      return queued > 0
        ? t('search.select.queued', { count: queued })
        : t('search.select.nothingQueued');
    },
    pin: () => {
      rail.setPinned(
        items.flatMap((item) => {
          const page = entryPage(item);
          return page
            ? [
                {
                  kind: 'video',
                  to: page.to,
                  search: page.search as Record<string, string>,
                  name: item.title,
                  artworkUrl: item.poster,
                },
              ]
            : [];
        }),
        true,
      );
      return Promise.resolve(t('home.continue.pinned', { count }));
    },
    removeFromLibrary: async () => {
      const seen = new Set<string>();
      for (const item of items) {
        if (item.sourceId === STREMIO_SOURCE && item.remoteId) {
          if (seen.has(item.remoteId)) continue;
          seen.add(item.remoteId);
          const { kind, id } = stremioParts(item.remoteId);
          await unwrapResult(commands.stremioSetInLibrary(kind, id, false));
        } else {
          const titleId = storedTitle(item);
          if (!titleId || seen.has(titleId)) continue;
          seen.add(titleId);
          await unwrapResult(commands.videoSetInLibrary(titleId, false));
        }
      }
      await client.invalidateQueries({ queryKey: ['video', 'library'] });
      return t('home.continue.removedFromLibrary', { count: seen.size });
    },
    refresh,
  };
};

/**
 * An entry's pick circle, in its top left corner: it shows on hover or focus (to start picking
 * with a mouse or keys) and always while picking. Shift-click picks a range.
 */
export const PickCircle: FC<{
  checked: boolean;
  /** Picking is on: the circle always shows. */
  shown: boolean;
  name: string;
  onPick: (range: boolean) => void;
  className?: string;
}> = ({ checked, shown, name, onPick, className = 'absolute top-1.5 left-1.5' }) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      aria-label={t('watch.continueSelect.pick', { title: name })}
      onClick={(event) => {
        event.preventDefault();
        event.stopPropagation();
        onPick(event.shiftKey);
      }}
      className={`z-10 grid size-6 shrink-0 place-items-center rounded-full border-2 shadow transition-opacity ${
        checked
          ? 'border-primary bg-primary text-background'
          : 'border-white/90 bg-black/40 text-transparent'
      } ${
        shown || checked
          ? 'opacity-100'
          : 'pointer-events-none opacity-0 group-hover/pick:pointer-events-auto group-hover/pick:opacity-100 focus-visible:pointer-events-auto focus-visible:opacity-100'
      } ${className}`}
    >
      <Check aria-hidden size={14} strokeWidth={3} />
    </button>
  );
};

/**
 * What can be done to the picked entries: pick all (or none), or remove them, asked first with
 * how many. `history` words it for the watch history, which removing empties too.
 */
export const ContinueSelectionBar: FC<{
  selection: ContinueSelection;
  /** The entries listed: the picked ones get the bar's other actions. */
  items?: ContinueWatching[];
  history?: boolean;
  className?: string;
}> = ({ selection, items = [], history = false, className = '' }) => {
  const { t } = useTranslation();
  const remove = useRemoveContinueMany();
  const [asking, setAsking] = useState<'remove' | 'library' | false>(false);
  const count = selection.selected.length;
  const picked = items.filter((item) => selection.isSelected(item.key));
  const bulk = useContinueBulk(picked);
  const scroller = useDragScroll<HTMLDivElement>();
  const other = useMutation({
    mutationFn: (work: () => Promise<string>) => work(),
    onSettled: () => bulk.refresh(),
    onSuccess: (message) => {
      showToast(message);
      setAsking(false);
      selection.clear();
    },
    onError: (error) => showToast(error.message),
  });
  const question = t(
    history ? 'watch.continueSelect.removeHistoryAsk' : 'watch.continueSelect.removeAsk',
    { count },
  );
  const confirm = () =>
    remove.mutate(selection.selected, {
      onSuccess: (removed) => {
        showToast(
          t(history ? 'watch.continueSelect.removedHistory' : 'watch.continueSelect.removed', {
            count: removed,
          }),
        );
        setAsking(false);
        selection.clear();
      },
    });

  const action = (label: string, Icon: LucideIcon, onClick: () => void, wide = false) => (
    <button
      type="button"
      aria-label={wide ? undefined : label}
      title={label}
      onClick={onClick}
      className={`hover:bg-background flex shrink-0 items-center gap-2 rounded-full text-sm font-medium ${
        wide ? 'px-3 py-2' : 'size-10 justify-center'
      }`}
    >
      <Icon aria-hidden size={wide ? 16 : 20} />
      {wide && label}
    </button>
  );

  return (
    <div
      data-testid="continue-selection-bar"
      className={`bg-surface flex flex-col gap-1 rounded-2xl p-1 shadow-xl ${className}`}
    >
      {asking === 'library' ? (
        <div
          role="alertdialog"
          aria-label={t('home.continue.removeFromLibraryAsk', { count })}
          className="flex flex-wrap items-center justify-end gap-2 px-2 py-1"
        >
          <p className="min-w-0 flex-1 text-sm">
            {t('home.continue.removeFromLibraryAsk', { count })}
          </p>
          <button
            type="button"
            onClick={() => setAsking(false)}
            className="hover:bg-background rounded-full px-3 py-1.5 text-sm"
          >
            {t('collections.cancel')}
          </button>
          <button
            type="button"
            autoFocus
            disabled={other.isPending}
            onClick={() => other.mutate(bulk.removeFromLibrary)}
            className="bg-primary text-background rounded-full px-4 py-1.5 text-sm font-semibold disabled:opacity-60"
          >
            {t('menu.removeFromLibrary')}
          </button>
        </div>
      ) : asking ? (
        <div
          role="alertdialog"
          aria-label={question}
          className="flex flex-wrap items-center justify-end gap-2 px-2 py-1"
        >
          <p className="min-w-0 flex-1 text-sm">{question}</p>
          <button
            type="button"
            onClick={() => setAsking(false)}
            className="hover:bg-background rounded-full px-3 py-1.5 text-sm"
          >
            {t('collections.cancel')}
          </button>
          <button
            type="button"
            autoFocus
            disabled={remove.isPending}
            onClick={confirm}
            className="bg-primary text-background rounded-full px-4 py-1.5 text-sm font-semibold disabled:opacity-60"
          >
            {t('watch.continueSelect.remove')}
          </button>
        </div>
      ) : (
        <div
          role="toolbar"
          aria-label={t('watch.continueSelect.actions')}
          className="flex items-center gap-1"
        >
          {action(t('watch.endSelection'), X, selection.clear)}
          <span aria-live="polite" className="min-w-0 flex-1 truncate px-1 text-sm font-semibold">
            {t('watch.selectedCount', { count })}
          </span>
          <div ref={scroller} className="flex min-w-0 no-scrollbar overflow-x-auto">
            {action(
              t(selection.allSelected ? 'watch.selectNone' : 'watch.selectAllShown'),
              CheckCheck,
              selection.toggleAll,
              true,
            )}
            {action(t('watch.continueSelect.remove'), Trash2, () => setAsking('remove'), true)}
            {picked.length > 0 && (
              <>
                {action(t('menu.markWatched'), Eye, () => other.mutate(bulk.markWatched), true)}
                {action(
                  t('library.select.download'),
                  Download,
                  () => other.mutate(bulk.download),
                  true,
                )}
                {action(t('rail.pin'), Pin, () => other.mutate(bulk.pin), true)}
                {action(
                  t('menu.removeFromLibrary'),
                  BookmarkMinus,
                  () => setAsking('library'),
                  true,
                )}
              </>
            )}
          </div>
        </div>
      )}
      {remove.error && <SourceIssue compact>{remove.error.message}</SourceIssue>}
    </div>
  );
};
