import type { SegmentAction, SponsorBlockPrefs, YouTubePrefs } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefChoice, PrefGroup, PrefInfo, PrefSwitch } from '../settings/prefs';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { SEGMENT_CATEGORIES, SPONSORBLOCK_DEFAULTS, YOUTUBE_DEFAULTS } from './youtubeDefaults';

const ACTIONS: SegmentAction[] = ['skip', 'show', 'off'];

/**
 * Settings → Music player's YouTube groups: SponsorBlock (each kind of part skipped, only marked
 * on the seek bar, or left alone; downloads without non-music parts) and the audio track of
 * videos with dubbed ones. They apply to YouTube's videos everywhere in Raygun.
 */
export const YouTubeSettings: FC = () => {
  const { t } = useTranslation();
  const data = useSettings().data;
  const update = useUpdateSetting();
  if (!data) return null;
  const block: Required<SponsorBlockPrefs> = {
    ...SPONSORBLOCK_DEFAULTS,
    ...data.youtube?.sponsorBlock,
  };
  const youtube: Required<YouTubePrefs> = {
    ...YOUTUBE_DEFAULTS,
    ...data.youtube,
    sponsorBlock: block,
  };
  const set = (change: Partial<YouTubePrefs>) =>
    update.mutate({ key: 'youtube', value: { ...youtube, ...change } });
  const setBlock = (change: Partial<SponsorBlockPrefs>) =>
    set({ sponsorBlock: { ...block, ...change } });
  return (
    <>
      <PrefGroup
        id="sponsorblock"
        title={t('sponsorBlock.title')}
        summary={t('sponsorBlock.summary')}
      >
        <PrefSwitch
          title={t('sponsorBlock.enabled')}
          checked={block.enabled}
          onChange={(enabled) => setBlock({ enabled })}
        />
        {SEGMENT_CATEGORIES.map((category) => (
          <PrefChoice<SegmentAction>
            key={category}
            title={t(`sponsorBlock.categories.${category}`)}
            value={block[category]}
            disabled={!block.enabled}
            options={ACTIONS.map((action) => ({
              value: action,
              label: t(`sponsorBlock.actions.${action}`),
            }))}
            onChange={(action) => setBlock({ [category]: action })}
          />
        ))}
        <PrefSwitch
          title={t('sponsorBlock.trimDownloads')}
          summary={t('sponsorBlock.trimDownloadsHint')}
          checked={block.trimDownloads}
          disabled={!block.enabled}
          onChange={(trimDownloads) => setBlock({ trimDownloads })}
        />
        <PrefInfo title={t('sponsorBlock.credit')} summary={t('sponsorBlock.creditHint')} />
      </PrefGroup>
      <PrefGroup id="youtube" title={t('sponsorBlock.youtube')}>
        <PrefSwitch
          title={t('sponsorBlock.originalAudio')}
          summary={t('sponsorBlock.originalAudioHint')}
          checked={youtube.originalAudio}
          onChange={(originalAudio) => set({ originalAudio })}
        />
      </PrefGroup>
    </>
  );
};
