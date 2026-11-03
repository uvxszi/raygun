import type { TrackInput } from '@raygun/bindings';
import { Heart, ListMinus, ListPlus, LoaderCircle, Plus } from 'lucide-react';
import { useCallback, useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { TrackTagChips } from '../music/TrackTagChips';
import { useTrackMenuItems } from '../music/useTrackMenu';
import { useCloseOnBack } from '../player/useCloseOnBack';
import { Popover } from '../shell/Popover';
import { useWideScreen } from '../shell/useWideScreen';
import { useTrackActionsStore } from './trackActionsStore';
import {
  useAddToPlaylist,
  useCreatePlaylist,
  useIsFavorite,
  usePlaylists,
  useRemovePlaylistItem,
  useSetFavorite,
} from './useCollections';

const ROW_CLASS = 'flex h-12 w-full items-center gap-3 rounded-lg px-2 text-left text-sm';

/** Picks a playlist for `track`, or makes a new one with it. */
const PlaylistPicker: FC<{ track: TrackInput; onDone: () => void }> = ({ track, onDone }) => {
  const { t } = useTranslation();
  const playlists = usePlaylists();
  const add = useAddToPlaylist();
  const create = useCreatePlaylist();
  const [naming, setNaming] = useState(false);
  const [name, setName] = useState('');

  const addTo = (id: string) => add.mutate({ id, tracks: [track] }, { onSuccess: onDone });

  const submit = (event: FormEvent) => {
    event.preventDefault();
    create.mutate(name, { onSuccess: (playlist) => addTo(playlist.id) });
  };

  const error = add.error ?? create.error;
  return (
    <div className="flex flex-col gap-1">
      <h3 className="text-muted px-2 text-xs font-medium tracking-wide uppercase">
        {t('collections.addToPlaylist')}
      </h3>
      {naming ? (
        <form onSubmit={submit} className="flex items-center gap-2 px-2">
          <input
            // Tapping "New playlist" asked for this field; focusing it saves a tap.
            autoFocus
            value={name}
            onChange={(event) => setName(event.target.value)}
            aria-label={t('collections.playlistName')}
            placeholder={t('collections.playlistName')}
            className="bg-background min-w-0 flex-1 rounded-lg px-3 py-2 text-sm outline-none"
          />
          <button
            type="submit"
            disabled={!name.trim() || create.isPending || add.isPending}
            className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-60"
          >
            {t('collections.create')}
          </button>
        </form>
      ) : (
        <button type="button" onClick={() => setNaming(true)} className={ROW_CLASS}>
          <Plus aria-hidden size={18} className="text-primary" />
          {t('collections.newPlaylist')}
        </button>
      )}
      <ul className="flex max-h-60 flex-col overflow-y-auto">
        {playlists.data?.map((playlist) => (
          <li key={playlist.id}>
            <button
              type="button"
              disabled={add.isPending}
              onClick={() => addTo(playlist.id)}
              className={ROW_CLASS}
            >
              <ListPlus aria-hidden size={18} className="text-muted" />
              <span className="min-w-0 flex-1 truncate">{playlist.name}</span>
              <span className="text-muted text-xs">
                {t('library.trackCount', { count: playlist.itemCount })}
              </span>
            </button>
          </li>
        ))}
      </ul>
      {error && (
        <p role="alert" className="text-primary px-2 text-sm wrap-anywhere">
          {error.message}
        </p>
      )}
    </div>
  );
};

/**
 * Favorite, playlist actions for one track. Mounted once; rows open it through the store. On wide
 * windows it pops up beside the button pressed (M46.7); on phones it's a bottom sheet.
 */
export const TrackActionsSheet: FC = () => {
  const track = useTrackActionsStore((state) => state.track);
  const anchor = useTrackActionsStore((state) => state.anchor);
  const close = useTrackActionsStore((state) => state.close);
  const stableClose = useCallback(() => close(), [close]);
  const wide = useWideScreen();
  useCloseOnBack(track !== null, stableClose);

  if (!track) {
    return null;
  }
  if (anchor && wide && anchor.isConnected) {
    return (
      <Popover anchor={anchor} label={track.title} onClose={stableClose} width={300}>
        <div className="flex flex-col gap-2 p-1">
          <TrackActions track={track} />
        </div>
      </Popover>
    );
  }
  return (
    <div
      className="motion-fade fixed inset-0 z-[60] flex items-end justify-center bg-black/60 sm:items-center"
      onClick={(event) => {
        if (event.target === event.currentTarget) close();
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="track-actions-title"
        className="bg-surface motion-rise flex w-full max-w-md flex-col gap-3 rounded-t-2xl px-4 pt-5 pb-[calc(1rem+env(safe-area-inset-bottom))] sm:rounded-2xl"
      >
        <TrackActions track={track} sheet />
      </div>
    </div>
  );
};

/** The actions themselves; `sheet` adds the bottom sheet's Cancel. */
const TrackActions: FC<{ track: TrackInput; sheet?: boolean }> = ({ track, sheet = false }) => {
  const { t } = useTranslation();
  const playlistItemId = useTrackActionsStore((state) => state.playlistItemId);
  const close = useTrackActionsStore((state) => state.close);
  const favorite = useIsFavorite(track);
  const setFavorite = useSetFavorite();
  const remove = useRemovePlaylistItem();
  const isFavorite = favorite.data === true;

  return (
    <>
      <div className="flex flex-col px-2">
        <h2 id="track-actions-title" className="truncate font-semibold">
          {track.title}
        </h2>
        <p className="text-muted truncate text-sm">
          {[track.artist, track.album].filter(Boolean).join(' · ')}
        </p>
        <TrackTagChips track={track} onOpen={close} className="pt-2" />
      </div>
      <button
        type="button"
        disabled={favorite.isPending || setFavorite.isPending}
        onClick={() =>
          setFavorite.mutate({ track, favorite: !isFavorite }, { onSuccess: () => close() })
        }
        className={ROW_CLASS}
      >
        {setFavorite.isPending ? (
          <LoaderCircle aria-hidden size={18} className="animate-spin" />
        ) : (
          <Heart
            aria-hidden
            size={18}
            className="text-primary"
            fill={isFavorite ? 'currentColor' : 'none'}
          />
        )}
        {t(isFavorite ? 'collections.unfavorite' : 'collections.favorite')}
      </button>
      {playlistItemId && (
        <button
          type="button"
          disabled={remove.isPending}
          onClick={() => remove.mutate(playlistItemId, { onSuccess: () => close() })}
          className={ROW_CLASS}
        >
          <ListMinus aria-hidden size={18} className="text-muted" />
          {t('collections.removeFromPlaylist')}
        </button>
      )}
      <PlaylistPicker track={track} onDone={close} />
      {sheet && <MoreActions track={track} onDone={close} />}
      {sheet && (
        <button
          type="button"
          onClick={close}
          className="bg-background self-end rounded-full px-5 py-2 text-sm"
        >
          {t('collections.cancel')}
        </button>
      )}
    </>
  );
};

/**
 * The rest of a song's menu on phones (M46.36): what a long press offers (play next, download,
 * go to the album or artist, edit, copy...), minus what the sheet already has above.
 */
const MoreActions: FC<{ track: TrackInput; onDone: () => void }> = ({ track, onDone }) => {
  const { t } = useTranslation();
  const menu = useTrackMenuItems();
  const above = new Set([
    t('collections.favorite'),
    t('collections.unfavorite'),
    t('collections.addToPlaylist'),
    t('collections.removeFromPlaylist'),
  ]);
  const items = menu(track, {}, { x: window.innerWidth / 2, y: window.innerHeight / 2 }).filter(
    (item) => item.kind !== 'heading' && !('label' in item && above.has(item.label)),
  );
  return (
    <div
      data-testid="track-more-actions"
      className="-mx-1 flex max-h-[40dvh] flex-col overflow-y-auto border-t border-white/10 pt-1"
    >
      {items.map((item, index) => {
        if (item.kind === 'separator') {
          // A line between groups, never first or doubled.
          return index > 0 && items[index - 1]?.kind !== 'separator' ? (
            <hr key={index} className="my-1 border-white/10" />
          ) : null;
        }
        if (item.kind === 'heading') return null;
        const Icon = item.icon;
        return (
          <button
            key={index}
            type="button"
            disabled={item.disabled}
            onClick={() => {
              onDone();
              item.onSelect();
            }}
            className={`${ROW_CLASS} ${item.danger ? 'text-red-400' : ''}`}
          >
            {Icon && <Icon aria-hidden size={18} className="text-muted" />}
            <span className="truncate">{item.label}</span>
          </button>
        );
      })}
    </div>
  );
};
