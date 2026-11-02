import type { StartScreen, WindowFrame } from '@raygun/bindings';
import { Outlet, useNavigate, useRouterState } from '@tanstack/react-router';
import { MotionConfig } from 'motion/react';
import { useCallback, useEffect, useRef, useState } from 'react';
import type { FC } from 'react';

import { TrackActionsSheet } from '../collections/TrackActionsSheet';
import { useSyncChangedListener } from '../devices/useDeviceSync';
import { useEngineHost } from '../engine/useEngineHost';
import { useSpeechHost } from '../engine/useSpeechHost';
import { useSyncLanguage } from '../i18n/useSyncLanguage';
import { useLibraryScanSync } from '../library/useLibrary';
import { useExtensionsSync } from '../extensions/useExtensions';
import { MiniPlayer } from '../player/MiniPlayer';
import { PlayerSheet } from '../player/PlayerSheet';
import { ShortcutsSheet } from '../player/ShortcutsSheet';
import { SidePanel } from '../player/SidePanel';
import { usePlaybackKeys } from '../player/usePlaybackKeys';
import { useMediaSession } from '../engine/useMediaSession';
import { usePlaybackSync } from '../player/usePlayback';
import { useApplyColors, useApplyFeel, useApplyTheme } from '../settings/useApplyTheme';
import { useSettings, useSettingsEvents } from '../settings/useSettings';
import { VideoPlayer } from '../video/VideoPlayer';
import { useVideoPlayerEvents } from '../video/videoPlayerStore';
import { useYouTubeVideoEvents } from '../video/trailerStore';
import { YouTubeOverlay } from '../video/YouTubeOverlay';
import { usePlayerLabels } from '../watch/usePlayerLabels';
import { AppBar } from './AppBar';
import { setAppZoom, setWindowDecorations, useWindowState } from './appWindow';
import { BottomTabs } from './BottomTabs';
import { ContextMenuHost } from './ContextMenu';
import { HoverLabel } from './HoverLabel';
import { useHorizontalWheel } from './useHorizontalWheel';
import { NAV_TABS } from './navTabs';
import { toggleWindowFullscreen } from './useWindowFullscreen';
import { SideRail } from './SideRail';
import { useIsDesktop } from './useIsDesktop';

/** Opens the start screen chosen in Settings (any tab, M46.5), once, when the app starts on Home. */
const useStartScreen = (screen: StartScreen | undefined) => {
  const navigate = useNavigate();
  const done = useRef(false);
  const path = useRouterState({ select: (state) => state.location.pathname });
  useEffect(() => {
    if (done.current || !screen) return;
    done.current = true;
    if (screen !== 'home' && path === '/') {
      const tab = NAV_TABS[screen];
      void navigate({ to: tab.to, search: tab.search, replace: true });
    }
  }, [screen, path, navigate]);
};

/** F11 turns the window's full screen on and off, everywhere (M46.7). */
const useFullscreenKey = () => {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'F11') return;
      event.preventDefault();
      void toggleWindowFullscreen();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
};

/** The desktop window's frame and zoom, from Settings (M46.9). */
const useApplyWindow = (desktop: boolean, frame?: WindowFrame, zoom?: number) => {
  useEffect(() => {
    if (desktop && frame) void setWindowDecorations(frame === 'system');
  }, [desktop, frame]);
  useEffect(() => {
    if (desktop && zoom) void setAppZoom(zoom / 100);
  }, [desktop, zoom]);
};

export const AppShell: FC = () => {
  const settings = useSettings();
  useApplyTheme(settings.data?.theme, settings.data?.accent, settings.data?.accentColor);
  useApplyFeel(settings.data?.scrollbars, settings.data?.motion);
  useApplyColors(settings.data?.colors);
  useStartScreen(settings.data?.startScreen);
  useSettingsEvents();
  useSyncLanguage(settings.data?.language);
  const desktop = useIsDesktop();
  const { fullscreen } = useWindowState();
  useApplyWindow(desktop, settings.data?.windowFrame, settings.data?.zoom);
  useEngineHost();
  useSpeechHost();
  useFullscreenKey();
  useHorizontalWheel();
  useMediaSession();
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const openShortcuts = useCallback(() => setShortcutsOpen(true), []);
  usePlaybackSync();
  useLibraryScanSync();
  useExtensionsSync();
  useSyncChangedListener();
  usePlayerLabels();
  useVideoPlayerEvents();
  useYouTubeVideoEvents();

  // Pages scroll inside <main>, not the window, so start each page at the top.
  const mainRef = useRef<HTMLElement>(null);
  const href = useRouterState({ select: (state) => state.location.href });
  // The readers take the whole screen.
  const immersive = useRouterState({
    select: (state) => ['/manga/read', '/novel/read'].includes(state.location.pathname),
  });
  useEffect(() => {
    mainRef.current?.scrollTo?.(0, 0);
  }, [href]);
  // Playback keys everywhere but the readers, whose keys turn pages (M46.8).
  usePlaybackKeys(openShortcuts, !immersive);

  return (
    <MotionConfig reducedMotion={settings.data?.motion === 'off' ? 'always' : 'user'}>
      <div className="flex h-full flex-col">
        {desktop && !fullscreen && (
          <AppBar
            frame={settings.data?.windowFrame ?? 'app'}
            onShortcuts={openShortcuts}
            immersive={immersive}
          />
        )}
        <div className="flex min-h-0 flex-1">
          <SideRail />
          <div className="flex min-w-0 flex-1 flex-col">
            <div className="flex min-h-0 flex-1">
              <main
                ref={mainRef}
                data-testid="page-content"
                className="min-w-0 flex-1 overflow-y-auto px-4 pt-[env(safe-area-inset-top)] pb-4 lg:px-10"
              >
                <Outlet />
              </main>
              {!immersive && <SidePanel />}
            </div>
            {!immersive && <MiniPlayer />}
            {!immersive && <BottomTabs />}
          </div>
        </div>
        <PlayerSheet />
        <TrackActionsSheet />
        <VideoPlayer />
        <YouTubeOverlay />
        <HoverLabel />
        <ContextMenuHost />
        {shortcutsOpen && <ShortcutsSheet onClose={() => setShortcutsOpen(false)} />}
        {/* Keeps scrolled content from showing through the transparent status bar on phones. */}
        <div
          aria-hidden
          className="bg-background pointer-events-none fixed inset-x-0 top-0 z-40 h-[env(safe-area-inset-top)]"
        />
      </div>
    </MotionConfig>
  );
};
