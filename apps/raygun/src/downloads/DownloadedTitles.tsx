import { commands } from '@raygun/bindings';
import type { MangaLibraryEntry } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import { Trash2 } from 'lucide-react';
import { useMemo, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { LibraryItems } from '../manga/LibraryItems';
import { withLibraryDisplay } from '../manga/libraryDisplay';
import { SelectionActions, SelectionTopBar } from '../manga/LibrarySelection';
import { ROUTES } from '../manga/medium';
import { entryPage, useOpenTitlePage } from '../manga/titleMenu';
import type { Medium } from '../manga/medium';
import { useCategories } from '../manga/useCategories';
import { fetchContinue, useRemoveTitles } from '../manga/useLibraryActions';
import { useSettings } from '../settings/useSettings';
import type { MenuItem } from '../shell/contextMenuStore';
import { SourceIssue } from '../shell/SourceIssue';
import { useDownloadedTitles } from './useDownloadedTitles';

/**
 * Downloads → Comics, Books or Videos: every title with something downloaded, as the Library
 * lists it. A title opens its page; its ⋮ has the Library's options (read, mark read, export,
 * copy…) and deletes its downloads; a long press selects, for the Library's selection actions.
 * `limit` shows only the first few (Downloads → All).
 */
export const DownloadedTitles: FC<{ medium: Medium; limit?: number }> = ({ medium, limit }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const titles = useDownloadedTitles(medium);
  const categories = useCategories(medium);
  const display = withLibraryDisplay(useSettings().data?.libraryDisplay);
  const remove = useRemoveTitles(medium);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const lastPicked = useRef<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const video = medium === 'video';

  const entries = useMemo(() => titles.data ?? [], [titles.data]);
  const shown = limit === undefined ? entries : entries.slice(0, limit);
  const selectedEntries = entries.filter((entry) => selected.has(entry.titleId));
  const selecting = selected.size > 0;

  const toggle = (entry: MangaLibraryEntry) =>
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(entry.titleId)) next.delete(entry.titleId);
      else next.add(entry.titleId);
      lastPicked.current = entry.titleId;
      return next;
    });
  // A long press adds the title, and everything between it and the last pick (as in the Library).
  const longPress = (entry: MangaLibraryEntry) =>
    setSelected((current) => {
      const next = new Set(current);
      const from = lastPicked.current;
      const start =
        from && current.has(from) ? shown.findIndex((item) => item.titleId === from) : -1;
      const end = shown.findIndex((item) => item.titleId === entry.titleId);
      if (start >= 0 && end >= 0) {
        const [low, high] = start < end ? [start, end] : [end, start];
        for (const item of shown.slice(low, high + 1)) next.add(item.titleId);
      } else {
        next.add(entry.titleId);
      }
      lastPicked.current = entry.titleId;
      return next;
    });
  const clearSelection = () => {
    setSelected(new Set());
    lastPicked.current = null;
  };

  const openTitlePage = useOpenTitlePage();
  const onContinue = async (entry: MangaLibraryEntry) => {
    setNotice(null);
    try {
      if (video) {
        // The title's page goes under the player: Back from it lands there.
        await openTitlePage(entryPage(medium, entry));
        await unwrapResult(commands.videoSourceWatch(entry.titleId, null));
        return;
      }
      const chapter = await fetchContinue(medium, entry.titleId);
      if (!chapter) {
        setNotice(t('library.noNextChapter'));
        return;
      }
      await openTitlePage(entryPage(medium, entry));
      void navigate({ to: ROUTES[medium].read, search: { title: entry.titleId, chapter } });
    } catch (error) {
      setNotice(error instanceof Error ? error.message : String(error));
    }
  };

  const deleteItem = (entry: MangaLibraryEntry): MenuItem[] =>
    entry.local
      ? []
      : [
          {
            label: t(video ? 'downloads.deleteEpisodes' : 'downloads.deleteChapters'),
            icon: Trash2,
            danger: true,
            confirm: t(video ? 'downloads.deleteEpisodesAsk' : 'downloads.deleteChaptersAsk', {
              name: entry.title,
            }),
            onSelect: () =>
              remove.mutate({ ids: [entry.titleId], fromLibrary: false, deleteDownloads: true }),
          },
        ];

  if (titles.error) {
    return (
      <SourceIssue compact onRetry={() => void titles.refetch()}>
        {titles.error.message}
      </SourceIssue>
    );
  }
  if (entries.length === 0) return null;
  return (
    <div data-testid={`downloaded-${medium}`} className="flex flex-col gap-2">
      {selecting && (
        <SelectionTopBar
          count={selected.size}
          onSelectAll={() => setSelected(new Set(entries.map((entry) => entry.titleId)))}
          onInvert={() =>
            setSelected(
              (current) =>
                new Set(
                  entries.map((entry) => entry.titleId).filter((titleId) => !current.has(titleId)),
                ),
            )
          }
          onClose={clearSelection}
        />
      )}
      {notice && (
        <p role="status" className="text-muted text-sm">
          {notice}
        </p>
      )}
      {remove.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {remove.error.message}
        </p>
      )}
      <LibraryItems
        entries={shown}
        medium={medium}
        mode="list"
        display={display}
        selected={selected}
        onToggle={toggle}
        onLongPress={longPress}
        onContinue={(entry) => void onContinue(entry)}
        moreItems={deleteItem}
        // Titles kept out of the library have a time it was added of 0.
        inLibrary={(entry) => (entry.addedAt ?? 0) > 0}
      />
      {selecting && (
        <SelectionActions
          medium={medium}
          categories={categories.data ?? []}
          selected={selectedEntries}
          onDone={clearSelection}
        />
      )}
    </div>
  );
};
