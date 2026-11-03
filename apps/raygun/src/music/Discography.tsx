import { commands } from '@raygun/bindings';
import type { MusicAlbum, ReleaseKind } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { AlbumTile } from '../search/MusicFinds';

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
 * the music sources (Deezer, MusicBrainz), newest first, picked with chips.
 */
export const Discography: FC<{ name: string; title: string }> = ({ name, title }) => {
  const { t } = useTranslation();
  const releases = useArtistReleases(name);
  const [shelf, setShelf] = useState<Shelf>('all');
  const [all, setAll] = useState(false);
  if (releases.isPending) {
    return (
      <section aria-label={title} className="flex flex-col gap-3">
        <h2 className="text-lg font-bold">{title}</h2>
        <LoaderCircle
          aria-label={t('artist.loadingReleases')}
          className="text-muted animate-spin"
        />
      </section>
    );
  }
  const list: MusicAlbum[] = releases.data ?? [];
  if (list.length === 0) return null;
  const counts = new Map<Shelf, number>();
  for (const release of list) {
    const kind = shelfOf(release.kind);
    counts.set(kind, (counts.get(kind) ?? 0) + 1);
  }
  const chips = SHELVES.filter((id) => id === 'all' || (counts.get(id) ?? 0) > 0);
  const shown = list.filter((release) => shelf === 'all' || shelfOf(release.kind) === shelf);
  const chip = (on: boolean) =>
    `shrink-0 rounded-full px-3 py-1.5 text-sm ${on ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`;
  return (
    <section aria-label={title} data-testid="discography" className="flex flex-col gap-3">
      <h2 className="text-lg font-bold">{title}</h2>
      {chips.length > 2 && (
        <div
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
          <li key={`${release.provider.id}:${release.id}`}>
            <AlbumTile
              album={release}
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
