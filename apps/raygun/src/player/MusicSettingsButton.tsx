import { Settings } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SettingsSaveIssue } from '../settings/Problem';
import { MusicPlayerSettings } from '../settings/screens/MusicPlayerSettings';
import { Sheet } from '../shell/Sheet';

/** Settings → Music player as a sheet over the player, from the full player and the side panel. */
export const MusicSettingsButton: FC<{ className?: string }> = ({ className = 'size-9' }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const title = t('settings.section.music.title');
  return (
    <>
      <button
        type="button"
        aria-label={t('settings.playerSettings')}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen(true)}
        className={`text-muted hover:text-foreground grid place-items-center rounded-full ${className}`}
      >
        <Settings aria-hidden size={20} />
      </button>
      {open && (
        <Sheet title={title} onClose={() => setOpen(false)}>
          <div data-testid="music-settings-sheet" className="-mx-3 flex flex-col">
            <MusicPlayerSettings />
            <SettingsSaveIssue />
          </div>
        </Sheet>
      )}
    </>
  );
};
