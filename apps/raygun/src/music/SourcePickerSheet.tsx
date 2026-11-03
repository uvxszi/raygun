import { commands } from '@raygun/bindings';
import type { OnlineTrack, TrackInput } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { LoaderCircle, Play, Search } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { COLLECTIONS } from '../collections/useCollections';
import { unwrapResult } from '../lib/commandResult';
import { formatTime } from '../player/formatTime';
import { rankSongs } from '../search/useMusicFinds';
import { playResults, useOnlineSearch } from '../search/useOnlineSearch';
import { Sheet } from '../shell/Sheet';
import { useSourcePickerStore } from './sourcePickerStore';
import { SourceIssue } from '../shell/SourceIssue';

/**
 * Find a source (M46.31): every source's answer for the song, with a preview, and Use this,
 * which every saved copy of the song plays from afterwards. The title and artist can be fixed
 * and searched again (M46.45: screenshots get names wrong); using a source then keeps the fixed
 * names as the song's details.
 */
export const SourcePickerSheet: FC = () => {
  const song = useSourcePickerStore((store) => store.song);
  const close = useSourcePickerStore((store) => store.close);
  if (!song) return null;
  return (
    <SourcePickerBody key={`${song.artist ?? ''}|${song.title}`} song={song} onClose={close} />
  );
};

const SourcePickerBody: FC<{ song: TrackInput; onClose: () => void }> = ({ song, onClose }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [names, setNames] = useState({ title: song.title, artist: song.artist ?? '' });
  const [query, setQuery] = useState([song.artist, song.title].filter(Boolean).join(' '));
  const search = useOnlineSearch(query);
  const renamed = names.title.trim() !== song.title || names.artist.trim() !== (song.artist ?? '');
  const providers = (search.data?.providers ?? []).filter((result) => !result.error);
  // The merged top results don't say where they're from: find them in a source's own list.
  const ranked = rankSongs(query, search.data?.top ?? [], providers)
    .slice(0, 12)
    .map((entry) => ({
      ...entry,
      source:
        entry.source ??
        providers.find((result) =>
          result.tracks.some(
            (track) =>
              (track.externalId !== null && track.externalId === entry.track.externalId) ||
              (track.title === entry.track.title && track.artist === entry.track.artist),
          ),
        )?.provider.name,
    }));
  const use = useMutation({
    mutationFn: async (chosen: { track: OnlineTrack; source: string | undefined }) => {
      await unwrapResult(commands.trackAssignSource(song, chosen.track));
      if (!renamed || !names.title.trim()) return;
      // The fixed names show everywhere the song does (Edit details, M46.30).
      const target = {
        kind: 'song' as const,
        query: { title: song.title, artist: song.artist, album: song.album, durationMs: null },
      };
      const edit = await unwrapResult(commands.detailEdit(target));
      await unwrapResult(
        commands.detailEditSet(target, {
          ...edit,
          title: names.title.trim(),
          artist: names.artist.trim() || null,
        }),
      );
    },
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: [COLLECTIONS] });
      void client.invalidateQueries({ queryKey: ['unavailable'] });
    },
  });

  return (
    <Sheet title={t('sourcePicker.title')} onClose={onClose}>
      <div data-testid="source-picker" className="flex flex-col gap-3">
        <form
          className="flex flex-wrap items-center gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            setQuery([names.artist.trim(), names.title.trim()].filter(Boolean).join(' '));
          }}
        >
          <input
            aria-label={t('details.songTitle')}
            placeholder={t('details.songTitle')}
            value={names.title}
            onChange={(event) => setNames({ ...names, title: event.target.value })}
            className="bg-background min-w-0 flex-1 basis-40 rounded-xl px-3 py-2 text-sm"
          />
          <input
            aria-label={t('details.artist')}
            placeholder={t('details.artist')}
            value={names.artist}
            onChange={(event) => setNames({ ...names, artist: event.target.value })}
            className="bg-background min-w-0 flex-1 basis-32 rounded-xl px-3 py-2 text-sm"
          />
          <button
            type="submit"
            disabled={!names.title.trim()}
            className="bg-primary text-background flex items-center gap-1.5 rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-50"
          >
            <Search aria-hidden size={14} />
            {t('sourcePicker.search')}
          </button>
        </form>
        {search.isPending && (
          <p className="text-muted flex items-center gap-2 text-sm">
            <LoaderCircle aria-hidden size={14} className="animate-spin" />
            {t('sourcePicker.searching')}
          </p>
        )}
        {search.isSuccess && ranked.length === 0 && (
          <p className="text-muted text-sm">{t('sourcePicker.none')}</p>
        )}
        <ul className="flex max-h-[50vh] flex-col overflow-y-auto">
          {ranked.map((entry, index) => (
            <li key={`${entry.source ?? ''}-${index}`} className="flex items-center gap-2 py-1.5">
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="truncate text-sm font-medium">{entry.track.title}</span>
                <span className="text-muted truncate text-xs">
                  {[
                    entry.track.artist,
                    entry.source,
                    entry.track.durationMs ? formatTime(entry.track.durationMs) : null,
                  ]
                    .filter(Boolean)
                    .join(' · ')}
                </span>
              </span>
              <button
                type="button"
                aria-label={`${t('sourcePicker.preview')}: ${entry.track.title}`}
                onClick={() => void playResults([entry.track], entry.track)}
                className="text-muted hover:text-foreground grid size-8 place-items-center rounded-full"
              >
                <Play aria-hidden size={15} />
              </button>
              <button
                type="button"
                disabled={use.isPending}
                onClick={() => use.mutate(entry)}
                className="bg-background rounded-full px-3 py-1 text-xs font-semibold"
              >
                {t('sourcePicker.use')}
              </button>
            </li>
          ))}
        </ul>
        {use.isSuccess && use.variables && (
          <p role="status" className="text-primary text-sm">
            {use.variables.source
              ? t('sourcePicker.used', { source: use.variables.source })
              : t('sourcePicker.usedAny')}
          </p>
        )}
        {use.error && <SourceIssue>{use.error.message}</SourceIssue>}
      </div>
    </Sheet>
  );
};
