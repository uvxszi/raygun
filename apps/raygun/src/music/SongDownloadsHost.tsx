import type { FC } from 'react';

import { useIsDesktop } from '../shell/useIsDesktop';
import { useSongCaptureHost } from './songCapture';
import { useSongDownloadSync } from './useSongDownloads';

/**
 * Song downloads' background work in the shell, drawing nothing: download changes are listened
 * for once, for every list (M46.46), and on desktop the streams Rust asks to capture are captured
 * here (M46.49).
 */
export const SongDownloadsHost: FC = () => {
  useSongDownloadSync();
  useSongCaptureHost(useIsDesktop());
  return null;
};
