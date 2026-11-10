import type { FC } from 'react';

import { useIsDesktop } from '../shell/useIsDesktop';
import { useSongCaptureHost } from './songCapture';
import { useSongDownloadSync } from './useSongDownloads';
import { useYouTubeCaptureHost } from './youtubeCapture';

/**
 * Song downloads' background work in the shell, drawing nothing: download changes are listened
 * for once, for every list (M46.46), on desktop the streams Rust asks to capture are captured here
 * (M46.49), and on every platform the YouTube videos it asks to capture play in hidden players of
 * their own (M46.52).
 */
export const SongDownloadsHost: FC = () => {
  useSongDownloadSync();
  useSongCaptureHost(useIsDesktop());
  useYouTubeCaptureHost();
  return null;
};
