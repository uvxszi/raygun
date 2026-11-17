import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { EqualizerPanel } from '../../sound/EqualizerPanel';
import { TempoPitch } from '../../sound/TempoPitch';
import { PanelHeading, PanelOption } from '../PlayerPanel';
import type { MusicEq } from '../protocol';
import { FLAT_BANDS } from './videoSound';
import type { FullVideoSound } from './videoSound';

/** The title's sound as the Effects panel changes it. */
export type VideoSoundControls = {
  /** The title's speed in hundredths: its tempo (the pitch kept while pitch correction is on). */
  speed: number;
  onSpeed: (hundredths: number) => void;
  sound: FullVideoSound;
  setSound: (next: FullVideoSound) => void;
  /** Tempo 1×, pitch 0, the music player's equalizer. */
  resetSound: () => void;
  musicEq: MusicEq | null;
};

/**
 * The Effects panel's Sound: the title's tempo and pitch, and its equalizer (its own, or the music
 * player's, which it follows until it has one).
 */
export const VideoSoundOptions: FC<{ controls: VideoSoundControls }> = ({ controls }) => {
  const { t } = useTranslation();
  const { sound, setSound, musicEq } = controls;
  return (
    <>
      <PanelHeading>{t('videoPlayer.effectsSound')}</PanelHeading>
      <div className="px-3 py-1">
        <TempoPitch
          rate={controls.speed / 100}
          onRate={(rate) => controls.onSpeed(Math.round(rate * 100))}
          pitch={sound.pitch}
          onPitch={(pitch) => setSound({ ...sound, pitch })}
          onReset={controls.resetSound}
        />
      </div>
      <PanelHeading>{t('videoPlayer.soundEqualizer')}</PanelHeading>
      <PanelOption
        toggle
        label={t('videoPlayer.soundEqualizerOwn')}
        hint={sound.ownEq ? undefined : t('videoPlayer.soundEqualizerFollows')}
        selected={sound.ownEq}
        onSelect={() =>
          setSound(
            sound.ownEq
              ? { ...sound, ownEq: false }
              : {
                  ...sound,
                  ownEq: true,
                  // It starts from what it followed.
                  eqEnabled: musicEq?.eqEnabled ?? false,
                  eqPreset: musicEq?.eqPreset ?? 'flat',
                  eqBands:
                    musicEq?.eqBands?.length === FLAT_BANDS.length ? musicEq.eqBands : FLAT_BANDS,
                  preamp: musicEq?.preamp ?? 0,
                },
          )
        }
      />
      {sound.ownEq && (
        <div className="px-3 py-2">
          <EqualizerPanel sound={sound} change={(patch) => setSound({ ...sound, ...patch })} />
        </div>
      )}
    </>
  );
};
