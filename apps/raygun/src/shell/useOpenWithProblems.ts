import { events } from '@raygun/bindings';
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { showToast } from './toastStore';

/**
 * Says when something another app opened with Raygun or shared to it didn't play: not a video,
 * song or playlist, a file Raygun couldn't read, or a playlist with nothing playable in it. What
 * did play opens its player (or Search, for words and links) from Rust.
 */
export const useOpenWithProblems = () => {
  const { t } = useTranslation();
  useEffect(() => {
    const unlisten = events.openWithEvent.listen(({ payload }) => {
      showToast(t(`openWith.${payload.kind}`, { name: payload.name }));
    });
    return () => void unlisten.then((stop) => stop());
  }, [t]);
};
