import { RotateCcw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PanelHeading } from './PlayerPanel';

/** The largest delay either way (ten minutes), as the Android player keeps it. */
const MAX_DELAY_S = 600;

/**
 * A delay as a typed number of seconds (M46.33): decimals, negative for earlier, up to ten
 * minutes either way, as on Android. `label` names the field.
 */
const DelayField: FC<{ delayMs: number; label: string; onChange: (ms: number) => void }> = ({
  delayMs,
  label,
  onChange,
}) => {
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
    <label className="flex min-w-0 flex-[1.6] cursor-text items-center justify-center gap-0.5 rounded-full bg-white/5 px-2 py-1 text-sm tabular-nums ring-1 ring-white/25 focus-within:ring-white/60">
      <input
        type="text"
        inputMode="decimal"
        aria-label={label}
        value={draft ?? (delayMs / 1000).toFixed(2)}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          // Enter leaves the field, which keeps what's typed (once).
          if (event.key === 'Enter') (event.target as HTMLInputElement).blur();
          event.stopPropagation();
        }}
        className="w-full min-w-0 bg-transparent py-0.5 text-center outline-none"
      />
      <span aria-hidden className="text-white/60">
        s
      </span>
    </label>
  );
};

/** The step buttons' changes, in milliseconds, and their words: a second and a tenth either way. */
const STEPS: [number, string][] = [
  [-1000, '−1'],
  [-100, '−0.1'],
  [100, '+0.1'],
  [1000, '+1'],
];

const STEP =
  'min-w-0 flex-1 rounded-full bg-white/10 px-1 py-1.5 text-sm font-medium tabular-nums hover:bg-white/20';

/**
 * A delay's heading and one row filling the width (the subtitles', the sound's): a second and a
 * tenth earlier, the seconds typed between them, a tenth and a second later, then Reset.
 */
export const DelayControls: FC<{
  delayMs: number;
  label: string;
  onChange: (ms: number) => void;
}> = ({ delayMs, label, onChange }) => {
  const { t } = useTranslation();
  const step = (ms: number) =>
    onChange(Math.max(-MAX_DELAY_S * 1000, Math.min(MAX_DELAY_S * 1000, delayMs + ms)));
  const button = ([ms, words]: [number, string]) => (
    <button key={ms} type="button" onClick={() => step(ms)} className={STEP}>
      {words}
    </button>
  );
  return (
    <>
      <PanelHeading>{t('videoPlayer.delay')}</PanelHeading>
      <div className="flex items-center gap-1.5 px-2 py-1">
        {STEPS.slice(0, 2).map(button)}
        <DelayField delayMs={delayMs} label={label} onChange={onChange} />
        {STEPS.slice(2).map(button)}
        <button
          type="button"
          aria-label={t('videoPlayer.reset')}
          disabled={delayMs === 0}
          onClick={() => onChange(0)}
          className="grid size-8 shrink-0 place-items-center rounded-full bg-white/10 hover:bg-white/20 disabled:opacity-40"
        >
          <RotateCcw aria-hidden size={15} />
        </button>
      </div>
    </>
  );
};
