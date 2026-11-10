import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { ListPlus, LoaderCircle, Plus, UserRound } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { COLLECTIONS, usePlaylists } from '../collections/useCollections';
import { ExportSheet } from '../export/ExportSheet';
import { unwrapResult } from '../lib/commandResult';
import { Artwork } from '../library/Artwork';
import { useArtistInfo } from '../player/musicInfo';
import { afterLayersClose } from '../shell/backLayers';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { showToast } from '../shell/toastStore';
import { DetailEditForm } from './DetailEditForm';
import { useMenuSheetsStore } from './menuSheetsStore';
import type { ArtistsToPick, DetailsToEdit, SongsToAdd } from './menuSheetsStore';

const ROW =
  'hover:bg-background flex h-12 w-full items-center gap-3 rounded-lg px-2 text-left text-sm pointer-coarse:h-auto pointer-coarse:min-h-[max(3rem,7dvh)] pointer-coarse:gap-4 pointer-coarse:text-[calc(clamp(1rem,4.3vw,1.125rem)*var(--text-scale,1))] pointer-coarse:[&>svg]:size-[calc(22px*var(--ui-scale,1))]';

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

/** One artist to go to: their picture (another source's when theirs is missing) and name. */
const ArtistTile: FC<{ name: string; onOpen: () => void }> = ({ name, onOpen }) => {
  const info = useArtistInfo(name);
  return (
    <button
      type="button"
      onClick={onOpen}
      className="hover:bg-background focus-visible:bg-background flex w-full min-w-0 flex-col items-center gap-2 rounded-2xl p-2 text-center outline-none"
    >
      <Artwork
        url={info.data?.imageUrl ?? null}
        // Asked only once their own is known to be missing.
        fallback={info.isPending ? null : { kind: 'artist', name }}
        icon={UserRound}
        className="w-full rounded-full shadow-md"
        iconSize={30}
      />
      <span className="line-clamp-2 w-full text-sm font-medium wrap-anywhere">{name}</span>
    </button>
  );
};

/** Go to artists: a song's or an album's artists as tiles, each opening the artist's page. */
const ArtistsSheet: FC<{ artists: ArtistsToPick; onClose: () => void }> = ({
  artists,
  onClose,
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  return (
    <Sheet
      title={t('menu.goToArtists')}
      onClose={onClose}
      anchor={artists.anchor}
      popoverWidth={380}
    >
      <ul
        data-testid="artist-tiles"
        className="grid max-h-[min(60dvh,28rem)] grid-cols-[repeat(auto-fill,minmax(min(7rem,40%),1fr))] gap-2 overflow-y-auto overscroll-contain"
      >
        {artists.names.map((name) => (
          <li key={name} className="min-w-0">
            <ArtistTile
              name={name}
              onOpen={() => {
                onClose();
                // After the sheet's history entry is gone (phones), so the page stays open.
                afterLayersClose(() => void navigate({ to: '/artist', search: { name } }));
              }}
            />
          </li>
        ))}
      </ul>
    </Sheet>
  );
};

/** Hosts the sheets that cards' menus open (mounted once by the shell). */
export const MenuSheets: FC = () => {
  const adding = useMenuSheetsStore((state) => state.adding);
  const editing = useMenuSheetsStore((state) => state.editing);
  const exporting = useMenuSheetsStore((state) => state.exporting);
  const artists = useMenuSheetsStore((state) => state.artists);
  const close = useMenuSheetsStore((state) => state.close);
  // Each is a Sheet: Back and Escape close it (shell/backLayers).
  if (adding) return <AddSongsSheet songs={adding} onClose={close} />;
  if (editing) return <EditDetailsSheet details={editing} onClose={close} />;
  if (exporting) {
    return (
      <ExportSheet
        medium={exporting.medium}
        titleIds={[exporting.titleId]}
        episodes={exporting.episodes}
        initialEpisodes={exporting.only}
        fixed={Boolean(exporting.only)}
        onClose={close}
      />
    );
  }
  if (artists) return <ArtistsSheet artists={artists} onClose={close} />;
  return null;
};
