import { RotateCcw } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PITCH_LIMIT } from '../engine/pitchShifter';
import { sendControl, usePlayerSelect } from '../player/usePlayback';
import { DragSlider } from './DragSlider';
import type { FullSound } from './soundPrefs';

/** Tempo from half to twice, in hundredths. */
export const TEMPO_MIN = 50;
export const TEMPO_MAX = 200;
/** Tempo moves in 0.05× steps, pitch in half semitones (tenths: 5). */
const TEMPO_STEP = 5;
const PITCH_STEP = 5;

/** "1.25×". */
const tempoText = (rate: number) => `${Number(rate.toFixed(2))}×`;

/** A shift in tenths of a semitone as words: "+1.5", "−2", "0". */
const pitchText = (tenths: number) => {
  const semitones = Number((tenths / 10).toFixed(1));
  if (semitones === 0) return '0';
  return semitones > 0 ? `+${semitones}` : `−${-semitones}`;
};

type Props = {
  /** The speed, 1 = as recorded (tempo: the pitch is kept). */
  rate: number;
  onRate: (rate: number) => void;
  /** Tenths of a semitone. */
  pitch: number;
  onPitch: (tenths: number) => void;
  /** Tempo 1×, pitch 0 (and whatever else the caller puts back). */
  onReset: () => void;
  disabled?: boolean;
};

/**
 * Tempo (speed with the pitch kept) and pitch (semitones with the tempo kept), with Reset sound:
 * the music player's Sound sheet and Settings → Sound, and the video player's Effects.
 */
export const TempoPitch: FC<Props> = ({ rate, onRate, pitch, onPitch, onReset, disabled }) => {
  const { t } = useTranslation();
  const tempo = Math.round(rate * 100);
  const changed = tempo !== 100 || pitch !== 0;
  const pitchWords = t('videoPlayer.soundPitchValue', { n: pitchText(pitch) });
  return (
    <div data-testid="tempo-pitch" className="flex flex-col gap-3">
      <label className="flex flex-col gap-1 text-sm">
        <span className="flex justify-between">
          <span>{t('videoPlayer.soundTempo')}</span>
          <span className="text-primary font-semibold tabular-nums">{tempoText(rate)}</span>
        </span>
        <DragSlider
          axis="x"
          enabled={!disabled}
          label={t('videoPlayer.soundTempo')}
          valueText={tempoText(rate)}
          min={TEMPO_MIN}
          max={TEMPO_MAX}
          step={TEMPO_STEP}
          value={Math.min(TEMPO_MAX, Math.max(TEMPO_MIN, tempo))}
          onValue={(next) => onRate(next / 100)}
          onReset={() => onRate(1)}
          boxClassName="flex w-full"
          className="accent-primary w-full"
        />
      </label>
      <label className="flex flex-col gap-1 text-sm">
        <span className="flex justify-between">
          <span>{t('videoPlayer.soundPitch')}</span>
          <span className="text-primary font-semibold tabular-nums">{pitchWords}</span>
        </span>
        <DragSlider
          axis="x"
          enabled={!disabled}
          label={t('videoPlayer.soundPitch')}
          valueText={pitchWords}
          min={-PITCH_LIMIT}
          max={PITCH_LIMIT}
          step={PITCH_STEP}
          value={pitch}
          onValue={onPitch}
          onReset={() => onPitch(0)}
          boxClassName="flex w-full"
          className="accent-primary w-full"
        />
      </label>
      <button
        type="button"
        disabled={disabled || !changed}
        onClick={onReset}
        className="flex items-center gap-1.5 self-start text-sm opacity-80 enabled:hover:opacity-100 disabled:opacity-40"
      >
        <RotateCcw aria-hidden size={14} />
        {t('videoPlayer.soundReset')}
      </button>
    </div>
  );
};

type MusicProps = {
  sound: FullSound;
  change: (patch: Partial<FullSound>) => void;
  disabled?: boolean;
};

/** The music player's tempo (its speed, for songs, podcasts and books alike) and pitch. */
export const MusicTempoPitch: FC<MusicProps> = ({ sound, change, disabled }) => {
  const rate = usePlayerSelect((state) => state?.rate ?? 1);
  const setRate = (next: number) => void sendControl({ type: 'setRate', rate: next });
  return (
    <TempoPitch
      rate={rate}
      onRate={setRate}
      pitch={sound.pitch}
      onPitch={(pitch) => change({ pitch })}
      onReset={() => {
        if (rate !== 1) setRate(1);
        if (sound.pitch !== 0) change({ pitch: 0 });
      }}
      disabled={disabled}
    />
  );
};
