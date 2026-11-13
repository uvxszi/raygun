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
    <label className="flex flex-1 items-center justify-center gap-1 text-sm tabular-nums">
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
        className="w-16 rounded-md bg-white/10 px-2 py-1 text-center outline-none focus:ring-1 focus:ring-white/40"
      />
      s
    </label>
  );
};

/**
 * A delay's heading and row (the subtitles', the sound's): a tenth of a second earlier or later,
 * the seconds typed, and Reset.
 */
export const DelayControls: FC<{
  delayMs: number;
  label: string;
  onChange: (ms: number) => void;
}> = ({ delayMs, label, onChange }) => {
  const { t } = useTranslation();
  const step = (ms: number) =>
    onChange(Math.max(-MAX_DELAY_S * 1000, Math.min(MAX_DELAY_S * 1000, delayMs + ms)));
  return (
    <>
      <PanelHeading>{t('videoPlayer.delay')}</PanelHeading>
      <div className="flex items-center gap-2 px-3 py-1">
        <button
          type="button"
          onClick={() => step(-100)}
          className="rounded-lg bg-white/10 px-3 py-1.5 text-sm hover:bg-white/20"
        >
          −0.1 s
        </button>
        <DelayField delayMs={delayMs} label={label} onChange={onChange} />
        <button
          type="button"
          onClick={() => step(100)}
          className="rounded-lg bg-white/10 px-3 py-1.5 text-sm hover:bg-white/20"
        >
          +0.1 s
        </button>
        <button
          type="button"
          disabled={delayMs === 0}
          onClick={() => onChange(0)}
          className="rounded-lg px-2 py-1.5 text-sm hover:bg-white/10 disabled:opacity-40"
        >
          {t('videoPlayer.reset')}
        </button>
      </div>
    </>
  );
};
