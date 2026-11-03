import { commands } from '@raygun/bindings';
import type { OnlineTrack, TrackInput } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { LoaderCircle, Play } from 'lucide-react';
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
 * which every saved copy of the song plays from afterwards.
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
  const query = [song.artist, song.title].filter(Boolean).join(' ');
  const search = useOnlineSearch(query);
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
    mutationFn: (chosen: { track: OnlineTrack; source: string | undefined }) =>
      unwrapResult(commands.trackAssignSource(song, chosen.track)),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: [COLLECTIONS] });
      void client.invalidateQueries({ queryKey: ['unavailable'] });
    },
  });

  return (
    <Sheet title={t('sourcePicker.title')} onClose={onClose}>
      <div data-testid="source-picker" className="flex flex-col gap-3">
        <p className="truncate text-sm font-semibold">
          {[song.artist, song.title].filter(Boolean).join(' – ')}
        </p>
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
