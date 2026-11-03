import { commands } from '@raygun/bindings';
import type { Lyrics, TrackQuery } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AudioLines, Check, ChevronDown, LoaderCircle, RotateCcw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { trackKey } from '../music/trackKey';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { lyricsSourceName } from './lyricsSource';
import { useSongEdit } from './useSound';

const firstLine = (lyrics: Lyrics) =>
  lyrics.synced.find((line) => line.text.trim() !== '')?.text ??
  lyrics.plain?.split('\n').find((line) => line.trim() !== '') ??
  '';

const sameWords = (a: Lyrics, b: { synced?: Lyrics['synced']; plain?: string | null }) => {
  const lines = b.synced ?? [];
  return (
    a.synced.length === lines.length &&
    a.synced.every((line, index) => line.text === lines[index]?.text) &&
    (a.synced.length > 0 || a.plain === (b.plain ?? null))
  );
};

/**
 * Where the lyrics come from, as one dropdown under them (M46.45): every source's version, the
 * words a transcription heard before (kept, so switching back is instant), the sources' own
 * pick, and hearing the words again.
 */
export const LyricsSourceMenu: FC<{ query: TrackQuery; current: Lyrics | undefined }> = ({
  query,
  current,
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const options = useQuery({
    queryKey: ['lyricsOptions', query.title, query.artist, query.album, query.durationMs],
    queryFn: () => unwrapResult(commands.lyricsOptions(query)),
    enabled: anchor !== null,
    staleTime: 10 * 60_000,
  });
  const { edit, save } = useSongEdit(query);
  const hear = useMutation({
    mutationFn: () => unwrapResult(commands.lyricsTranscribe(query)),
    onSettled: () =>
      client.invalidateQueries({
        queryKey: ['captions', `song:${trackKey(query.title, query.artist)}`],
      }),
  });
  const name = (provider: string) => lyricsSourceName(provider, t);
  const chosen = edit.data?.lyrics ?? null;
  const pick = (lyrics: Lyrics | null) => {
    if (edit.data) {
      save.mutate({
        ...edit.data,
        lyrics: lyrics ? { synced: lyrics.synced, plain: lyrics.plain } : null,
      });
    }
    setAnchor(null);
  };
  const row =
    'hover:bg-background flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm';

  return (
    <>
      <button
        type="button"
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        onClick={(event) => setAnchor(anchor ? null : event.currentTarget)}
        className="text-muted hover:text-foreground flex items-center gap-1 text-xs"
      >
        {current
          ? t('player.lyricsFrom', { source: name(current.providerName) })
          : t('lyricsEdit.sources')}
        <ChevronDown aria-hidden size={12} />
      </button>
      {anchor && (
        <Sheet
          title={t('lyricsEdit.sources')}
          anchor={anchor}
          popoverWidth={300}
          onClose={() => setAnchor(null)}
        >
          <div
            role="radiogroup"
            aria-label={t('lyricsEdit.sources')}
            data-testid="lyrics-source-menu"
            className="-mx-2 flex flex-col"
          >
            <button
              type="button"
              role="radio"
              aria-checked={chosen === null}
              onClick={() => pick(null)}
              className={row}
            >
              <RotateCcw aria-hidden size={15} className="text-muted shrink-0" />
              <span className="flex-1">{t('lyricsEdit.automatic')}</span>
              {chosen === null && <Check aria-hidden size={14} className="text-primary" />}
            </button>
            {options.isPending && (
              <p className="text-muted flex items-center gap-2 px-3 py-2 text-sm">
                <LoaderCircle aria-hidden size={14} className="animate-spin" />
                {t('lyricsEdit.sourcesLoading')}
              </p>
            )}
            {options.error && <SourceIssue>{options.error.message}</SourceIssue>}
            {options.data?.map((lyrics, index) => {
              const on = chosen !== null && sameWords(lyrics, chosen);
              return (
                <button
                  key={`${lyrics.providerName}-${index}`}
                  type="button"
                  role="radio"
                  aria-checked={on}
                  onClick={() => pick(lyrics)}
                  className={row}
                >
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate font-medium">
                      {name(lyrics.providerName)} ·{' '}
                      {t(lyrics.synced.length > 0 ? 'lyricsEdit.synced' : 'lyricsEdit.plain')}
                    </span>
                    <span className="text-muted truncate text-xs">{firstLine(lyrics)}</span>
                  </span>
                  {on && <Check aria-hidden size={14} className="text-primary shrink-0" />}
                </button>
              );
            })}
            <button
              type="button"
              disabled={hear.isPending}
              onClick={() => {
                hear.mutate();
                setAnchor(null);
              }}
              className={`${row} disabled:opacity-60`}
            >
              <AudioLines aria-hidden size={15} className="text-muted shrink-0" />
              <span className="flex-1">{t('lyricsEdit.hear')}</span>
            </button>
          </div>
        </Sheet>
      )}
      {hear.error && <SourceIssue>{hear.error.message}</SourceIssue>}
    </>
  );
};
