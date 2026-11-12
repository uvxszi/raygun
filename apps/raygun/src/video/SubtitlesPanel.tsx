import type { SubtitleStyle } from '@raygun/bindings';
import { ArrowLeft, ChevronRight, FilePlus2, Languages, Palette, Sparkles } from 'lucide-react';
import { useState } from 'react';
import type { ComponentProps, FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { cssColor } from '../settings/playerOptions';
import { AiSubtitles } from './AiSubtitles';
import { PanelHeading, PanelOption } from './PlayerPanel';
import { edgeShadow, subtitleFontCss } from './subtitleLook';
import { parseSubtitles } from './subtitles';
import type { Cue } from './subtitles';
import { TranslateSubtitles } from './TranslateSubtitles';
import { fileLanguage, nameTracks, useTrackWords } from './trackNames';
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
  /** A subtitle file read here: it shows at once; `source` is the file's name. */
  onAdd: (label: string, language: string | null, cues: Cue[], source: string | null) => void;
  delayMs: number;
  onDelay: (ms: number) => void;
  /** How the subtitles look now, for the Style card's sample. */
  style: Required<SubtitleStyle>;
  /** A font of the device's picked for subtitles, in front of the style's own. */
  family: string | null;
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
 * "Aa" as the subtitles look now (font, colours, edge, background), over a grey like a
 * picture's: the Style card's sample.
 */
const StyleSample: FC<{ style: Required<SubtitleStyle>; family: string | null }> = ({
  style,
  family,
}) => (
  <span
    aria-hidden
    data-testid="subtitle-style-sample"
    className="grid shrink-0 place-items-center rounded-lg bg-slate-600 px-2.5 py-1.5"
  >
    <span
      style={{
        ...subtitleFontCss(style.font, family),
        color: cssColor(style.textColor),
        backgroundColor: cssColor(style.backgroundColor),
        textShadow: edgeShadow(style.edge, cssColor(style.edgeColor), 1),
        fontWeight: style.bold ? 700 : 500,
        fontStyle: style.italic ? 'italic' : 'normal',
        fontSize: '1.05rem',
        lineHeight: 1.2,
        padding: '0 0.25em',
        borderRadius: '0.25em',
      }}
    >
      Aa
    </span>
  </span>
);

/**
 * The subtitles panel (M46.33, after v1.6.0), in two tabs. **Subtitles**: what adds subtitles
 * first, as buttons of its own (a file, transcribing with AI, translating; the last two open in
 * the panel), then the delay and the style (a card of its own with a sample, apart from the
 * tracks), then the subtitles to show with the viewer's languages first, each named apart from
 * the others with where it comes from under it. **Second subtitles**: a second track shown at
 * the top of the picture. AI subtitles and translations are listed once they have lines or are
 * being made.
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
  style,
  family,
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
  const words = useTrackWords();
  const [tool, setTool] = useState<'ai' | 'translate' | null>(null);
  const [tab, setTab] = useState<'main' | 'second'>('main');
  const listed = byLanguages(options, languages);
  // Under each name: how far AI subtitles or a translation got, else where it comes from (a
  // file without an addon or a name of its own is still a file).
  const detail = (option: SubtitleOption) => {
    if (option.hint) return option.hint;
    if (option.key.startsWith('embedded:')) return t('desktopPlayer.inTheVideo');
    if (option.source) return option.source;
    return option.key.startsWith('file:') || option.key.startsWith('added:')
      ? t('videoPlayer.subtitleFile')
      : null;
  };

  const tabs = (
    <div role="tablist" className="flex gap-2 px-2 pb-2">
      {(['main', 'second'] as const).map((name) => (
        <button
          key={name}
          type="button"
          role="tab"
          aria-selected={tab === name}
          onClick={() => setTab(name)}
          className={`rounded-full px-3 py-1.5 text-sm font-semibold ${
            tab === name ? 'bg-white text-black' : 'bg-white/10 hover:bg-white/20'
          }`}
        >
          {t(name === 'main' ? 'videoPlayer.subtitles' : 'videoPlayer.secondSubtitles')}
        </button>
      ))}
    </div>
  );

  if (tab === 'second') {
    // The video's own tracks can't show second: the browser reads one text track at a time.
    const seconds = listed.filter((option) => !option.key.startsWith('embedded:'));
    const names = nameTracks(seconds, words, detail);
    return (
      <>
        {tabs}
        <PanelOption
          label={t('videoPlayer.off')}
          selected={second === null}
          onSelect={() => onSecond(null)}
        />
        {seconds.map((option, index) => (
          <PanelOption
            key={option.key}
            label={names[index]?.name ?? option.label}
            hint={names[index]?.detail}
            selected={option.key === second}
            onSelect={() => onSecond(option.key)}
          />
        ))}
        {seconds.length === 0 && (
          <p className="px-3 py-2 text-sm text-white/60">{t('videoPlayer.secondNone')}</p>
        )}
      </>
    );
  }

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

  const names = nameTracks(listed, words, detail);
  return (
    <>
      {tabs}
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
              // "Film.en.srt" says its language: it's named by it, the file under it.
              void file
                .text()
                .then((text) =>
                  onAdd(
                    file.name,
                    fileLanguage(file.name),
                    parseSubtitles(text, extension),
                    file.name,
                  ),
                );
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
      {/* The style is a card of its own, with a sample, so it doesn't read as one more track. */}
      <button
        type="button"
        onClick={onStyle}
        className="mx-1 mt-2 mb-1 flex items-center gap-3 rounded-xl bg-white/10 px-3 py-2.5 text-left hover:bg-white/15 focus-visible:ring-1 focus-visible:ring-white/40 focus-visible:outline-none"
      >
        <Palette aria-hidden size={20} className="shrink-0" />
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="text-sm font-semibold">{t('videoPlayer.style')}</span>
          <span className="truncate text-xs text-white/60">
            {t('videoPlayer.styleSummary', {
              n: family ?? t(`settings.player.fonts.${style.font}`),
              of: style.size,
            })}
          </span>
        </span>
        <StyleSample style={style} family={family} />
        <ChevronRight aria-hidden size={16} className="shrink-0 text-white/55" />
      </button>
      <PanelHeading>{t('videoPlayer.subtitles')}</PanelHeading>
      <PanelOption
        label={t('videoPlayer.off')}
        selected={selected === null}
        onSelect={() => onSelect(null)}
      />
      {listed.map((option, index) => (
        <PanelOption
          key={option.key}
          label={names[index]?.name ?? option.label}
          hint={names[index]?.detail}
          selected={option.key === selected}
          onSelect={() => onSelect(option.key)}
        />
      ))}
      {find}
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
