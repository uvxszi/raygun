import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { ArrowDown, ArrowUp, Check } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { Sheet } from '../shell/Sheet';
import { TextField } from '../shell/TextField';
import { PlaylistCover } from './PlaylistCover';
import { COLLECTIONS, usePlaylists } from './useCollections';

/**
 * Merge playlists (M46.45): pick two or more, in the order their songs should come (numbered,
 * moved with up and down), name the new playlist, keep each song once or not, and keep or remove
 * the merged ones.
 */
export const MergePlaylistsSheet: FC<{ picked: string[]; onClose: () => void }> = ({
  picked: first,
  onClose,
}) => {
  const { t } = useTranslation();
  const playlists = usePlaylists();
  const client = useQueryClient();
  const navigate = useNavigate();
  const [picked, setPicked] = useState(first);
  const [name, setName] = useState('');
  const [skipDuplicates, setSkipDuplicates] = useState(true);
  const [removeMerged, setRemoveMerged] = useState(false);
  const all = playlists.data ?? [];
  const nameOf = (id: string) => all.find((list) => list.id === id)?.name ?? '';
  const suggested = picked.map(nameOf).filter(Boolean).join(' + ');
  const merge = useMutation({
    mutationFn: () =>
      unwrapResult(
        commands.playlistsMerge(picked, name.trim() || suggested, skipDuplicates, removeMerged),
      ),
    onSuccess: (made) => {
      void client.invalidateQueries({ queryKey: [COLLECTIONS] });
      onClose();
      void navigate({ to: '/library/playlist', search: { id: made.id } });
    },
  });
  const toggle = (id: string) =>
    setPicked((list) => (list.includes(id) ? list.filter((other) => other !== id) : [...list, id]));
  const move = (index: number, by: number) =>
    setPicked((list) => {
      const next = [...list];
      const [id] = next.splice(index, 1);
      if (id) next.splice(index + by, 0, id);
      return next;
    });
  const small =
    'text-muted hover:text-foreground grid size-8 place-items-center rounded-full disabled:opacity-30';

  return (
    <Sheet title={t('collections.merge.title')} onClose={onClose}>
      <div data-testid="merge-playlists" className="flex flex-col gap-4">
        <ul className="flex max-h-80 flex-col gap-1 overflow-y-auto">
          {all.map((playlist) => {
            const order = picked.indexOf(playlist.id);
            const on = order >= 0;
            return (
              <li key={playlist.id} className="flex items-center gap-2">
                <button
                  type="button"
                  role="checkbox"
                  aria-checked={on}
                  onClick={() => toggle(playlist.id)}
                  className="hover:bg-background flex min-w-0 flex-1 items-center gap-3 rounded-lg p-1.5 text-left"
                >
                  <span
                    className={`grid size-6 shrink-0 place-items-center rounded-full text-xs font-semibold tabular-nums ${
                      on ? 'bg-primary text-background' : 'ring-muted ring-1'
                    }`}
                  >
                    {on ? order + 1 : ''}
                  </span>
                  <PlaylistCover
                    artworkUrls={playlist.artworkUrls}
                    artworkSongs={playlist.artworkSongs}
                    coverUrl={playlist.coverUrl}
                    className="size-10 shrink-0 rounded"
                  />
                  <span className="flex min-w-0 flex-col">
                    <span className="truncate text-sm font-medium">{playlist.name}</span>
                    <span className="text-muted text-xs">
                      {t('library.trackCount', { count: playlist.itemCount })}
                    </span>
                  </span>
                </button>
                {on && (
                  <>
                    <button
                      type="button"
                      aria-label={t('collections.moveUp', { title: playlist.name })}
                      disabled={order === 0}
                      onClick={() => move(order, -1)}
                      className={small}
                    >
                      <ArrowUp aria-hidden size={16} />
                    </button>
                    <button
                      type="button"
                      aria-label={t('collections.moveDown', { title: playlist.name })}
                      disabled={order === picked.length - 1}
                      onClick={() => move(order, 1)}
                      className={small}
                    >
                      <ArrowDown aria-hidden size={16} />
                    </button>
                  </>
                )}
              </li>
            );
          })}
        </ul>
        <TextField
          value={name}
          onChange={setName}
          aria-label={t('collections.merge.name')}
          placeholder={suggested || t('collections.merge.name')}
          className="bg-background rounded-xl pr-1.5 pl-4"
          inputClassName="py-2.5 text-sm"
        />
        {(
          [
            ['skip', skipDuplicates, setSkipDuplicates],
            ['remove', removeMerged, setRemoveMerged],
          ] as const
        ).map(([key, value, setValue]) => (
          <button
            key={key}
            type="button"
            role="switch"
            aria-checked={value}
            onClick={() => setValue(!value)}
            className="flex items-center justify-between gap-3 text-left text-sm"
          >
            {t(`collections.merge.${key}`)}
            <span
              className={`grid size-5 place-items-center rounded ${value ? 'bg-primary text-background' : 'ring-muted ring-1'}`}
            >
              {value && <Check aria-hidden size={14} />}
            </span>
          </button>
        ))}
        {merge.error && (
          <p role="alert" className="text-primary text-sm wrap-anywhere">
            {merge.error.message}
          </p>
        )}
        <button
          type="button"
          disabled={picked.length < 2 || merge.isPending}
          onClick={() => merge.mutate()}
          className="bg-primary text-background self-end rounded-full px-5 py-2.5 text-sm font-semibold disabled:opacity-50"
        >
          {t('collections.merge.action', { count: picked.length })}
        </button>
      </div>
    </Sheet>
  );
};
