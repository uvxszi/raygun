import type { StartScreen } from '@raygun/bindings';
import { Outlet, useNavigate, useRouterState } from '@tanstack/react-router';
import { useEffect, useRef } from 'react';
import type { FC } from 'react';

import { TrackActionsSheet } from '../collections/TrackActionsSheet';
import { useSyncChangedListener } from '../devices/useDeviceSync';
import { useEngineHost } from '../engine/useEngineHost';
import { useSyncLanguage } from '../i18n/useSyncLanguage';
import { useLibraryScanSync } from '../library/useLibrary';
import { useExtensionsSync } from '../extensions/useExtensions';
import { MiniPlayer } from '../player/MiniPlayer';
import { PlayerSheet } from '../player/PlayerSheet';
import { QueuePanel } from '../player/QueuePanel';
import { usePlaybackSync } from '../player/usePlayback';
import { useApplyTheme } from '../settings/useApplyTheme';
import { useSettings, useSettingsEvents } from '../settings/useSettings';
import { VideoPlayer } from '../video/VideoPlayer';
import { useVideoPlayerEvents } from '../video/videoPlayerStore';
import { useYouTubeVideoEvents } from '../video/trailerStore';
import { YouTubeOverlay } from '../video/YouTubeOverlay';
import { usePlayerLabels } from '../watch/usePlayerLabels';
import { BottomTabs } from './BottomTabs';
import { NAV_TABS } from './navTabs';
import { SideRail } from './SideRail';

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

export const AppShell: FC = () => {
  const settings = useSettings();
  useApplyTheme(settings.data?.theme, settings.data?.accent);
  useStartScreen(settings.data?.startScreen);
  useSettingsEvents();
  useSyncLanguage(settings.data?.language);
  useEngineHost();
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

  return (
    <div className="flex h-full">
      <SideRail />
      <div className="flex min-w-0 flex-1 flex-col">
        <main
          ref={mainRef}
          data-testid="page-content"
          className="flex-1 overflow-y-auto px-4 pt-[env(safe-area-inset-top)] pb-4 lg:px-10"
        >
          <Outlet />
        </main>
        {!immersive && <MiniPlayer />}
        {!immersive && <BottomTabs />}
      </div>
      <PlayerSheet />
      {!immersive && <QueuePanel />}
      <TrackActionsSheet />
      <VideoPlayer />
      <YouTubeOverlay />
      {/* Keeps scrolled content from showing through the transparent status bar on phones. */}
      <div
        aria-hidden
        className="bg-background pointer-events-none fixed inset-x-0 top-0 z-40 h-[env(safe-area-inset-top)]"
      />
    </div>
  );
};
