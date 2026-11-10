import { commands } from '@raygun/bindings';
import type { MusicAlbum, ReleaseKind } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { ArrowUpDown, Check, LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { useLate } from '../shell/useLate';
import { AlbumTile } from '../search/MusicFinds';
import { useAlbumMenuItems } from './cardMenus';
import { Sheet } from '../shell/Sheet';
import { useDragScroll } from '../shell/useDragScroll';
import { RELEASE_SORTS, readReleaseSorts, saveReleaseSorts, sortReleases } from './releaseSort';
import type { ReleaseSort } from './releaseSort';
import { releaseReveal, useRevealIndex } from './reveal';

/** The chips of an artist's discography (M46.36), in Spotify's order; singles and EPs share one. */
type Shelf = 'all' | 'album' | 'single' | 'compilation' | 'live' | 'other';
const SHELVES: Shelf[] = ['all', 'album', 'single', 'compilation', 'live', 'other'];

const shelfOf = (kind: ReleaseKind | null | undefined): Exclude<Shelf, 'all'> => {
  switch (kind) {
    case 'album':
    case undefined:
    case null:
      return 'album';
    case 'single':
    case 'ep':
      return 'single';
    default:
      return kind;
  }
};

/** Releases shown before Show all. */
const SHOWN = 12;

const useArtistReleases = (name: string) =>
  useQuery({
    queryKey: ['music', 'artistReleases', name],
    queryFn: () => unwrapResult(commands.musicArtistReleases(name)),
    enabled: name.trim() !== '',
    staleTime: 10 * 60_000,
  });

/**
 * An artist's discography (M46.36): albums, singles and EPs, compilations and live albums from
 * the music sources (Deezer, MusicBrainz), picked with chips. Each chip has its own sort
 * (M46.45: newest, oldest, title, most songs), remembered on this device. A release asked for
 * (a new single's notification) is shown wherever it is: on its chip, past Show all.
 */
export const Discography: FC<{ name: string; title: string }> = ({ name, title }) => {
  const { t } = useTranslation();
  const menu = useAlbumMenuItems();
  const releases = useArtistReleases(name);
  const slow = useLate(releases.isPending);
  const [shelf, setShelf] = useState<Shelf>('all');
  const [all, setAll] = useState(false);
  const [sorts, setSorts] = useState(readReleaseSorts);
  const [sortAnchor, setSortAnchor] = useState<HTMLElement | null>(null);
  const list: MusicAlbum[] = releases.data ?? [];
  const wantedAt = useRevealIndex(list, releaseReveal);
  const wanted = wantedAt >= 0 ? list[wantedAt] : undefined;
  // On a chip that doesn't hold it, back to All, which holds every release.
  if (wanted && shelf !== 'all' && shelfOf(wanted.kind) !== shelf) setShelf('all');
  const chipRow = useDragScroll<HTMLDivElement>();
  const sort: ReleaseSort = sorts[shelf] ?? 'newest';
  const pickSort = (next: ReleaseSort) => {
    const saved = { ...sorts, [shelf]: next };
    setSorts(saved);
    saveReleaseSorts(saved);
    setSortAnchor(null);
  };
  if (releases.isPending) {
    return (
      <section aria-label={title} className="flex flex-col gap-3">
        <h2 className="text-lg font-bold">{title}</h2>
        {slow && (
          <LoaderCircle
            aria-label={t('artist.loadingReleases')}
            className="text-muted animate-spin"
          />
        )}
      </section>
    );
  }
  if (list.length === 0) return null;
  const counts = new Map<Shelf, number>();
  for (const release of list) {
    const kind = shelfOf(release.kind);
    counts.set(kind, (counts.get(kind) ?? 0) + 1);
  }
  const chips = SHELVES.filter((id) => id === 'all' || (counts.get(id) ?? 0) > 0);
  const shown = sortReleases(
    list.filter((release) => shelf === 'all' || shelfOf(release.kind) === shelf),
    sort,
  );
  if (wanted && !all && shown.indexOf(wanted) >= SHOWN) setAll(true);
  const chip = (on: boolean) =>
    `shrink-0 rounded-full px-3 py-1.5 text-sm ${on ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`;
  return (
    <section aria-label={title} data-testid="discography" className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <h2 className="min-w-0 flex-1 truncate text-lg font-bold">{title}</h2>
        <button
          type="button"
          aria-haspopup="dialog"
          aria-label={t('artist.sortButton', { sort: t(`artist.sort.${sort}`) })}
          onClick={(event) => setSortAnchor(sortAnchor ? null : event.currentTarget)}
          className="text-muted hover:text-foreground flex shrink-0 items-center gap-1.5 rounded-full px-3 py-1.5 text-sm"
        >
          {t(`artist.sort.${sort}`)}
          <ArrowUpDown aria-hidden size={15} />
        </button>
      </div>
      {sortAnchor && (
        <Sheet
          title={t('tracks.sortBy')}
          anchor={sortAnchor}
          popoverWidth={220}
          onClose={() => setSortAnchor(null)}
        >
          <div role="radiogroup" aria-label={t('tracks.sortBy')} className="-mx-2 flex flex-col">
            {RELEASE_SORTS.map((key) => (
              <button
                key={key}
                type="button"
                role="radio"
                aria-checked={sort === key}
                onClick={() => pickSort(key)}
                className={`hover:bg-background flex items-center justify-between rounded-lg px-3 py-2 text-sm ${sort === key ? 'text-primary' : ''}`}
              >
                {t(`artist.sort.${key}`)}
                {sort === key && <Check aria-hidden size={14} />}
              </button>
            ))}
          </div>
        </Sheet>
      )}
      {chips.length > 2 && (
        <div
          ref={chipRow}
          role="group"
          aria-label={t('artist.releaseKinds')}
          className="no-scrollbar -mx-4 flex gap-2 overflow-x-auto px-4"
        >
          {chips.map((id) => (
            <button
              key={id}
              type="button"
              aria-pressed={shelf === id}
              onClick={() => {
                setShelf(id);
                setAll(false);
              }}
              className={chip(shelf === id)}
            >
              {t(`artist.release.${id}`)}
            </button>
          ))}
        </div>
      )}
      <ul className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-3">
        {(all ? shown : shown.slice(0, SHOWN)).map((release) => (
          <li key={`${release.provider.id}:${release.id}`} data-reveal={releaseReveal(release)}>
            <AlbumTile
              album={release}
              menu={() => menu(release)}
              detail={[release.year, t(`artist.release.${shelfOf(release.kind)}One`)]
                .filter(Boolean)
                .join(' · ')}
            />
          </li>
        ))}
      </ul>
      {shown.length > SHOWN && (
        <button
          type="button"
          onClick={() => setAll(!all)}
          className="text-muted self-start text-sm font-semibold hover:underline"
        >
          {all ? t('artist.showLess') : t('artist.showAll', { count: shown.length })}
        </button>
      )}
    </section>
  );
};
