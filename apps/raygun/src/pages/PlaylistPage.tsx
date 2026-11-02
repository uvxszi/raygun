import { commands } from '@raygun/bindings';
import type { PlaylistDetail, PlaylistItem } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate, useSearch } from '@tanstack/react-router';
import { ArrowDown, ArrowUp, Trash2, X } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { TopBar } from '../shell/TopBar';

import { PlayButtons } from '../collections/PlayButtons';
import { PlaylistCover } from '../collections/PlaylistCover';
import { savedRow } from '../music/tableView';
import { TrackTable } from '../music/TrackTable';
import { unwrapResult } from '../lib/commandResult';
import {
  COLLECTIONS,
  playCollection,
  useDeletePlaylist,
  useMovePlaylistItem,
  usePlaylist,
  useRemovePlaylistItem,
  useRenamePlaylist,
} from '../collections/useCollections';

const ICON_BUTTON =
  'text-muted grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-30';

/** Up, down and remove for one item while editing. */
const EditControls: FC<{ items: PlaylistItem[]; index: number }> = ({ items, index }) => {
  const { t } = useTranslation();
  const move = useMovePlaylistItem();
  const remove = useRemovePlaylistItem();
  const item = items[index] as PlaylistItem;
  const title = item.track.title;
  const busy = move.isPending || remove.isPending;
  return (
    <span className="flex">
      <button
        type="button"
        aria-label={t('collections.moveUp', { title })}
        disabled={busy || index === 0}
        onClick={() => move.mutate({ itemId: item.id, beforeItemId: items[index - 1]?.id ?? null })}
        className={ICON_BUTTON}
      >
        <ArrowUp aria-hidden size={18} />
      </button>
      <button
        type="button"
        aria-label={t('collections.moveDown', { title })}
        disabled={busy || index === items.length - 1}
        // Before the item after next, or to the end when the next one is last.
        onClick={() => move.mutate({ itemId: item.id, beforeItemId: items[index + 2]?.id ?? null })}
        className={ICON_BUTTON}
      >
        <ArrowDown aria-hidden size={18} />
      </button>
      <button
        type="button"
        aria-label={t('collections.remove', { title })}
        disabled={busy}
        onClick={() => remove.mutate(item.id)}
        className={ICON_BUTTON}
      >
        <X aria-hidden size={18} />
      </button>
    </span>
  );
};

/** Rename and delete, shown while editing. */
const PlaylistSettings: FC<{ detail: PlaylistDetail }> = ({ detail }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const rename = useRenamePlaylist();
  const remove = useDeletePlaylist();
  const [name, setName] = useState(detail.playlist.name);
  const [confirming, setConfirming] = useState(false);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    rename.mutate({ id: detail.playlist.id, name });
  };
  const error = rename.error ?? remove.error;

  return (
    <div className="bg-surface flex flex-col gap-3 rounded-xl p-4">
      <form onSubmit={submit} className="flex items-center gap-2">
        <input
          value={name}
          onChange={(event) => setName(event.target.value)}
          aria-label={t('collections.playlistName')}
          className="bg-background min-w-0 flex-1 rounded-lg px-3 py-2 text-sm outline-none"
        />
        <button
          type="submit"
          disabled={!name.trim() || name === detail.playlist.name || rename.isPending}
          className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-60"
        >
          {t('collections.save')}
        </button>
      </form>
      {confirming ? (
        <div className="flex flex-col gap-2 text-sm">
          <p>{t('collections.deleteConfirm', { name: detail.playlist.name })}</p>
          <div className="flex gap-2">
            <button
              type="button"
              disabled={remove.isPending}
              onClick={() =>
                remove.mutate(detail.playlist.id, {
                  onSuccess: () => void navigate({ to: '/library', search: { view: 'playlists' } }),
                })
              }
              className="bg-primary text-background rounded-full px-4 py-2 font-semibold"
            >
              {t('collections.delete')}
            </button>
            <button
              type="button"
              onClick={() => setConfirming(false)}
              className="bg-background rounded-full px-4 py-2"
            >
              {t('collections.cancel')}
            </button>
          </div>
        </div>
      ) : (
        <button
          type="button"
          onClick={() => setConfirming(true)}
          className="text-primary flex items-center gap-2 self-start text-sm"
        >
          <Trash2 aria-hidden size={16} />
          {t('collections.delete')}
        </button>
      )}
      {error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {error.message}
        </p>
      )}
    </div>
  );
};

export const PlaylistPage: FC = () => {
  const { t } = useTranslation();
  const { id } = useSearch({ from: '/library/playlist' });
  const playlist = usePlaylist(id);
  const client = useQueryClient();
  const [editing, setEditing] = useState(false);

  if (playlist.error) {
    return (
      <section data-testid="playlist-page">
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {playlist.error.message}
        </p>
      </section>
    );
  }
  const detail = playlist.data;
  if (!detail) {
    return <section data-testid="playlist-page" />;
  }
  const collection = { kind: 'playlist' as const, id };

  return (
    <section data-testid="playlist-page" className="flex flex-col gap-5">
      <TopBar back fallback="/library" plainTitle title={t('library.view.playlists')} />
      <header className="flex flex-col gap-4 sm:flex-row sm:items-end">
        <PlaylistCover
          artworkUrls={detail.playlist.artworkUrls}
          className="size-48 self-center rounded-xl shadow-lg sm:self-auto"
        />
        <div className="flex flex-col gap-1 text-center sm:text-left">
          <h1 className="text-2xl font-bold tracking-tight wrap-anywhere">
            {detail.playlist.name}
          </h1>
          <p className="text-muted text-xs">
            {t('library.trackCount', { count: detail.playlist.itemCount })}
          </p>
        </div>
      </header>
      <div className="flex items-center justify-center gap-3 sm:justify-start">
        <PlayButtons
          disabled={detail.items.length === 0}
          onPlay={(shuffle) => void playCollection({ collection, startIndex: null, shuffle })}
        />
        <button
          type="button"
          aria-pressed={editing}
          onClick={() => setEditing((on) => !on)}
          className="text-muted px-3 py-2.5 text-sm"
        >
          {t(editing ? 'collections.done' : 'collections.edit')}
        </button>
      </div>
      {editing && <PlaylistSettings detail={detail} />}
      {detail.items.length === 0 ? (
        <p className="text-muted text-sm">{t('collections.emptyPlaylist')}</p>
      ) : (
        <TrackTable
          kind="playlist"
          tracks={detail.items.map((item) => savedRow(item.track, item.id, item.id))}
          // A sorted or filtered table plays in the order it shows (M46.8).
          onPlay={(ordered, index) =>
            void playCollection({
              collection,
              startIndex: index,
              shuffle: false,
              order: ordered.map((row) => row.trackKey),
            })
          }
          onRemove={(removed) =>
            void Promise.all(
              removed.map((row) =>
                row.playlistItemId
                  ? unwrapResult(commands.playlistRemoveItem(row.playlistItemId))
                  : null,
              ),
            ).then(() => client.invalidateQueries({ queryKey: [COLLECTIONS] }))
          }
          actions={
            editing
              ? (row) => (
                  <EditControls
                    items={detail.items}
                    index={detail.items.findIndex((item) => item.id === row.rowKey)}
                  />
                )
              : undefined
          }
        />
      )}
    </section>
  );
};
