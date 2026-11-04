import { ArrowLeft, X } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PlayerPageScreen } from '../pages/PlayerSettingsPage';
import { withDefaults } from '../settings/playerOptions';
import type { PlayerSettingsPage } from '../settings/playerOptions';
import { SettingsSaveIssue } from '../settings/Problem';
import { VideoPlayerSettings } from '../settings/screens/VideoPlayerSettings';
import { useSettings } from '../settings/useSettings';
import { SheetAboveContext } from '../shell/sheetAbove';
import { useVideoPlayerStore } from './videoPlayerStore';

/**
 * Settings → Video player over the desktop player, its screens opened in place, so the video
 * keeps playing behind it. Changes reach the player as they're saved.
 */
export const VideoSettingsPanel: FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  const [page, setPage] = useState<PlayerSettingsPage | null>(null);
  const settings = useSettings().data;
  // What the player read when the video opened, brought up to date as rows change here (as
  // Rust's next request would: `player_prefs` in video.rs).
  const opened = useRef(settings);
  useEffect(() => {
    if (!settings || settings === opened.current) return;
    useVideoPlayerStore.setState({
      prefs: withDefaults(settings.videoPlayer),
      seekStepMs: settings.seekSeconds * 1_000,
      subtitleLanguage: settings.subtitleLanguage,
      subtitleMode:
        settings.videoPlayer.subtitleMode ?? (settings.autoSubtitles ? 'always' : 'foreign'),
    });
  }, [settings]);
  const title = page
    ? t(`settings.player.section.${page}.title`)
    : t('settings.section.video.title');
  return (
    <section
      aria-label={title}
      data-testid="video-settings-panel"
      onClick={(event) => event.stopPropagation()}
      onDoubleClick={(event) => event.stopPropagation()}
      className="bg-background text-foreground absolute inset-y-4 right-4 z-20 flex w-[min(32rem,calc(100%-2rem))] flex-col overflow-hidden rounded-2xl shadow-2xl ring-1 ring-white/10 select-text"
    >
      <header className="flex shrink-0 items-center gap-2 px-3 pt-3 pb-2">
        {page && (
          <button
            type="button"
            aria-label={t('nav.back')}
            onClick={() => setPage(null)}
            className="hover:bg-surface grid size-9 place-items-center rounded-full"
          >
            <ArrowLeft aria-hidden size={18} />
          </button>
        )}
        <h2 className="min-w-0 flex-1 truncate px-1 text-base font-semibold">{title}</h2>
        <button
          type="button"
          aria-label={t('desktopPlayer.closePanel')}
          onClick={onClose}
          className="hover:bg-surface grid size-9 place-items-center rounded-full"
        >
          <X aria-hidden size={18} />
        </button>
      </header>
      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto px-2 pb-3">
        <SheetAboveContext.Provider value>
          {page ? <PlayerPageScreen page={page} /> : <VideoPlayerSettings openPage={setPage} />}
        </SheetAboveContext.Provider>
        <SettingsSaveIssue />
      </div>
    </section>
  );
};
