import { commands } from '@raygun/bindings';
import type { StartScreen, WindowFrame } from '@raygun/bindings';
import { Outlet, useNavigate, useRouterState } from '@tanstack/react-router';
import { MotionConfig, MotionGlobalConfig } from 'motion/react';
import { useCallback, useEffect, useRef, useState } from 'react';
import type { CSSProperties, FC } from 'react';

import { TrackActionsSheet } from '../collections/TrackActionsSheet';
import { SongEditSheet } from '../sound/SongEditSheet';
import { CoverSheet } from '../music/CoverSheet';
import { MenuSheets } from '../music/MenuSheets';
import { SourcePickerSheet } from '../music/SourcePickerSheet';
import { CollectionSheets } from '../collections/CollectionSheetsHost';
import { SongDownloadsHost } from '../music/SongDownloadsHost';
import { useEngineHost } from '../engine/useEngineHost';
import { useSpeechHost } from '../engine/useSpeechHost';
import { useSyncLanguage } from '../i18n/useSyncLanguage';
import { useLibraryScanSync } from '../library/useLibrary';
import { useExtensionsSync } from '../extensions/useExtensions';
import { MiniPlayer } from '../player/MiniPlayer';
import { PlayerSheet } from '../player/PlayerSheet';
import { usePlayerUiStore } from '../player/playerUiStore';
import { ShortcutsSheet } from '../player/ShortcutsSheet';
import { SidePanel } from '../player/SidePanel';
import { usePlaybackKeys } from '../player/usePlaybackKeys';
import { useMediaSession } from '../engine/useMediaSession';
import { usePlaybackSync } from '../player/usePlayback';
import { useApplySize } from '../settings/useApplySize';
import { useIncognitoSync } from '../privacy/useIncognito';
import { useApplyColors, useApplyFeel, useApplyTheme } from '../settings/useApplyTheme';
import { useSettings, useSettingsEvents } from '../settings/useSettings';
import { VideoPlayer } from '../video/VideoPlayer';
import { useVideoPlayerEvents } from '../video/videoPlayerStore';
import { useYouTubeVideoEvents } from '../video/trailerStore';
import { YouTubeOverlay } from '../video/YouTubeOverlay';
import { usePlayerLabels } from '../watch/usePlayerLabels';
import { AppBar } from './AppBar';
import { setWindowDecorations, useWindowState } from './appWindow';
import { BottomTabs } from './BottomTabs';
import { ConnectPrompt } from '../connect/ConnectPrompt';
import { PermissionPromptHost } from '../permissions/PermissionPrompt';
import { useNotificationRoutes } from '../permissions/useNotificationRoutes';
import { ContextMenuHost } from './ContextMenu';
import { ToastHost } from './Toast';
import { HoverLabel } from './HoverLabel';
import { useHorizontalWheel } from './useHorizontalWheel';
import { NAV_TABS } from './navTabs';
import { toggleWindowFullscreen } from './useWindowFullscreen';
import { SideRail } from './SideRail';
import { useIsDesktop } from './useIsDesktop';
import { useLiveInvalidation } from './useLiveInvalidation';
import { useRailStamps } from './useRail';
import { movesFreely, useDeviceStateSync } from './deviceState';

/**
 * With motion off, motion's animations (the player sheet, the bar's swipe) jump to their end at
 * once, so nothing waits on one; the value from before comes back after.
 */
const useSkipAnimations = (skip: boolean) => {
  useEffect(() => {
    if (!skip) return;
    const before = MotionGlobalConfig.skipAnimations;
    MotionGlobalConfig.skipAnimations = true;
    return () => {
      MotionGlobalConfig.skipAnimations = before;
    };
  }, [skip]);
};

/** Engines start listening right after the shell mounts; the last queue comes back then. */
const RESTORE_DELAY_MS = 600;

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

/** The desktop window's frame, from Settings (M46.9). */
const useApplyWindow = (desktop: boolean, frame?: WindowFrame) => {
  useEffect(() => {
    if (desktop && frame) void setWindowDecorations(frame === 'system');
  }, [desktop, frame]);
};

export const AppShell: FC = () => {
  const settings = useSettings();
  useApplyTheme(settings.data?.theme, settings.data?.accent, settings.data?.accentColor);
  useDeviceStateSync();
  const motion = useApplyFeel(settings.data?.scrollbars, settings.data?.motion);
  useSkipAnimations(motion === 'off');
  useApplyColors(settings.data?.colors);
  useStartScreen(settings.data?.startScreen);
  useSettingsEvents();
  useSyncLanguage(settings.data?.language);
  const desktop = useIsDesktop();
  const { fullscreen } = useWindowState();
  useApplyWindow(desktop, settings.data?.windowFrame);
  // Sizes from the screen, then Interface size (M46.48).
  useApplySize(desktop, settings.data?.zoom);
  useEngineHost();
  // The sidebar is the desktop's (M46.29); incognito (M47) leaves its stamps alone.
  const incognito = useIncognitoSync();
  useRailStamps(desktop && incognito === false);
  useSpeechHost();
  useFullscreenKey();
  useHorizontalWheel();
  useMediaSession();
  const sheetOpen = usePlayerUiStore((store) => store.sheetOpen);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const openShortcuts = useCallback(() => setShortcutsOpen(true), []);
  usePlaybackSync();
  useEffect(() => {
    // What played when Raygun closed comes back, paused (M46.16), once the engines listen.
    const timer = setTimeout(() => void commands.playbackRestore(), RESTORE_DELAY_MS);
    return () => clearTimeout(timer);
  }, []);
  useLibraryScanSync();
  useExtensionsSync();
  useLiveInvalidation();
  usePlayerLabels();
  useVideoPlayerEvents();
  useYouTubeVideoEvents();
  useNotificationRoutes();

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
    <MotionConfig reducedMotion={movesFreely(motion) ? 'never' : 'always'}>
      <div
        className="flex h-full flex-col"
        // Full-window screens (the full player) start below the desktop bar (M46.9).
        style={{ '--app-bar': desktop && !fullscreen ? '4rem' : '0px' } as CSSProperties}
      >
        {desktop && !fullscreen && (
          <AppBar
            frame={settings.data?.windowFrame ?? 'app'}
            onShortcuts={openShortcuts}
            immersive={immersive}
          />
        )}
        {/* Under the full player nothing animates (styles.css `[data-covered]`). */}
        <div className="flex min-h-0 flex-1" data-covered={sheetOpen ? '' : undefined}>
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
        <SongEditSheet />
        <CoverSheet />
        <SourcePickerSheet />
        <CollectionSheets />
        <SongDownloadsHost />
        <VideoPlayer />
        <YouTubeOverlay />
        <HoverLabel />
        <MenuSheets />
        <ContextMenuHost />
        <ToastHost />
        <ConnectPrompt />
        <PermissionPromptHost />
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
