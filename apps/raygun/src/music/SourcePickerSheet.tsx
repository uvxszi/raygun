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
import { playResults, toQueueItem, useOnlineSearch } from '../search/useOnlineSearch';
import { Sheet } from '../shell/Sheet';
import { useYtDlpStatus } from '../youtube/useYtDlp';
import { useSourcePickerStore } from './sourcePickerStore';
import { SourceIssue } from '../shell/SourceIssue';
import { SPIN } from './songDownloadState';
import { sourceAnswers } from './sourceAnswers';
import { differences, sourceChoice } from './sourcePick';
import type { DetailField } from './sourcePick';
import { SourcePickConfirm } from './SourcePickConfirm';
import { useSongDownloadActions, useSongDownloadsSelect } from './useSongDownloads';

/** A source's answer, and the details to take from it (M46.48). */
type Picked = { track: OnlineTrack; source: string | undefined };
type Confirmed = Picked & { ticked: ReadonlySet<DetailField> };

/**
 * Find a source (M46.31): every source's answer for the song, with a preview, and Use this,
 * which every saved copy of the song plays from afterwards. The title and artist can be fixed
 * and searched again (M46.45: screenshots get names wrong); using a source then keeps the fixed
 * names as the song's details.
 */
export const SourcePickerSheet: FC = () => {
  const song = useSourcePickerStore((store) => store.song);
  const download = useSourcePickerStore((store) => store.download);
  const close = useSourcePickerStore((store) => store.close);
  if (!song) return null;
  return (
    <SourcePickerBody
      key={`${song.artist ?? ''}|${song.title}|${download ?? ''}`}
      song={song}
      download={download}
      onClose={close}
    />
  );
};

/**
 * With `download` (M46.47), a failed or YouTube-only download's sources: why it didn't download,
 * then only answers with a stream of their own that this device can save (YouTube's only where
 * yt-dlp runs), each with a preview and Download from this. The download shows as queued from the
 * picked source at once.
 */
const SourcePickerBody: FC<{ song: TrackInput; download: string | null; onClose: () => void }> = ({
  song,
  download,
  onClose,
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [names, setNames] = useState({ title: song.title, artist: song.artist ?? '' });
  const [query, setQuery] = useState([song.artist, song.title].filter(Boolean).join(' '));
  const search = useOnlineSearch(query);
  // Phones can't save YouTube's copy: its answers would fail again.
  const savesYouTube = useYtDlpStatus(download !== null).data?.supported === true;
  const why = useSongDownloadsSelect(
    (list) => list.find((item) => item.id === download)?.error ?? null,
  ).data;
  const { useSource } = useSongDownloadActions();
  const renamed = names.title.trim() !== song.title || names.artist.trim() !== (song.artist ?? '');
  const saveable = (track: OnlineTrack) =>
    track.source !== null && (track.source.kind !== 'youTube' || savesYouTube);
  // Each source's own answers: the same song from Audius and from YouTube are two choices.
  const ranked = sourceAnswers(query, search.data?.providers ?? [], (track) =>
    download ? saveable(track) : true,
  ).slice(0, download ? 24 : 16);
  const [picked, setPicked] = useState<Picked | null>(null);
  // Downloads from the source (M46.47); the details ticked become the song's too (M46.48).
  const downloadFrom = (chosen: Confirmed) => {
    if (!download) return;
    useSource.mutate(
      { id: download, track: chosen.track, source: chosen.source ?? null },
      {
        onSuccess: () => {
          if (chosen.ticked.size > 0) {
            void unwrapResult(
              commands.trackUseSource(
                song,
                chosen.track,
                sourceChoice(false, chosen.source, chosen.ticked),
              ),
            ).then(() => client.invalidateQueries({ queryKey: [COLLECTIONS] }));
          }
          onClose();
        },
      },
    );
  };
  // A preview joins the queue and plays; what was playing stays in the queue.
  const preview = (track: OnlineTrack) =>
    download
      ? void commands.playbackEnqueue([toQueueItem(track)], true)
      : void playResults([track], track);
  const use = useMutation({
    mutationFn: async (chosen: Confirmed) => {
      // Every saved copy, the queue and the resolver take the stream (M46.48).
      await unwrapResult(
        commands.trackUseSource(
          song,
          chosen.track,
          sourceChoice(true, chosen.source, chosen.ticked),
        ),
      );
      const title = !chosen.ticked.has('title') && names.title.trim();
      const artist = !chosen.ticked.has('artist');
      if (!renamed || !title) return;
      // The fixed names show everywhere the song does (Edit details, M46.30).
      const target = {
        kind: 'song' as const,
        query: { title: song.title, artist: song.artist, album: song.album, durationMs: null },
      };
      const edit = await unwrapResult(commands.detailEdit(target));
      await unwrapResult(
        commands.detailEditSet(target, {
          ...edit,
          title,
          ...(artist ? { artist: names.artist.trim() || null } : {}),
        }),
      );
    },
    onSuccess: () => {
      setPicked(null);
      void client.invalidateQueries({ queryKey: [COLLECTIONS] });
      void client.invalidateQueries({ queryKey: ['unavailable'] });
      void client.invalidateQueries({ queryKey: ['trackImportStatus'] });
    },
  });
  // A pick whose details differ asks which to take first; others go at once.
  const choose = (entry: Picked) => {
    if (differences(song, entry.track).length > 0) setPicked(entry);
    else if (download) downloadFrom({ ...entry, ticked: new Set() });
    else use.mutate({ ...entry, ticked: new Set() });
  };

  return (
    <Sheet
      title={download ? t('sourcePicker.downloadTitle') : t('sourcePicker.title')}
      onClose={onClose}
    >
      <div data-testid="source-picker" className="flex flex-col gap-3">
        {download && why && <p className="text-muted text-sm wrap-anywhere">{why}</p>}
        {picked && (
          <SourcePickConfirm
            song={song}
            track={picked.track}
            sourceName={picked.source}
            confirmLabel={download ? t('sourcePicker.downloadFrom') : t('sourcePicker.useSource')}
            busy={use.isPending || useSource.isPending}
            onConfirm={(ticked) =>
              download ? downloadFrom({ ...picked, ticked }) : use.mutate({ ...picked, ticked })
            }
            onBack={() => setPicked(null)}
          />
        )}
        {/* Kept while a pick is confirmed (hidden), so Back finds the list as it was. */}
        <form
          className={picked ? 'hidden' : 'flex flex-wrap items-center gap-2'}
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
        {search.isPending && !picked && (
          <p className="text-muted flex items-center gap-2 text-sm">
            <LoaderCircle aria-hidden size={14} className={SPIN} />
            {t('sourcePicker.searching')}
          </p>
        )}
        {search.isSuccess && ranked.length === 0 && !picked && (
          <p className="text-muted text-sm">{t('sourcePicker.none')}</p>
        )}
        <ul className={picked ? 'hidden' : 'flex max-h-[50vh] flex-col overflow-y-auto'}>
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
                onClick={() => preview(entry.track)}
                className="text-muted hover:text-foreground grid size-8 place-items-center rounded-full"
              >
                <Play aria-hidden size={15} />
              </button>
              {download ? (
                <button
                  type="button"
                  disabled={useSource.isPending}
                  onClick={() => choose(entry)}
                  className="bg-primary text-background flex items-center gap-1 rounded-full px-3 py-1 text-xs font-semibold disabled:opacity-60"
                >
                  {useSource.isPending && useSource.variables?.track === entry.track && (
                    <LoaderCircle aria-hidden size={12} className={SPIN} />
                  )}
                  {t('sourcePicker.downloadFrom')}
                </button>
              ) : (
                <button
                  type="button"
                  disabled={use.isPending}
                  onClick={() => choose(entry)}
                  className="bg-background rounded-full px-3 py-1 text-xs font-semibold"
                >
                  {t('sourcePicker.use')}
                </button>
              )}
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
        {useSource.error && (
          <p role="alert" className="text-primary text-sm wrap-anywhere">
            {useSource.error.message}
          </p>
        )}
      </div>
    </Sheet>
  );
};
