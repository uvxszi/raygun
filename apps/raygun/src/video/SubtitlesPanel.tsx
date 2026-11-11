import { ArrowLeft, ChevronRight, FilePlus2, Languages, Sparkles } from 'lucide-react';
import { useState } from 'react';
import type { ComponentProps, FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { AiSubtitles } from './AiSubtitles';
import { PanelHeading, PanelOption } from './PlayerPanel';
import { parseSubtitles } from './subtitles';
import type { Cue } from './subtitles';
import { TranslateSubtitles } from './TranslateSubtitles';
import { byLanguages } from './trackPicking';
import type { TrackOption } from './trackPicking';

/** A track the subtitles panel offers: the video's own, a file, AI subtitles, a translation. */
export type SubtitleOption = TrackOption & { cues?: Cue[]; hint?: string };

/** The largest delay either way (ten minutes), as the Android player keeps it. */
const MAX_DELAY_S = 600;

type Props = {
  videoKey: string;
  /** What plays (the proxy's link): AI subtitles read its sound. */
  url: string;
  format: string | null;
  positionMs: number;
  /** Every track to choose from. */
  options: SubtitleOption[];
  /** The languages listed first (`listedLanguages`). */
  languages: string[];
  selected: string | null;
  onSelect: (key: string | null) => void;
  /** A subtitle file read here: it shows at once. */
  onAdd: (label: string, language: string | null, cues: Cue[]) => void;
  delayMs: number;
  onDelay: (ms: number) => void;
  /** Settings → Player → Subtitles (the style) over the video. */
  onStyle: () => void;
  second: string | null;
  onSecond: (key: string | null) => void;
  /** Starting AI subtitles shows them. */
  onAiStarted: () => void;
  /** Settings → AI opens in the app: the player closes. */
  onClose: () => void;
  translate: Omit<ComponentProps<typeof TranslateSubtitles>, 'videoKey' | 'positionMs'>;
  /** Find subtitles in the addons. */
  find: ReactNode;
  /** Changing the chosen track's lines, when it can be. */
  editor: ReactNode;
};

const TOOL =
  'flex min-h-20 cursor-pointer flex-col items-center justify-center gap-1.5 rounded-xl bg-white/10 px-2 py-2.5 text-center text-xs leading-tight font-medium hover:bg-white/15 focus-within:ring-1 focus-within:ring-white/40';

/**
 * The subtitles panel (M46.33, after v1.6.0): what adds subtitles first, as buttons of its own
 * (a file, transcribing with AI, translating; the last two open in the panel), then the delay
 * and the style, then the subtitles to show with the viewer's languages first. AI subtitles and
 * translations are listed once they have lines or are being made.
 */
export const SubtitlesPanel: FC<Props> = ({
  videoKey,
  url,
  format,
  positionMs,
  options,
  languages,
  selected,
  onSelect,
  onAdd,
  delayMs,
  onDelay,
  onStyle,
  second,
  onSecond,
  onAiStarted,
  onClose,
  translate,
  find,
  editor,
}) => {
  const { t } = useTranslation();
  const [tool, setTool] = useState<'ai' | 'translate' | null>(null);
  const listed = byLanguages(options, languages);

  if (tool) {
    return (
      <>
        <button
          type="button"
          onClick={() => setTool(null)}
          className="flex items-center gap-2 self-start rounded-lg px-3 py-2 text-sm text-white/80 hover:bg-white/10"
        >
          <ArrowLeft aria-hidden size={16} />
          {t('desktopPlayer.allSubtitles')}
        </button>
        {tool === 'ai' ? (
          <AiSubtitles
            videoKey={videoKey}
            url={url}
            format={format}
            positionMs={positionMs}
            onStarted={onAiStarted}
            onClose={onClose}
          />
        ) : (
          <TranslateSubtitles videoKey={videoKey} positionMs={positionMs} {...translate} />
        )}
      </>
    );
  }

  return (
    <>
      <div className="grid grid-cols-3 gap-2 px-1 pt-1 pb-2">
        <label className={TOOL}>
          <FilePlus2 aria-hidden size={20} />
          {t('videoPlayer.addSubtitleFile')}
          <input
            type="file"
            accept=".srt,.vtt,.ass,.ssa,.sub,.txt"
            className="sr-only"
            onChange={(event) => {
              const file = event.target.files?.[0];
              if (!file) return;
              const extension = file.name.split('.').pop() ?? null;
              void file
                .text()
                .then((text) => onAdd(file.name, null, parseSubtitles(text, extension)));
            }}
          />
        </label>
        <button type="button" onClick={() => setTool('ai')} className={TOOL}>
          <Sparkles aria-hidden size={20} />
          {t('videoPlayer.aiTranscribe')}
        </button>
        <button type="button" onClick={() => setTool('translate')} className={TOOL}>
          <Languages aria-hidden size={20} />
          {t('videoPlayer.translate')}
        </button>
      </div>
      <PanelHeading>{t('videoPlayer.delay')}</PanelHeading>
      <div className="flex items-center gap-2 px-3 py-1">
        <button
          type="button"
          onClick={() => onDelay(delayMs - 100)}
          className="rounded-lg bg-white/10 px-3 py-1.5 text-sm hover:bg-white/20"
        >
          −0.1 s
        </button>
        <DelayField delayMs={delayMs} onChange={onDelay} />
        <button
          type="button"
          onClick={() => onDelay(delayMs + 100)}
          className="rounded-lg bg-white/10 px-3 py-1.5 text-sm hover:bg-white/20"
        >
          +0.1 s
        </button>
        <button
          type="button"
          disabled={delayMs === 0}
          onClick={() => onDelay(0)}
          className="rounded-lg px-2 py-1.5 text-sm hover:bg-white/10 disabled:opacity-40"
        >
          {t('videoPlayer.reset')}
        </button>
      </div>
      <button
        type="button"
        onClick={onStyle}
        className="mt-1 flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm hover:bg-white/10"
      >
        <span className="flex-1">{t('videoPlayer.style')}</span>
        <ChevronRight aria-hidden size={16} className="text-white/55" />
      </button>
      <PanelHeading>{t('videoPlayer.subtitles')}</PanelHeading>
      <PanelOption
        label={t('videoPlayer.off')}
        selected={selected === null}
        onSelect={() => onSelect(null)}
      />
      {listed.map((option) => (
        <PanelOption
          key={option.key}
          label={option.label}
          hint={
            option.hint ??
            (option.key.startsWith('embedded:') ? t('desktopPlayer.inTheVideo') : option.language)
          }
          selected={option.key === selected}
          onSelect={() => onSelect(option.key)}
        />
      ))}
      {find}
      <label className="flex items-center gap-3 px-3 pt-3 pb-1.5 text-sm">
        <span className="flex-1">{t('videoPlayer.secondSubtitles')}</span>
        <select
          value={second ?? ''}
          onChange={(event) => onSecond(event.target.value || null)}
          className="max-w-40 rounded-lg bg-white/10 px-2 py-1 outline-none"
        >
          <option value="" className="bg-neutral-900">
            {t('videoPlayer.off')}
          </option>
          {listed
            .filter((option) => !option.key.startsWith('embedded:'))
            .map((option) => (
              <option key={option.key} value={option.key} className="bg-neutral-900">
                {option.label}
              </option>
            ))}
        </select>
      </label>
      {editor}
    </>
  );
};

/**
 * The subtitle delay as a typed number of seconds (M46.33): decimals, negative for earlier, up to
 * ten minutes either way, as on Android.
 */
const DelayField: FC<{ delayMs: number; onChange: (ms: number) => void }> = ({
  delayMs,
  onChange,
}) => {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<string | null>(null);
  const commit = () => {
    if (draft === null) return;
    const seconds = Number(
      draft.trim().replace(/s$/i, '').trim().replace(',', '.').replace('−', '-'),
    );
    if (draft.trim() !== '' && Number.isFinite(seconds)) {
      onChange(Math.round(Math.max(-MAX_DELAY_S, Math.min(MAX_DELAY_S, seconds)) * 1000));
    }
    setDraft(null);
  };
  return (
    <label className="flex flex-1 items-center justify-center gap-1 text-sm tabular-nums">
      <input
        type="text"
        inputMode="decimal"
        aria-label={t('desktopPlayer.delaySeconds')}
        value={draft ?? (delayMs / 1000).toFixed(2)}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          // Enter leaves the field, which keeps what's typed (once).
          if (event.key === 'Enter') (event.target as HTMLInputElement).blur();
          event.stopPropagation();
        }}
        className="w-16 rounded-md bg-white/10 px-2 py-1 text-center outline-none focus:ring-1 focus:ring-white/40"
      />
      s
    </label>
  );
};
