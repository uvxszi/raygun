import { commands } from '@raygun/bindings';
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

/** The native video player's words, in the app's language; sent again when it changes (M44, M45). */
export const usePlayerLabels = () => {
  const { t, i18n } = useTranslation();
  useEffect(() => {
    // Every `videoPlayer.*` word, with `{n}` (and `{of}`) where the player puts a number or name.
    const words = t('videoPlayer', {
      returnObjects: true,
      n: '{n}',
      seconds: '{n}',
      of: '{of}',
    }) as Record<string, string>;
    void commands.videoSetPlayerLabels({ ...words, video: t('watch.video') });
  }, [t, i18n.language]);
};
