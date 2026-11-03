import type { TrackInput, TrackSaves } from '@raygun/bindings';
import { CircleCheck, Heart, Plus, Search } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { PlaylistCover } from './PlaylistCover';
import { useCreatePlaylist, usePlaylists, useSetTrackSaves, useTrackSaves } from './useCollections';

const CHECK = 'grid size-5 shrink-0 place-items-center rounded-full border-2 transition-colors';

/** A tick box drawn like Spotify's: a ring, filled with a check when ticked. */
const Tick: FC<{ on: boolean }> = ({ on }) =>
  on ? (
    <CircleCheck aria-hidden size={22} className="text-primary shrink-0" />
  ) : (
    <span aria-hidden className={`${CHECK} border-muted`} />
  );

/**
 * Spotify's Add to playlist (M46.8): find a playlist, make one on the spot, and tick where the
 * song is saved (Favorites and playlists). Done saves the ticks; Cancel leaves things as they were.
 */
const SaveSheet: FC<{
  track: TrackInput;
  saves: TrackSaves;
  anchor: HTMLElement;
  onClose: () => void;
}> = ({ track, saves, anchor, onClose }) => {
  const { t } = useTranslation();
  const playlists = usePlaylists();
  const create = useCreatePlaylist();
  const save = useSetTrackSaves();
  const [draft, setDraft] = useState<TrackSaves>(saves);
  const [filter, setFilter] = useState('');
  const [naming, setNaming] = useState(false);
  const [name, setName] = useState('');
  const toggle = (id: string) =>
    setDraft((current) => ({
      ...current,
      playlistIds: current.playlistIds.includes(id)
        ? current.playlistIds.filter((other) => other !== id)
        : [...current.playlistIds, id],
    }));
  const makePlaylist = (event: FormEvent) => {
    event.preventDefault();
    create.mutate(name, {
      onSuccess: (playlist) => {
        setDraft((current) => ({ ...current, playlistIds: [...current.playlistIds, playlist.id] }));
        setNaming(false);
        setName('');
      },
    });
  };
  const shown = (playlists.data ?? []).filter((playlist) =>
    playlist.name.toLowerCase().includes(filter.trim().toLowerCase()),
  );
  // Playlists already holding the song come first, as in Spotify's "Saved in".
  const savedIn = shown.filter((playlist) => saves.playlistIds.includes(playlist.id));
  const others = shown.filter((playlist) => !saves.playlistIds.includes(playlist.id));
  const row = (
    id: string,
    label: string,
    detail: string,
    cover: ReactNode,
    on: boolean,
    onToggle: () => void,
  ) => (
    <button
      key={id}
      type="button"
      role="checkbox"
      aria-checked={on}
      aria-label={label}
      onClick={onToggle}
      className="hover:bg-background flex w-full items-center gap-3 rounded-lg px-2 py-1.5 text-left"
    >
      {cover}
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-sm font-medium">{label}</span>
        <span className="text-muted truncate text-xs">{detail}</span>
      </span>
      <Tick on={on} />
    </button>
  );

  return (
    <Sheet
      title={t('collections.addToPlaylist')}
      anchor={anchor}
      popoverWidth={320}
      onClose={onClose}
    >
      <div className="flex flex-col gap-2">
        <label className="bg-background flex items-center gap-2 rounded-lg px-3 py-2">
          <Search aria-hidden size={16} className="text-muted" />
          <input
            type="search"
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
            placeholder={t('collections.findPlaylist')}
            aria-label={t('collections.findPlaylist')}
            className="placeholder:text-muted min-w-0 flex-1 bg-transparent text-sm outline-none"
          />
        </label>
        {naming ? (
          <form onSubmit={makePlaylist} className="flex items-center gap-2">
            <input
              autoFocus
              value={name}
              onChange={(event) => setName(event.target.value)}
              aria-label={t('collections.playlistName')}
              placeholder={t('collections.playlistName')}
              className="bg-background min-w-0 flex-1 rounded-lg px-3 py-2 text-sm outline-none"
            />
            <button
              type="submit"
              disabled={!name.trim() || create.isPending}
              className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-60"
            >
              {t('collections.create')}
            </button>
          </form>
        ) : (
          <button
            type="button"
            onClick={() => setNaming(true)}
            className="hover:bg-background flex items-center gap-3 rounded-lg px-2 py-2 text-sm font-medium"
          >
            <Plus aria-hidden size={18} />
            {t('collections.newPlaylist')}
          </button>
        )}
        <div className="flex max-h-72 flex-col overflow-y-auto">
          <h3 className="text-muted px-2 pt-1 pb-1 text-xs font-semibold">
            {t('collections.savedIn')}
          </h3>
          {row(
            'favorites',
            t('collections.favorites'),
            t('collections.favoritesDetail'),
            <span className="from-primary/80 to-primary/30 grid size-10 shrink-0 place-items-center rounded bg-gradient-to-br">
              <Heart aria-hidden size={18} className="text-background" fill="currentColor" />
            </span>,
            draft.favorite,
            () => setDraft((current) => ({ ...current, favorite: !current.favorite })),
          )}
          {savedIn.map((playlist) =>
            row(
              playlist.id,
              playlist.name,
              t('collections.songs', { count: playlist.itemCount }),
              <PlaylistCover
                artworkUrls={playlist.artworkUrls}
                coverUrl={playlist.coverUrl}
                className="size-10 shrink-0 rounded"
              />,
              draft.playlistIds.includes(playlist.id),
              () => toggle(playlist.id),
            ),
          )}
          {others.length > 0 && (
            <h3 className="text-muted px-2 pt-3 pb-1 text-xs font-semibold">
              {t('collections.yourPlaylists')}
            </h3>
          )}
          {others.map((playlist) =>
            row(
              playlist.id,
              playlist.name,
              t('collections.songs', { count: playlist.itemCount }),
              <PlaylistCover
                artworkUrls={playlist.artworkUrls}
                coverUrl={playlist.coverUrl}
                className="size-10 shrink-0 rounded"
              />,
              draft.playlistIds.includes(playlist.id),
              () => toggle(playlist.id),
            ),
          )}
        </div>
        {save.error && (
          <p role="alert" className="text-primary text-xs wrap-anywhere">
            {save.error.message}
          </p>
        )}
        <div className="flex justify-end gap-2 pt-1">
          <button type="button" onClick={onClose} className="rounded-full px-4 py-2 text-sm">
            {t('collections.cancel')}
          </button>
          <button
            type="button"
            disabled={save.isPending}
            onClick={() => save.mutate({ track, saves: draft }, { onSuccess: onClose })}
            className="bg-primary text-background rounded-full px-5 py-2 text-sm font-semibold disabled:opacity-60"
          >
            {t('collections.done')}
          </button>
        </div>
      </div>
    </Sheet>
  );
};

/**
 * Saving a song (M46.8), Spotify's way: a Plus that adds it to Favorites at once and opens the
 * Add to playlist popover; a check once it's saved anywhere, which opens the popover to change it.
 */
export const SaveButton: FC<{ track: TrackInput; compact?: boolean; className?: string }> = ({
  track,
  compact = false,
  className = '',
}) => {
  const { t } = useTranslation();
  const saves = useTrackSaves(track);
  const save = useSetTrackSaves();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const current = saves.data;
  const saved = current ? current.favorite || current.playlistIds.length > 0 : false;
  const size = compact ? 18 : 22;

  const press = (button: HTMLElement) => {
    if (anchor) {
      setAnchor(null);
      return;
    }
    if (current && !saved) {
      // Saved to Favorites straight away; the popover offers playlists too.
      save.mutate({ track, saves: { ...current, favorite: true } });
    }
    setAnchor(button);
  };

  return (
    <>
      <button
        type="button"
        aria-label={t(saved ? 'collections.saved' : 'collections.save')}
        aria-pressed={saved}
        aria-haspopup="dialog"
        disabled={!current}
        onClick={(event) => press(event.currentTarget)}
        className={`grid shrink-0 place-items-center rounded-full ${compact ? 'size-9' : 'size-11'} ${saved ? 'text-primary' : 'text-muted hover:text-foreground'} ${className}`}
      >
        {saved ? <CircleCheck aria-hidden size={size} /> : <Plus aria-hidden size={size} />}
      </button>
      {anchor && current && (
        <SaveSheet
          track={track}
          saves={saved ? current : { ...current, favorite: true }}
          anchor={anchor}
          onClose={() => setAnchor(null)}
        />
      )}
    </>
  );
};
