import { Outlet } from '@tanstack/react-router';
import type { FC } from 'react';

import { useEngineHost } from '../engine/useEngineHost';
import { useSyncLanguage } from '../i18n/useSyncLanguage';
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

  return (
    <div className="flex h-full">
      <SideRail />
      <div className="flex min-w-0 flex-1 flex-col">
        <main
          data-testid="page-content"
          className="flex-1 overflow-y-auto px-4 pt-[max(1rem,env(safe-area-inset-top))] pb-6 lg:px-10 lg:pt-8"
        >
          <Outlet />
        </main>
        <MiniPlayer />
        <BottomTabs />
      </div>
      <PlayerSheet />
    </div>
  );
};
