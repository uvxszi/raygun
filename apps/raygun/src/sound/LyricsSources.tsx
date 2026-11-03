import { commands } from '@raygun/bindings';
import type { Lyrics, TrackQuery } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AudioLines, Library, LoaderCircle } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { trackKey } from '../music/trackKey';
import { useCaptions } from '../player/spoken';
import { SourceIssue } from '../shell/SourceIssue';
import { lyricsSourceName } from './lyricsSource';
import type { FullSongEdit } from './soundPrefs';

/**
 * Where a song's lyrics come from (M46.36): every lyrics source's version to pick from, or the
 * words heard by the transcription set up in Settings → AI. A pick becomes the song's own lyrics,
 * to fix like any others.
 */
export const LyricsSources: FC<{
  song: TrackQuery;
  edit: FullSongEdit;
  save: (edit: FullSongEdit) => void;
}> = ({ song, edit, save }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const options = useQuery({
    queryKey: ['lyricsOptions', song.title, song.artist, song.album, song.durationMs],
    queryFn: () => unwrapResult(commands.lyricsOptions(song)),
    enabled: open,
    staleTime: 10 * 60_000,
  });
  const use = (lyrics: Lyrics) =>
    save({ ...edit, lyrics: { synced: lyrics.synced, plain: lyrics.plain } });
  return (
    <div data-testid="lyrics-sources" className="flex flex-col gap-2">
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
          className="bg-background hover:bg-surface flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm"
        >
          <Library aria-hidden size={15} />
          {t('lyricsEdit.sources')}
        </button>
        <HearWords song={song} />
      </div>
      {open && options.isPending && (
        <p className="text-muted flex items-center gap-2 text-sm">
          <LoaderCircle aria-hidden size={15} className="animate-spin" />
          {t('lyricsEdit.sourcesLoading')}
        </p>
      )}
      {open && options.error && <SourceIssue>{options.error.message}</SourceIssue>}
      {open && options.data?.length === 0 && (
        <p className="text-muted text-sm">{t('lyricsEdit.sourcesNone')}</p>
      )}
      {open && (options.data?.length ?? 0) > 0 && (
        <ul className="bg-background flex flex-col rounded-xl">
          {options.data?.map((lyrics, index) => {
            const first =
              lyrics.synced.find((line) => line.text.trim() !== '')?.text ??
              lyrics.plain?.split('\n').find((line) => line.trim() !== '') ??
              '';
            return (
              <li key={`${lyrics.providerName}-${index}`}>
                <button
                  type="button"
                  onClick={() => use(lyrics)}
                  className="hover:bg-surface flex w-full flex-col gap-0.5 rounded-xl px-3 py-2 text-left"
                >
                  <span className="text-sm font-medium">
                    {lyricsSourceName(lyrics.providerName, t)} ·{' '}
                    {t(lyrics.synced.length > 0 ? 'lyricsEdit.synced' : 'lyricsEdit.plain')}
                  </span>
                  <span className="text-muted truncate text-xs">{first}</span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
};

/**
 * Hears the song's words with AI (M46.36): its downloaded copy, or a stream a source has. When
 * it's done the words are the song's synced lyrics.
 */
const HearWords: FC<{ song: TrackQuery }> = ({ song }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const key = `song:${trackKey(song.title, song.artist)}`;
  const captions = useCaptions(key);
  const start = useMutation({
    mutationFn: () => unwrapResult(commands.lyricsTranscribe(song)),
    onSettled: () => client.invalidateQueries({ queryKey: ['captions', key] }),
  });
  const running = captions.data?.running ?? false;
  // When the job ends its words are the song's lyrics: show them.
  useEffect(() => {
    if (!running && captions.data?.cues.length) {
      void client.invalidateQueries({ queryKey: ['lyrics'] });
      void client.invalidateQueries({ queryKey: ['songEdit'] });
    }
  }, [running, captions.data?.cues.length, client]);
  if (captions.data && !captions.data.canTranscribe) {
    return <span className="text-muted self-center text-xs">{t('lyricsEdit.hearSetUp')}</span>;
  }
  return (
    <>
      <button
        type="button"
        disabled={running || start.isPending}
        onClick={() => start.mutate()}
        className="bg-background hover:bg-surface flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm disabled:opacity-60"
      >
        {running ? (
          <LoaderCircle aria-hidden size={15} className="animate-spin" />
        ) : (
          <AudioLines aria-hidden size={15} />
        )}
        {running
          ? t('lyricsEdit.hearing', { n: captions.data?.cues.length ?? 0 })
          : t('lyricsEdit.hear')}
      </button>
      {(start.error ?? captions.data?.error) && (
        <SourceIssue>{start.error?.message ?? captions.data?.error}</SourceIssue>
      )}
    </>
  );
};
