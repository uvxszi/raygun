import type { VideoResolution } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { shownOn, useIsDesktop } from '../../shell/useIsDesktop';
import { PLAYER_PAGES, playerPagePath, withDefaults } from '../playerOptions';
import type { PlayerPrefs, PlayerSettingsPage } from '../playerOptions';
import { PrefAction, PrefChoice, PrefGroup, PrefLink, PrefSlider, PrefSwitch } from '../prefs';
import { useSettings, useUpdateSetting } from '../useSettings';

/** Matches `VideoResolution::ALL` in raygun-model. */
const RESOLUTIONS: VideoResolution[] = [
  'highest',
  '2160p',
  '1440p',
  '1080p',
  '720p',
  '480p',
  '360p',
];
/** Matches `NEXT_PROMPT_SECONDS` in raygun-model (0: at the end). */
const NEXT_PROMPT_SECONDS = [0, 5, 10, 15, 30];

type Props = {
  /** Opens one of the player's screens in place (the player's own panel) instead of routing. */
  openPage?: (page: PlayerSettingsPage) => void;
};

/** Settings → Video player: resolution and what happens around videos, then the player's screens. */
export const VideoPlayerSettings: FC<Props> = ({ openPage }) => {
  const { t } = useTranslation();
  const data = useSettings().data;
  const update = useUpdateSetting();
  const desktop = useIsDesktop();
  if (!data) return null;
  const prefs = withDefaults(data.videoPlayer);
  const setPlayer = (change: Partial<PlayerPrefs>) =>
    update.mutate({ key: 'videoPlayer', value: { ...prefs, ...change } });
  return (
    <>
      <PrefGroup title={t('settings.videoPlayback')}>
        <PrefChoice<VideoResolution>
          title={t('settings.videoQuality')}
          value={prefs.preferredResolution}
          options={RESOLUTIONS.map((resolution) => ({
            value: resolution,
            label:
              resolution === 'highest'
                ? t('settings.videoQualityHighest')
                : resolution === '2160p'
                  ? '2160p (4K)'
                  : resolution,
          }))}
          onChange={(preferredResolution) => setPlayer({ preferredResolution })}
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
        {PLAYER_PAGES.filter((page) => shownOn(page.only, desktop)).map((page) => {
          const title = t(`settings.player.section.${page.id}.title`);
          return openPage ? (
            <PrefAction
              key={page.id}
              icon={page.icon}
              title={title}
              onClick={() => openPage(page.id)}
            />
          ) : (
            <PrefLink key={page.id} icon={page.icon} title={title} to={playerPagePath(page.id)} />
          );
        })}
      </PrefGroup>
    </>
  );
};
