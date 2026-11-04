import type { Reverb } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefBlock, PrefChoice, PrefGroup, PrefSlider, PrefSwitch } from '../settings/prefs';
import { EqualizerPanel } from './EqualizerPanel';
import { LyricsTranslationSettings } from './LyricsTranslationSettings';
import { useSoundPrefs } from './useSound';

const REVERBS: Reverb[] = [
  'off',
  'smallRoom',
  'mediumRoom',
  'largeRoom',
  'mediumHall',
  'largeHall',
  'plate',
];
/** Spotify's volume levels: Quiet, Normal and Loud, in LUFS. */
const LEVELS = [-23, -14, -11];

/** Settings → Sound (M46.12): the equalizer, effects, normalization and crossfade. */
export const SoundSettings: FC = () => {
  const { t } = useTranslation();
  const { sound, change, loading } = useSoundPrefs();
  return (
    <>
      <PrefGroup title={t('sound.equalizer')}>
        <PrefBlock>
          <EqualizerPanel sound={sound} change={change} />
        </PrefBlock>
      </PrefGroup>
      <PrefGroup title={t('sound.effects')}>
        <PrefSlider
          title={t('sound.bassBoost')}
          value={sound.bassBoost}
          min={0}
          max={100}
          step={5}
          format={(value) => (value === 0 ? t('sound.off') : `${value}%`)}
          disabled={loading}
          onChange={(bassBoost) => change({ bassBoost })}
        />
        <PrefChoice
          title={t('sound.reverb')}
          value={sound.reverb}
          options={REVERBS.map((reverb) => ({
            value: reverb,
            label: t(`sound.reverbs.${reverb}`),
          }))}
          disabled={loading}
          onChange={(reverb) => change({ reverb })}
        />
        <PrefSlider
          title={t('sound.balance')}
          value={sound.balance}
          min={-100}
          max={100}
          step={5}
          format={(value) =>
            value === 0
              ? t('sound.centre')
              : t(value < 0 ? 'sound.left' : 'sound.right', { amount: Math.abs(value) })
          }
          disabled={loading}
          onChange={(balance) => change({ balance })}
        />
        <PrefSwitch
          title={t('sound.mono')}
          summary={t('sound.monoHint')}
          checked={sound.mono}
          disabled={loading}
          onChange={(mono) => change({ mono })}
        />
      </PrefGroup>
      <PrefGroup title={t('sound.volume')}>
        <PrefSwitch
          title={t('sound.normalize')}
          summary={t('sound.normalizeHint')}
          checked={sound.normalize}
          disabled={loading}
          onChange={(normalize) => change({ normalize })}
        />
        <PrefChoice
          title={t('sound.level')}
          value={LEVELS.includes(sound.normalizeTarget) ? sound.normalizeTarget : -14}
          options={LEVELS.map((level) => ({
            value: level,
            label: t(`sound.levels.${level}`),
          }))}
          disabled={loading || !sound.normalize}
          onChange={(normalizeTarget) => change({ normalizeTarget })}
        />
      </PrefGroup>
      <PrefGroup title={t('sound.crossfade')}>
        <PrefSlider
          title={t('sound.crossfade')}
          summary={t('sound.crossfadeHint')}
          value={sound.crossfadeMs / 1000}
          min={0}
          max={12}
          step={1}
          format={(seconds) => (seconds === 0 ? t('sound.off') : t('sound.seconds', { seconds }))}
          disabled={loading}
          onChange={(seconds) => change({ crossfadeMs: seconds * 1000 })}
        />
      </PrefGroup>
      <LyricsTranslationSettings />
    </>
  );
};
