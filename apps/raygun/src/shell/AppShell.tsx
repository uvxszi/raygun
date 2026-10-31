import { Outlet, useRouterState } from '@tanstack/react-router';
import { useEffect, useRef } from 'react';
import type { FC } from 'react';

import { useEngineHost } from '../engine/useEngineHost';
import { useSyncLanguage } from '../i18n/useSyncLanguage';
import { useLibraryScanSync } from '../library/useLibrary';
import { useExtensionsSync } from '../extensions/useExtensions';
import { MiniPlayer } from '../player/MiniPlayer';
import { PlayerSheet } from '../player/PlayerSheet';
import { usePlaybackSync } from '../player/usePlayback';
import { useApplyTheme } from '../settings/useApplyTheme';
import { useSettings } from '../settings/useSettings';
import { BottomTabs } from './BottomTabs';
import { SideRail } from './SideRail';

export const AppShell: FC = () => {
  const settings = useSettings();
  useApplyTheme(settings.data?.theme);
  useSyncLanguage(settings.data?.language);
  useEngineHost();
  usePlaybackSync();
  useLibraryScanSync();
  useExtensionsSync();

  // Pages scroll inside <main>, not the window, so start each page at the top.
  const mainRef = useRef<HTMLElement>(null);
  const href = useRouterState({ select: (state) => state.location.href });
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
          className="flex-1 overflow-y-auto px-4 pt-[max(1rem,env(safe-area-inset-top))] pb-6 lg:px-10 lg:pt-8"
        >
          <Outlet />
        </main>
        <MiniPlayer />
        <BottomTabs />
      </div>
      <PlayerSheet />
      {/* Keeps scrolled content from showing through the transparent status bar on phones. */}
      <div
        aria-hidden
        className="bg-background pointer-events-none fixed inset-x-0 top-0 z-40 h-[env(safe-area-inset-top)]"
      />
    </div>
  );
};
