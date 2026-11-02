import type { FC } from 'react';

import { useSyncLanguage } from '../i18n/useSyncLanguage';
import { usePlaybackSync } from '../player/usePlayback';
import { useApplyColors, useApplyTheme } from '../settings/useApplyTheme';
import { useSettings, useSettingsEvents } from '../settings/useSettings';
import { MiniplayerWindow } from './MiniplayerWindow';

/**
 * The miniplayer window's whole app (M46.13): the user's theme and language, playback followed
 * through Rust's events, and nothing else. The main window keeps the engines; this one only
 * draws and sends controls.
 */
export const MiniplayerApp: FC = () => {
  const settings = useSettings();
  useApplyTheme(settings.data?.theme, settings.data?.accent, settings.data?.accentColor);
  useApplyColors(settings.data?.colors);
  useSyncLanguage(settings.data?.language);
  useSettingsEvents();
  usePlaybackSync();
  return <MiniplayerWindow />;
};
