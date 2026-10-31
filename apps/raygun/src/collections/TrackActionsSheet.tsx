import type { TrackInput } from '@raygun/bindings';
import { Heart, ListMinus, ListPlus, LoaderCircle, Plus } from 'lucide-react';
import { useCallback, useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { useCloseOnBack } from '../player/useCloseOnBack';
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

/** Favorite, playlist actions for one track. Mounted once; rows open it through the store. */
export const TrackActionsSheet: FC = () => {
  const { t } = useTranslation();
  const track = useTrackActionsStore((state) => state.track);
  const playlistItemId = useTrackActionsStore((state) => state.playlistItemId);
  const close = useTrackActionsStore((state) => state.close);
  const stableClose = useCallback(() => close(), [close]);
  useCloseOnBack(track !== null, stableClose);

  const favorite = useIsFavorite(track);
  const setFavorite = useSetFavorite();
  const remove = useRemovePlaylistItem();

  if (!track) {
    return null;
  }
  const isFavorite = favorite.data === true;

  return (
    <div
      className="fixed inset-0 z-[60] flex items-end justify-center bg-black/60 sm:items-center"
      onClick={(event) => {
        if (event.target === event.currentTarget) close();
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="track-actions-title"
        className="bg-surface flex w-full max-w-md flex-col gap-3 rounded-t-2xl px-4 pt-5 pb-[calc(1rem+env(safe-area-inset-bottom))] sm:rounded-2xl"
      >
        <div className="flex flex-col px-2">
          <h2 id="track-actions-title" className="truncate font-semibold">
            {track.title}
          </h2>
          <p className="text-muted truncate text-sm">
            {[track.artist, track.album].filter(Boolean).join(' · ')}
          </p>
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
        <button
          type="button"
          onClick={close}
          className="bg-background self-end rounded-full px-5 py-2 text-sm"
        >
          {t('collections.cancel')}
        </button>
      </div>
    </div>
  );
};
