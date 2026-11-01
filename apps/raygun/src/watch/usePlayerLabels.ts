import { commands } from '@raygun/bindings';
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

/** The native video player's words, in the app's language; sent again when it changes (M44). */
export const usePlayerLabels = () => {
  const { t, i18n } = useTranslation();
  useEffect(() => {
    void commands.videoSetPlayerLabels({
      episodes: t('videoPlayer.episodes'),
      sources: t('videoPlayer.sources'),
      cancel: t('videoPlayer.cancel'),
      nextEpisode: t('videoPlayer.nextEpisode'),
      nextIn: t('videoPlayer.nextIn', { seconds: '{n}' }),
      autoplayOn: t('videoPlayer.autoplayOn'),
      autoplayOff: t('videoPlayer.autoplayOff'),
      findingSource: t('videoPlayer.findingSource'),
      tryingNext: t('videoPlayer.tryingNext'),
      noSourceWorked: t('videoPlayer.noSourceWorked'),
      resolving: t('videoPlayer.resolving'),
      failed: t('videoPlayer.failed'),
      empty: t('videoPlayer.empty'),
      downloaded: t('videoPlayer.downloaded'),
      onDevice: t('videoPlayer.onDevice'),
      addons: t('videoPlayer.addons'),
      video: t('watch.video'),
    });
  }, [t, i18n.language]);
};
