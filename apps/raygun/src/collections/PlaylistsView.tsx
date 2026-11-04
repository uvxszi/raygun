import { Link } from '@tanstack/react-router';
import { Merge, Plus } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { describeLength, joinCount, playlistLengthSummary } from '../music/collectionLength';
import { Sheet } from '../shell/Sheet';
import { TextField } from '../shell/TextField';
import { useCollectionSheets } from './collectionSheets';
import { PlaylistCover } from './PlaylistCover';
import { useCreatePlaylist, usePlaylists } from './useCollections';
import { usePlaylistMenu } from './usePlaylistMenu';

export const NewPlaylistForm: FC<{ onDone: () => void }> = ({ onDone }) => {
  const { t } = useTranslation();
  const create = useCreatePlaylist();
  const [name, setName] = useState('');
  const submit = (event: FormEvent) => {
    event.preventDefault();
    create.mutate(name, { onSuccess: onDone });
  };
  return (
    <Sheet title={t('collections.newPlaylist')} onClose={onDone}>
      <form onSubmit={submit} className="flex flex-col gap-3">
        <div className="flex flex-col gap-3">
          <TextField
            // Tapping "New playlist" asked for this field; focusing it saves a tap.
            autoFocus
            value={name}
            onChange={setName}
            aria-label={t('collections.playlistName')}
            placeholder={t('collections.playlistName')}
            className="bg-background rounded-xl pr-1.5 pl-4"
            inputClassName="py-3 text-sm"
          />
          <button
            type="submit"
            disabled={!name.trim() || create.isPending}
            className="bg-primary text-background self-end rounded-full px-5 py-2.5 text-sm font-semibold disabled:opacity-60"
          >
            {t('collections.create')}
          </button>
        </div>
        {create.error && (
          <p role="alert" className="text-primary text-sm wrap-anywhere">
            {create.error.message}
          </p>
        )}
      </form>
    </Sheet>
  );
};

export const PlaylistsView: FC = () => {
  const { t, i18n } = useTranslation();
  const playlists = usePlaylists();
  const [creating, setCreating] = useState(false);
  const openMerge = useCollectionSheets((state) => state.openMerge);
  const menu = usePlaylistMenu();

  return (
    <div data-testid="playlists-view" className="flex flex-col gap-3">
      {creating && <NewPlaylistForm onDone={() => setCreating(false)} />}
      {
        <button
          type="button"
          onClick={() => setCreating(true)}
          className="flex h-14 items-center gap-3 text-left text-sm font-medium"
        >
          <span className="bg-surface text-primary grid size-12 place-items-center rounded-lg">
            <Plus aria-hidden size={22} />
          </span>
          {t('collections.newPlaylist')}
        </button>
      }
      {(playlists.data?.length ?? 0) >= 2 && (
        <button
          type="button"
          onClick={() => openMerge()}
          className="flex h-14 items-center gap-3 text-left text-sm font-medium"
        >
          <span className="bg-surface text-primary grid size-12 place-items-center rounded-lg">
            <Merge aria-hidden size={22} />
          </span>
          {t('collections.merge.title')}
        </button>
      )}
      {playlists.data?.length === 0 && (
        <p className="text-muted text-sm">{t('collections.noPlaylists')}</p>
      )}
      <ul className="flex flex-col gap-1">
        {playlists.data?.map((playlist) => (
          <li key={playlist.id}>
            <Link
              to="/library/playlist"
              search={{ id: playlist.id }}
              data-testid="playlist-link"
              onContextMenu={menu(playlist)}
              className="flex h-16 items-center gap-3"
            >
              <PlaylistCover
                artworkUrls={playlist.artworkUrls}
                coverUrl={playlist.coverUrl}
                className="size-12 rounded-lg"
              />
              <span className="flex min-w-0 flex-col">
                <span className="truncate text-sm font-medium">{playlist.name}</span>
                <span className="text-muted text-xs">
                  {joinCount(
                    t('library.trackCount', { count: playlist.itemCount }),
                    describeLength(
                      playlistLengthSummary(playlist.length ?? undefined, playlist.itemCount),
                      t,
                      i18n.language,
                    ),
                  )}
                </span>
              </span>
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
};
