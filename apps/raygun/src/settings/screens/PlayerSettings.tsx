import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PlayerViewSettings } from '../../player/PlayerViewSettings';
import { AudioQualitySettings } from '../AudioQualitySettings';
import { PLAYER_PAGES, withDefaults } from '../playerOptions';
import type { PlayerPrefs } from '../playerOptions';
import { PrefChoice, PrefGroup, PrefLink, PrefSlider, PrefSwitch } from '../prefs';
import { useSettings, useUpdateSetting } from '../useSettings';

/** Matches `VIDEO_QUALITIES` in raygun-model. */
const QUALITIES = [1080, 720, 480, 360];
/** Matches `NEXT_PROMPT_SECONDS` in raygun-model (0: at the end). */
const NEXT_PROMPT_SECONDS = [0, 5, 10, 15, 30];

/** Settings → Player: music first (quality, what happens on start), then videos and their player. */
export const PlayerSettings: FC = () => {
  const { t } = useTranslation();
  const data = useSettings().data;
  const update = useUpdateSetting();
  if (!data) return null;
  const prefs = withDefaults(data.videoPlayer);
  const setPlayer = (change: Partial<PlayerPrefs>) =>
    update.mutate({ key: 'videoPlayer', value: { ...prefs, ...change } });
  return (
    <>
      <AudioQualitySettings />
      <PrefGroup>
        <PrefSwitch
          title={t('settings.resumeOnStart')}
          checked={data.resumeOnStart ?? true}
          onChange={(value) => update.mutate({ key: 'resumeOnStart', value })}
        />
        <PrefSwitch
          title={t('settings.keepMusicPlaying')}
          checked={data.keepMusicPlaying ?? false}
          onChange={(value) => update.mutate({ key: 'keepMusicPlaying', value })}
        />
      </PrefGroup>
      <PlayerViewSettings title={t('player.view.musicTitle')} />
      <PrefGroup title={t('settings.videoPlayback')}>
        <PrefChoice<number | null>
          title={t('settings.videoQuality')}
          value={data.preferredVideoQuality}
          options={[
            { value: null, label: t('settings.videoQualityHighest') },
            ...QUALITIES.map((height) => ({
              value: height,
              label: t('settings.videoQualityUpTo', { height }),
            })),
          ]}
          onChange={(value) => update.mutate({ key: 'preferredVideoQuality', value })}
        />
        <PrefSwitch
          title={t('settings.resumeVideos')}
          checked={data.resumeVideos}
          onChange={(value) => update.mutate({ key: 'resumeVideos', value })}
        />
        <PrefSlider
          title={t('settings.watchedPercent')}
          value={data.watchedPercent}
          min={50}
          max={100}
          step={5}
          format={(value) => `${value}%`}
          onChange={(value) => update.mutate({ key: 'watchedPercent', value })}
        />
        <PrefSwitch
          title={t('settings.autoplayNext')}
          checked={data.autoplayNext}
          onChange={(value) => update.mutate({ key: 'autoplayNext', value })}
        />
        <PrefChoice<number>
          title={t('settings.nextPrompt')}
          value={data.nextPromptSeconds}
          options={NEXT_PROMPT_SECONDS.map((seconds) => ({
            value: seconds,
            label:
              seconds === 0
                ? t('settings.nextPromptAtEnd')
                : t('settings.nextPromptBefore', { count: seconds }),
          }))}
          onChange={(value) => update.mutate({ key: 'nextPromptSeconds', value })}
        />
        <PrefSwitch
          title={t('settings.player.showFailedSources')}
          checked={prefs.showFailedSources}
          onChange={(showFailedSources) => setPlayer({ showFailedSources })}
        />
        <PrefSwitch
          title={t('settings.player.showEmptySources')}
          checked={prefs.showEmptySources}
          onChange={(showEmptySources) => setPlayer({ showEmptySources })}
        />
      </PrefGroup>
      <PrefGroup title={t('settings.videoPlayer')}>
        {PLAYER_PAGES.map((page) => (
          <PrefLink
            key={page.id}
            icon={page.icon}
            title={t(`settings.player.section.${page.id}.title`)}
            to={`/settings/player/${page.id}`}
          />
        ))}
      </PrefGroup>
    </>
  );
};
