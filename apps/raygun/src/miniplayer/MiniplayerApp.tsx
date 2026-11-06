import type { FC } from 'react';

import { useApplyFonts } from '../fonts/systemFonts';
import { useSyncLanguage } from '../i18n/useSyncLanguage';
import { usePlaybackSync } from '../player/usePlayback';
import { useApplySize } from '../settings/useApplySize';
import { useApplyColors, useApplyTheme } from '../settings/useApplyTheme';
import { useSettings, useSettingsEvents } from '../settings/useSettings';
import { MiniplayerWindow } from './MiniplayerWindow';

/**
 * The miniplayer window's whole app (M46.13): the user's theme, sizes, font and language,
 * playback followed through Rust's events, and nothing else. The main window keeps the engines;
 * this one only draws and sends controls.
 */
export const MiniplayerApp: FC = () => {
  const settings = useSettings();
  useApplyTheme(settings.data?.theme, settings.data?.accent, settings.data?.accentColor);
  useApplyColors(settings.data?.colors);
  // Its own webview zooms by the interface size too (the window opens as much larger).
  useApplySize(true, settings.data?.zoom, settings.data?.textSize);
  useApplyFonts(settings.data?.fonts);
  useSyncLanguage(settings.data?.language);
  useSettingsEvents();
  usePlaybackSync();
  return <MiniplayerWindow />;
};
