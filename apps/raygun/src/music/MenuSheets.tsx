import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ListPlus, LoaderCircle, Plus } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { COLLECTIONS, usePlaylists } from '../collections/useCollections';
import { unwrapResult } from '../lib/commandResult';
import { useCloseOnBack } from '../player/useCloseOnBack';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { showToast } from '../shell/toastStore';
import { DetailEditForm } from './DetailEditForm';
import { useMenuSheetsStore } from './menuSheetsStore';
import type { DetailsToEdit, SongsToAdd } from './menuSheetsStore';

const ROW =
  'hover:bg-background flex h-12 w-full items-center gap-3 rounded-lg px-2 text-left text-sm';

/** Adds an album's, a mix's or a playlist's songs to a playlist, or to a new one. */
const AddSongsSheet: FC<{ songs: SongsToAdd; onClose: () => void }> = ({ songs, onClose }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const playlists = usePlaylists();
  const [naming, setNaming] = useState(false);
  const [name, setName] = useState(songs.title);
  const add = useMutation({
    mutationFn: async (target: { id: string; name: string } | { name: string }) => {
      const tracks = await songs.load();
      if (tracks.length === 0) throw new Error(t('menu.noSongs', { name: songs.title }));
      const playlist =
        'id' in target ? target : await unwrapResult(commands.playlistCreate(target.name));
      // Songs the playlist holds already stay out (M46.48): the count is what was added.
      const added = await unwrapResult(commands.playlistAdd(playlist.id, tracks, false));
      return { count: added.added, name: playlist.name };
    },
    onSuccess: ({ count, name: playlist }) => {
      void client.invalidateQueries({ queryKey: [COLLECTIONS] });
      showToast(t('menu.addedToPlaylist', { count, name: playlist }));
      onClose();
    },
  });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (name.trim()) add.mutate({ name: name.trim() });
  };
  return (
    <Sheet title={t('collections.addToPlaylist')} onClose={onClose}>
      <p className="text-muted -mt-2 truncate text-sm">{songs.title}</p>
      <div className="flex flex-col gap-1" aria-busy={add.isPending}>
        {naming ? (
          <form onSubmit={submit} className="flex items-center gap-2">
            <input
              // Tapping "New playlist" asked for this field.
              autoFocus
              value={name}
              onChange={(event) => setName(event.target.value)}
              aria-label={t('collections.playlistName')}
              placeholder={t('collections.playlistName')}
              className="bg-background min-w-0 flex-1 rounded-lg px-3 py-2 text-sm outline-none"
            />
            <button
              type="submit"
              disabled={!name.trim() || add.isPending}
              className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-60"
            >
              {t('collections.create')}
            </button>
          </form>
        ) : (
          <button type="button" onClick={() => setNaming(true)} className={ROW}>
            <Plus aria-hidden size={18} className="text-primary" />
            {t('collections.newPlaylist')}
          </button>
        )}
        <ul className="flex max-h-72 flex-col overflow-y-auto">
          {playlists.data?.map((playlist) => (
            <li key={playlist.id}>
              <button
                type="button"
                disabled={add.isPending}
                onClick={() => add.mutate({ id: playlist.id, name: playlist.name })}
                className={ROW}
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
        {add.isPending && (
          <LoaderCircle aria-label={t('menu.adding')} className="text-muted animate-spin" />
        )}
        {add.error && <SourceIssue compact>{add.error.message}</SourceIssue>}
      </div>
    </Sheet>
  );
};

/** Edit details (M46.30) for an album or an artist, opened from a card's menu. */
const EditDetailsSheet: FC<{ details: DetailsToEdit; onClose: () => void }> = ({
  details,
  onClose,
}) => {
  const { t } = useTranslation();
  return details.kind === 'album' ? (
    <Sheet title={t('details.editAlbum')} onClose={onClose}>
      <DetailEditForm
        target={{ kind: 'album', artist: details.artist, album: details.title }}
        fields={['title']}
        original={{ title: details.title, artworkUrl: details.artworkUrl }}
        titleLabel={t('details.albumTitle')}
        onSaved={onClose}
      />
    </Sheet>
  ) : (
    <Sheet title={t('details.editArtist')} onClose={onClose}>
      <DetailEditForm
        target={{ kind: 'artist', name: details.name }}
        fields={['title']}
        original={{ title: details.name, artworkUrl: details.artworkUrl }}
        titleLabel={t('details.artistName')}
        onSaved={onClose}
      />
    </Sheet>
  );
};

/** Hosts the sheets that cards' menus open (mounted once by the shell). */
export const MenuSheets: FC = () => {
  const adding = useMenuSheetsStore((state) => state.adding);
  const editing = useMenuSheetsStore((state) => state.editing);
  const close = useMenuSheetsStore((state) => state.close);
  useCloseOnBack(adding !== null || editing !== null, close);
  if (adding) return <AddSongsSheet songs={adding} onClose={close} />;
  if (editing) return <EditDetailsSheet details={editing} onClose={close} />;
  return null;
};
