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
import { TextField } from '../shell/TextField';
import { SPIN } from './songDownloadState';
import { sourceAnswers } from './sourceAnswers';
import { useSongDownloadActions, useSongDownloadsSelect } from './useSongDownloads';

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
  const downloadFrom = (chosen: { track: OnlineTrack; source: string | undefined }) => {
    if (!download) return;
    useSource.mutate(
      { id: download, track: chosen.track, source: chosen.source ?? null },
      { onSuccess: onClose },
    );
  };
  // A preview joins the queue and plays; what was playing stays in the queue.
  const preview = (track: OnlineTrack) =>
    download
      ? void commands.playbackEnqueue([toQueueItem(track)], true)
      : void playResults([track], track);
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
    <Sheet
      title={download ? t('sourcePicker.downloadTitle') : t('sourcePicker.title')}
      onClose={onClose}
    >
      <div data-testid="source-picker" className="flex flex-col gap-3">
        {download && why && <p className="text-muted text-sm wrap-anywhere">{why}</p>}
        <form
          className="flex flex-wrap items-center gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            setQuery([names.artist.trim(), names.title.trim()].filter(Boolean).join(' '));
          }}
        >
          <TextField
            aria-label={t('details.songTitle')}
            placeholder={t('details.songTitle')}
            value={names.title}
            onChange={(title) => setNames({ ...names, title })}
            className="bg-background flex-1 basis-40 rounded-xl pr-0.5 pl-3"
            inputClassName="py-2 text-sm"
          />
          <TextField
            aria-label={t('details.artist')}
            placeholder={t('details.artist')}
            value={names.artist}
            onChange={(artist) => setNames({ ...names, artist })}
            className="bg-background flex-1 basis-32 rounded-xl pr-0.5 pl-3"
            inputClassName="py-2 text-sm"
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
            <LoaderCircle aria-hidden size={14} className={SPIN} />
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
                onClick={() => preview(entry.track)}
                className="text-muted hover:text-foreground grid size-8 place-items-center rounded-full"
              >
                <Play aria-hidden size={15} />
              </button>
              {download ? (
                <button
                  type="button"
                  disabled={useSource.isPending}
                  onClick={() => downloadFrom(entry)}
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
                  onClick={() => use.mutate(entry)}
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
