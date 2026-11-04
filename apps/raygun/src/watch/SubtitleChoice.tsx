import type { VideoSubtitle } from '@raygun/bindings';
import { Check, Download, LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSubtitleDefaults } from './useVideoLibrary';

/** A track's language in the app's language ("eng" reads "English"), else its code. */
const languageName = (lang: string | null, locale: string) => {
  if (!lang) return null;
  try {
    return new Intl.DisplayNames([locale], { type: 'language' }).of(lang) ?? lang;
  } catch {
    return lang;
  }
};

/** What a track row says: its label, its language, or both when they differ. */
const trackName = (track: VideoSubtitle, locale: string, fallback: string) => {
  const language = languageName(track.lang, locale);
  if (track.label && language && track.label !== language && track.label !== track.lang) {
    return `${track.label} · ${language}`;
  }
  return track.label ?? language ?? fallback;
};

/**
 * The subtitle tracks a download saves, ticked as picked at this source before (and the device's
 * language unless it was left out); what's ticked is remembered for the source.
 */
export const SubtitleChoice: FC<{
  /** A video source's id, or `stremio`. */
  source: string;
  tracks: VideoSubtitle[];
  busy: boolean;
  onDownload: (urls: string[]) => void;
  onBack: () => void;
}> = ({ source, tracks, busy, onDownload, onBack }) => {
  const { t, i18n } = useTranslation();
  const defaults = useSubtitleDefaults(source, tracks);
  const [picked, setPicked] = useState<Set<string> | null>(null);
  // Until a box is touched, the remembered choice shows.
  const ticked = picked ?? new Set(defaults.data ?? []);
  const toggle = (url: string) => {
    const next = new Set(ticked);
    if (next.has(url)) next.delete(url);
    else next.add(url);
    setPicked(next);
  };
  const unnamed = t('watch.subtitleUnnamed');

  return (
    <section aria-labelledby="subtitle-choice" className="flex flex-col gap-3">
      <h3 id="subtitle-choice" className="text-muted text-xs font-semibold tracking-wide uppercase">
        {t('watch.subtitlesToSave')}
      </h3>
      {defaults.isPending ? (
        <LoaderCircle
          role="status"
          aria-label={t('watch.loading')}
          className="text-muted mx-auto animate-spin"
        />
      ) : (
        <ul className="-mx-2 flex flex-col">
          {tracks.map((track) => {
            const checked = ticked.has(track.url);
            return (
              <li key={track.url}>
                <button
                  type="button"
                  role="checkbox"
                  aria-checked={checked}
                  onClick={() => toggle(track.url)}
                  className="hover:bg-background flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left text-sm"
                >
                  <span
                    aria-hidden
                    className={`grid size-5 shrink-0 place-items-center rounded border-2 ${checked ? 'border-primary bg-primary text-background' : 'border-muted'}`}
                  >
                    {checked && <Check size={14} strokeWidth={3} />}
                  </span>
                  <span className="min-w-0 wrap-anywhere">
                    {trackName(track, i18n.language, unnamed)}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
      <div className="flex justify-end gap-2">
        <button type="button" onClick={onBack} className="rounded-full px-4 py-2.5 font-medium">
          {t('nav.back')}
        </button>
        <button
          type="button"
          disabled={busy || defaults.isPending}
          onClick={() =>
            onDownload(tracks.filter((track) => ticked.has(track.url)).map((track) => track.url))
          }
          className="bg-primary text-background flex items-center gap-2 rounded-full px-5 py-2.5 font-semibold disabled:opacity-50"
        >
          <Download aria-hidden size={16} />
          {t('watch.downloadWithSubtitles', { count: ticked.size })}
        </button>
      </div>
    </section>
  );
};
