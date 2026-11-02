import type { LibraryCategory, LibraryList, MangaLibraryEntry } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import {
  BookCheck,
  BookX,
  Bookmark,
  Check,
  CheckCheck,
  Download,
  FolderCog,
  Heart,
  Minus,
  Pin,
  PinOff,
  Trash2,
  X,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { LISTS, listKey } from './libraryDisplay';
import type { Medium } from './medium';
import {
  useChangeCategories,
  useDownloadNext,
  useMarkRead,
  useRemoveTitles,
  useSetFavorite,
  useSetList,
  useSetPinned,
} from './useLibraryActions';

/** Mihon's selection toolbar: how many are picked, select all or invert (this tab), close. */
export const SelectionTopBar: FC<{
  count: number;
  onSelectAll: () => void;
  onInvert: () => void;
  onClose: () => void;
}> = ({ count, onSelectAll, onInvert, onClose }) => {
  const { t } = useTranslation();
  const action = (label: string, Icon: LucideIcon, onClick: () => void) => (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className="hover:bg-surface grid size-11 shrink-0 place-items-center rounded-full"
    >
      <Icon aria-hidden size={22} />
    </button>
  );
  return (
    <header
      data-testid="selection-bar"
      className="bg-background sticky top-0 z-30 -mx-4 mb-1 flex min-h-14 items-center gap-1 px-2 lg:-mx-10 lg:px-6"
    >
      {action(t('library.select.close'), X, onClose)}
      <h1 className="min-w-0 flex-1 truncate px-1 text-lg font-semibold" aria-live="polite">
        {t('library.select.count', { count })}
      </h1>
      {action(t('library.select.all'), CheckCheck, onSelectAll)}
      {action(t('library.select.invert'), Minus, onInvert)}
    </header>
  );
};

type CheckState = 'on' | 'off' | 'mixed';

/** Categories for the selection: tri-state boxes where only some of the titles are in one. */
const CategoriesSheet: FC<{
  medium: Medium;
  categories: LibraryCategory[];
  selected: MangaLibraryEntry[];
  onDone: () => void;
  onClose: () => void;
}> = ({ medium, categories, selected, onDone, onClose }) => {
  const { t } = useTranslation();
  const change = useChangeCategories(medium);
  // What the titles were in when the sheet opened; mixed boxes keep a third state.
  const [initial] = useState<Record<string, CheckState>>(() =>
    Object.fromEntries(
      categories.map((category) => {
        const holding = selected.filter((entry) => entry.categoryIds.includes(category.id)).length;
        const state: CheckState =
          holding === 0 ? 'off' : holding === selected.length ? 'on' : 'mixed';
        return [category.id, state];
      }),
    ),
  );
  const [states, setStates] = useState(initial);
  const mixedAtStart = new Set(Object.keys(initial).filter((id) => initial[id] === 'mixed'));
  const cycle = (id: string) =>
    setStates((current) => {
      const state = current[id];
      // Mixed boxes go mixed → off → on → mixed (Mihon); the others just toggle.
      const next: CheckState = mixedAtStart.has(id)
        ? state === 'mixed'
          ? 'off'
          : state === 'off'
            ? 'on'
            : 'mixed'
        : state === 'on'
          ? 'off'
          : 'on';
      return { ...current, [id]: next };
    });
  const save = () => {
    const ids = selected.map((entry) => entry.titleId);
    const add = Object.keys(states).filter((id) => states[id] === 'on');
    const remove = Object.keys(states).filter((id) => states[id] === 'off');
    change.mutate({ ids, add, remove }, { onSuccess: onDone });
  };
  return (
    <Sheet title={t('library.select.categories')} onClose={onClose}>
      <div className="flex flex-col gap-1 text-sm">
        {categories.length === 0 && (
          <p className="text-muted">{t('library.select.noCategories')}</p>
        )}
        {categories.map((category) => {
          const state = states[category.id];
          return (
            <button
              key={category.id}
              type="button"
              role="checkbox"
              aria-checked={state === 'mixed' ? 'mixed' : state === 'on'}
              onClick={() => cycle(category.id)}
              className="hover:bg-background flex items-center gap-3 rounded-xl px-3 py-3 text-left"
            >
              <span
                aria-hidden
                className={`grid size-5 shrink-0 place-items-center rounded border-2 ${state === 'off' ? 'border-muted' : 'border-primary bg-primary text-background'}`}
              >
                {state === 'on' && <Check size={14} strokeWidth={3} />}
                {state === 'mixed' && <Minus size={14} strokeWidth={3} />}
              </span>
              {category.name}
            </button>
          );
        })}
        {change.error && (
          <p role="alert" className="text-primary wrap-anywhere">
            {change.error.message}
          </p>
        )}
        <div className="mt-2 flex items-center justify-between gap-2">
          <Link
            to="/library/categories"
            search={{ medium }}
            className="text-primary rounded-full px-3 py-2 font-medium"
          >
            {t('library.categories.edit')}
          </Link>
          {categories.length > 0 && (
            <button
              type="button"
              onClick={save}
              disabled={change.isPending}
              className="bg-primary text-background rounded-full px-5 py-2.5 font-semibold disabled:opacity-50"
            >
              {t('library.select.save')}
            </button>
          )}
        </div>
      </div>
    </Sheet>
  );
};

/** One list for the selection, or none. */
export const ListSheet: FC<{
  medium: Medium;
  current: LibraryList | null | 'mixed';
  onPick: (list: LibraryList | null) => void;
  onClose: () => void;
}> = ({ medium, current, onPick, onClose }) => {
  const { t } = useTranslation();
  const video = medium === 'video';
  const option = (list: LibraryList | null) => {
    const label = list ? t(listKey(list, video)) : t('library.lists.none');
    const chosen = current === list;
    return (
      <button
        key={list ?? 'none'}
        type="button"
        role="radio"
        aria-checked={chosen}
        onClick={() => onPick(list)}
        className="hover:bg-background flex items-center gap-4 rounded-xl px-3 py-3 text-left"
      >
        <span
          aria-hidden
          className={`grid size-5 shrink-0 place-items-center rounded-full border-2 ${chosen ? 'border-primary' : 'border-muted'}`}
        >
          {chosen && <span className="bg-primary size-2.5 rounded-full" />}
        </span>
        {label}
      </button>
    );
  };
  return (
    <Sheet title={t('library.lists.title')} onClose={onClose}>
      <div
        role="radiogroup"
        aria-label={t('library.lists.title')}
        className="-mx-2 flex flex-col text-sm"
      >
        {LISTS.map(option)}
        {option(null)}
      </div>
    </Sheet>
  );
};

/** Download the next chapters of the selection (Mihon's menu). */
const DownloadSheet: FC<{ onPick: (amount: number | null) => void; onClose: () => void }> = ({
  onPick,
  onClose,
}) => {
  const { t } = useTranslation();
  return (
    <Sheet title={t('library.select.download')} onClose={onClose}>
      <div className="-mx-2 flex flex-col text-sm">
        {[1, 5, 10, 25].map((amount) => (
          <button
            key={amount}
            type="button"
            onClick={() => onPick(amount)}
            className="hover:bg-background rounded-xl px-3 py-3 text-left"
          >
            {t('library.select.downloadNext', { count: amount })}
          </button>
        ))}
        <button
          type="button"
          onClick={() => onPick(null)}
          className="hover:bg-background rounded-xl px-3 py-3 text-left"
        >
          {t('library.select.downloadUnread')}
        </button>
      </div>
    </Sheet>
  );
};

/** Mihon's Remove dialog: from the library, their downloads, or both. */
const RemoveSheet: FC<{
  medium: Medium;
  selected: MangaLibraryEntry[];
  onDone: () => void;
  onClose: () => void;
}> = ({ medium, selected, onDone, onClose }) => {
  const { t } = useTranslation();
  const remove = useRemoveTitles(medium);
  const [fromLibrary, setFromLibrary] = useState(false);
  const [downloads, setDownloads] = useState(false);
  const anyLocal = selected.some((entry) => entry.local);
  const box = (label: string, checked: boolean, onChange: (value: boolean) => void) => (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className="hover:bg-background flex items-center gap-3 rounded-xl px-3 py-3 text-left"
    >
      <span
        aria-hidden
        className={`grid size-5 shrink-0 place-items-center rounded border-2 ${checked ? 'border-primary bg-primary text-background' : 'border-muted'}`}
      >
        {checked && <Check size={14} strokeWidth={3} />}
      </span>
      {label}
    </button>
  );
  return (
    <Sheet title={t('library.select.remove')} onClose={onClose}>
      <div className="flex flex-col gap-1 text-sm">
        {box(t('library.select.fromLibrary'), fromLibrary, setFromLibrary)}
        {!anyLocal &&
          box(
            t(
              medium === 'video'
                ? 'library.select.downloadedEpisodes'
                : 'library.select.downloadedChapters',
            ),
            downloads,
            setDownloads,
          )}
        {remove.error && (
          <p role="alert" className="text-primary wrap-anywhere">
            {remove.error.message}
          </p>
        )}
        <button
          type="button"
          disabled={(!fromLibrary && !downloads) || remove.isPending}
          onClick={() =>
            remove.mutate(
              {
                ids: selected.map((entry) => entry.titleId),
                fromLibrary,
                deleteDownloads: downloads,
              },
              { onSuccess: onDone },
            )
          }
          className="bg-primary text-background mt-2 self-end rounded-full px-5 py-2.5 font-semibold disabled:opacity-50"
        >
          {t('library.select.removeConfirm')}
        </button>
      </div>
    </Sheet>
  );
};

type Panel = 'categories' | 'list' | 'download' | 'remove' | null;

/** The bottom bar of selection mode: what to do with the picked titles. */
export const SelectionActions: FC<{
  medium: Medium;
  categories: LibraryCategory[];
  selected: MangaLibraryEntry[];
  onDone: () => void;
}> = ({ medium, categories, selected, onDone }) => {
  const { t } = useTranslation();
  const [panel, setPanel] = useState<Panel>(null);
  const setList = useSetList(medium);
  const setFavorite = useSetFavorite(medium);
  const setPinned = useSetPinned(medium);
  const markRead = useMarkRead(medium);
  const download = useDownloadNext(medium);
  const ids = selected.map((entry) => entry.titleId);
  const allFavorite = selected.every((entry) => entry.favorite);
  const allPinned = selected.every((entry) => entry.pinned);
  const lists = new Set(selected.map((entry) => entry.list ?? null));
  const currentList = lists.size === 1 ? ([...lists][0] ?? null) : 'mixed';
  const canDownload = medium !== 'video' && !selected.some((entry) => entry.local);
  const video = medium === 'video';
  const finish = { onSuccess: onDone };
  const error =
    setList.error ?? setFavorite.error ?? setPinned.error ?? markRead.error ?? download.error;

  const action = (label: string, Icon: LucideIcon, onClick: () => void, pressed?: boolean) => (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-pressed={pressed}
      onClick={onClick}
      className={`hover:bg-background grid size-10 shrink-0 place-items-center rounded-full ${pressed ? 'text-primary' : ''}`}
    >
      <Icon aria-hidden size={22} />
    </button>
  );

  return (
    <>
      <div
        role="toolbar"
        aria-label={t('library.select.actions')}
        data-testid="selection-actions"
        className="bg-surface sticky bottom-2 z-30 mt-auto flex w-full justify-around overflow-x-auto rounded-2xl p-1 shadow-xl"
      >
        {action(t('library.select.categories'), FolderCog, () => setPanel('categories'))}
        {action(t('library.lists.title'), Bookmark, () => setPanel('list'))}
        {action(
          t(allFavorite ? 'library.select.unfavorite' : 'library.select.favorite'),
          Heart,
          () => setFavorite.mutate({ ids, favorite: !allFavorite }, finish),
          allFavorite,
        )}
        {action(
          t(allPinned ? 'library.select.unpin' : 'library.select.pin'),
          allPinned ? PinOff : Pin,
          () => setPinned.mutate({ ids, pinned: !allPinned }, finish),
        )}
        {action(
          t(video ? 'library.select.markWatched' : 'library.select.markRead'),
          BookCheck,
          () => markRead.mutate({ ids, read: true }, finish),
        )}
        {action(
          t(video ? 'library.select.markUnwatched' : 'library.select.markUnread'),
          BookX,
          () => markRead.mutate({ ids, read: false }, finish),
        )}
        {canDownload && action(t('library.select.download'), Download, () => setPanel('download'))}
        {action(t('library.select.remove'), Trash2, () => setPanel('remove'))}
      </div>
      {error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {error.message}
        </p>
      )}
      {panel === 'categories' && (
        <CategoriesSheet
          medium={medium}
          categories={categories}
          selected={selected}
          onDone={onDone}
          onClose={() => setPanel(null)}
        />
      )}
      {panel === 'list' && (
        <ListSheet
          medium={medium}
          current={currentList}
          onPick={(list) => {
            setPanel(null);
            setList.mutate({ ids, list }, finish);
          }}
          onClose={() => setPanel(null)}
        />
      )}
      {panel === 'download' && (
        <DownloadSheet
          onPick={(amount) => {
            setPanel(null);
            download.mutate({ ids, amount }, finish);
          }}
          onClose={() => setPanel(null)}
        />
      )}
      {panel === 'remove' && (
        <RemoveSheet
          medium={medium}
          selected={selected}
          onDone={onDone}
          onClose={() => setPanel(null)}
        />
      )}
    </>
  );
};
