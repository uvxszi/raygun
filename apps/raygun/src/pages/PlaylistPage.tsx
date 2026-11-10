import { commands } from '@raygun/bindings';
import type { PlaylistDetail, PlaylistItem } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate, useSearch } from '@tanstack/react-router';
import { ImageIcon, Pencil, Trash2, X } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { TopBar } from '../shell/TopBar';
import { PinButton } from '../shell/PinButton';
import { Sheet } from '../shell/Sheet';
import { TextField } from '../shell/TextField';
import { useCoarsePointer } from '../shell/useWideScreen';
import { DetailEditForm } from '../music/DetailEditForm';

import { PlayButtons } from '../collections/PlayButtons';
import { PlaylistCover } from '../collections/PlaylistCover';
import { joinCount, useSongsLength } from '../music/collectionLength';
import { savedRow } from '../music/tableView';
import { TrackTable } from '../music/TrackTable';
import { unwrapResult } from '../lib/commandResult';
import {
  COLLECTIONS,
  playCollection,
  useDeletePlaylist,
  usePlaylist,
  useRemovePlaylistItem,
  useRenamePlaylist,
  useReorderPlaylist,
} from '../collections/useCollections';
import { usePlayContext } from '../music/playContext';

const ICON_BUTTON =
  'text-muted grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-30';

/**
 * Remove for one item while editing; the table puts its up and down buttons beside it (a hold
 * moves the song to the top or the bottom).
 */
const RemoveItem: FC<{ item: PlaylistItem }> = ({ item }) => {
  const { t } = useTranslation();
  const remove = useRemovePlaylistItem();
  return (
    <button
      type="button"
      aria-label={t('collections.remove', { title: item.track.title })}
      disabled={remove.isPending}
      onClick={() => remove.mutate(item.id)}
      className={ICON_BUTTON}
    >
      <X aria-hidden size={18} />
    </button>
  );
};

/**
 * The Edit popover (M46.46): rename, change cover, edit mode (grips, move and remove buttons),
 * delete.
 */
const PlaylistSettings: FC<{
  detail: PlaylistDetail;
  editing: boolean;
  onEditing: (on: boolean) => void;
}> = ({ detail, editing, onEditing }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const rename = useRenamePlaylist();
  const remove = useDeletePlaylist(() =>
    navigate({ to: '/library', search: { view: 'playlists' } }),
  );
  const [name, setName] = useState(detail.playlist.name);
  const [confirming, setConfirming] = useState(false);
  const [coverOpen, setCoverOpen] = useState(false);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    rename.mutate({ id: detail.playlist.id, name });
  };
  const error = rename.error ?? remove.error;

  return (
    <div data-testid="playlist-edit" className="flex flex-col gap-3">
      <form onSubmit={submit} className="flex items-center gap-2">
        <TextField
          value={name}
          onChange={setName}
          aria-label={t('collections.playlistName')}
          className="bg-background flex-1 rounded-lg pr-0.5 pl-3"
          inputClassName="py-2 text-sm"
        />
        <button
          type="submit"
          disabled={!name.trim() || name === detail.playlist.name || rename.isPending}
          className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-60"
        >
          {t('collections.save')}
        </button>
      </form>
      <button
        type="button"
        onClick={() => setCoverOpen(true)}
        className="flex items-center gap-2 self-start text-sm"
      >
        <ImageIcon aria-hidden size={16} />
        {t('collections.changeCover')}
      </button>
      <button
        type="button"
        role="switch"
        aria-checked={editing}
        onClick={() => onEditing(!editing)}
        className="flex items-center justify-between gap-3 text-left text-sm"
      >
        {t('collections.rowButtons')}
        <span
          aria-hidden
          className={`flex h-5 w-9 items-center rounded-full p-0.5 transition ${editing ? 'bg-primary justify-end' : 'bg-background justify-start'}`}
        >
          <span className="bg-foreground size-4 rounded-full" />
        </span>
      </button>
      {coverOpen && (
        <Sheet title={t('collections.changeCover')} onClose={() => setCoverOpen(false)}>
          <DetailEditForm
            target={{ kind: 'playlist', id: detail.playlist.id }}
            fields={[]}
            titleLabel=""
            original={{ artworkUrl: detail.playlist.artworkUrls[0] ?? null }}
            onSaved={() => setCoverOpen(false)}
          />
        </Sheet>
      )}
      {confirming ? (
        <div className="flex flex-col gap-2 text-sm">
          <p>{t('collections.deleteConfirm', { name: detail.playlist.name })}</p>
          <div className="flex gap-2">
            <button
              type="button"
              disabled={remove.isPending}
              onClick={() => remove.mutate(detail.playlist.id)}
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
  const summary = playlist.data?.playlist;
  usePlayContext(
    summary
      ? {
          kind: 'playlist',
          name: summary.name,
          subtitle: null,
          artworkUrl: summary.artworkUrls[0] ?? null,
          to: '/library/playlist',
          search: { id },
        }
      : null,
  );
  const client = useQueryClient();
  const reorder = useReorderPlaylist(id);
  const [editing, setEditing] = useState(false);
  const [editAnchor, setEditAnchor] = useState<HTMLElement | null>(null);
  // On touch screens the Edit sheet stands on its own, never tied to the list (M46.49).
  const touch = useCoarsePointer();
  const songs = useMemo(
    () => playlist.data?.items.map((item) => item.track) ?? [],
    [playlist.data],
  );
  const length = useSongsLength(songs);

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
      <TopBar
        back
        fallback="/library"
        plainTitle
        title={t('library.view.playlists')}
        actions={
          <PinButton
            target={{
              kind: 'playlist',
              to: '/library/playlist',
              search: { id: detail.playlist.id },
              name: detail.playlist.name,
              artworkUrl: detail.playlist.coverUrl ?? detail.playlist.artworkUrls[0] ?? null,
            }}
          />
        }
      />
      <header className="flex flex-col gap-4 sm:flex-row sm:items-end">
        <PlaylistCover
          artworkUrls={detail.playlist.artworkUrls}
          coverUrl={detail.playlist.coverUrl}
          className="size-48 self-center rounded-xl shadow-lg sm:self-auto"
        />
        <div className="flex flex-col gap-1 text-center sm:text-left">
          <h1 className="text-2xl font-bold tracking-tight wrap-anywhere">
            {detail.playlist.name}
          </h1>
          <p className="text-muted text-xs">
            {joinCount(t('library.trackCount', { count: detail.playlist.itemCount }), length)}
          </p>
        </div>
      </header>
      <div className="flex items-center justify-center gap-3 sm:justify-start">
        <PlayButtons
          disabled={detail.items.length === 0}
          onPlay={(shuffle) => void playCollection({ collection, startIndex: null, shuffle })}
        />
        {/* A full pill with its options beside it (M46.46). */}
        <button
          type="button"
          aria-haspopup="dialog"
          aria-expanded={editAnchor !== null}
          onClick={(event) => setEditAnchor(editAnchor ? null : event.currentTarget)}
          className="bg-surface hover:bg-background flex items-center gap-2 rounded-full px-4 py-2.5 text-sm font-semibold"
        >
          <Pencil aria-hidden size={16} />
          {t('collections.edit')}
        </button>
      </div>
      {editAnchor && (
        <Sheet
          title={t('collections.editPlaylist')}
          anchor={touch ? null : editAnchor}
          popoverWidth={360}
          onClose={() => setEditAnchor(null)}
        >
          <PlaylistSettings detail={detail} editing={editing} onEditing={setEditing} />
        </Sheet>
      )}
      {detail.items.length === 0 ? (
        <p className="text-muted text-sm">{t('collections.emptyPlaylist')}</p>
      ) : (
        <TrackTable
          kind="playlist"
          tracks={detail.items.map((item) => savedRow(item.track, item.id, item.id))}
          onReorder={(change) => reorder.mutate(change)}
          editing={editing}
          onEditingChange={setEditing}
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
              ? (row) => {
                  const item = detail.items.find((each) => each.id === row.rowKey);
                  return item ? <RemoveItem item={item} /> : null;
                }
              : undefined
          }
        />
      )}
    </section>
  );
};
