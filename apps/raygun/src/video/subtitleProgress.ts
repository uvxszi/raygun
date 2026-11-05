import { commands, events } from '@raygun/bindings';
import type { Caption, SubtitleTranslation } from '@raygun/bindings';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import type { TFunction } from 'i18next';
import { useEffect } from 'react';

import { nativeName } from '../i18n/languages';
import { unwrapResult } from '../lib/commandResult';
import { formatTime } from '../player/formatTime';
import type { Cue } from './subtitles';

/**
 * AI subtitles and translated subtitles in the desktop player: how far ahead of the viewer they
 * are, and the video's translations as Rust keeps them (`video_subtitles`).
 */

/** How far past `positionMs` the stretches already heard reach (`[start, end)`); 0 outside. */
export const coveredAhead = (covered: [number, number][], positionMs: number): number => {
  let at = positionMs;
  for (;;) {
    const ends = covered.filter(([start, end]) => start <= at && at < end).map(([, end]) => end);
    const next = ends.length > 0 ? Math.max(...ends) : at;
    if (next <= at) return at - positionMs;
    at = next;
  }
};

/** A track's cues as Rust takes them: a cue's lines in one text. */
export const toCaptions = (cues: Cue[]): Caption[] =>
  cues.map((cue) => ({
    startMs: Math.max(0, Math.round(cue.startMs)),
    endMs: Math.max(0, Math.round(cue.endMs)),
    text: cue.lines.join('\n'),
  }));

/** Rust's cues as the player draws them. */
export const fromCaptions = (captions: Caption[]): Cue[] =>
  captions.map((caption) => ({
    startMs: caption.startMs,
    endMs: caption.endMs,
    lines: caption.text.split('\n'),
  }));

/** A translated track's name: "Deutsch (translated)". */
export const translationName = (translation: SubtitleTranslation, t: TFunction) =>
  t('desktopPlayer.translate.track', { language: nativeName(translation.language) });

/** Where translating a track stands, in a few words, for a viewer at `positionMs`. */
export const translationStatus = (
  translation: SubtitleTranslation,
  t: TFunction,
  positionMs: number,
): string => {
  if (translation.error) return translation.error.split('\n')[0] ?? translation.error;
  if (translation.downloading !== null) {
    return t('desktopPlayer.translate.downloading', { percent: translation.downloading });
  }
  if (translation.running) {
    return translation.readyUntilMs === null
      ? t('desktopPlayer.translate.running')
      : t('desktopPlayer.translate.ahead', {
          time: formatTime(Math.max(0, translation.readyUntilMs - positionMs)),
        });
  }
  if (translation.complete) return t('desktopPlayer.translate.complete');
  return t('desktopPlayer.translate.partly', { done: translation.done, total: translation.total });
};

export const translationsKey = (videoKey: string) => ['videoSubtitles', videoKey];

/** A video's translated subtitles, kept and being made, followed as lines are translated. */
export const useVideoTranslations = (videoKey: string) => {
  const client = useQueryClient();
  useEffect(() => {
    const unlisten = events.videoSubtitlesEvent.listen(({ payload }) => {
      if (payload === videoKey) {
        void client.invalidateQueries({ queryKey: translationsKey(videoKey) });
      }
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client, videoKey]);
  return useQuery({
    queryKey: translationsKey(videoKey),
    queryFn: () => unwrapResult(commands.videoSubtitlesTranslations(videoKey)),
    enabled: videoKey !== '',
  });
};
