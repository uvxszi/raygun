import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { PLAYBACK_SHORTCUTS } from './usePlaybackKeys';

/** Shortcuts that work everywhere else in the app (the readers list their own). */
const APP_SHORTCUTS = [
  ['F11', 'fullscreen'],
  ['Ctrl + scroll', 'zoom'],
  ['Alt + ←', 'navBack'],
  ['Right click', 'menu'],
] as const;

/** The keyboard shortcuts, from `?` (M46.8). */
export const ShortcutsSheet: FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  const row = (keys: string, what: string) => (
    <li key={keys} className="flex items-center justify-between gap-4 py-1.5 text-sm">
      <span>{t(`shortcuts.${what}`)}</span>
      <kbd className="bg-background text-muted rounded-md px-2 py-0.5 font-mono text-xs">
        {keys}
      </kbd>
    </li>
  );
  return (
    <Sheet title={t('shortcuts.title')} onClose={onClose}>
      <div className="flex flex-col gap-3">
        <ul className="flex flex-col">
          {PLAYBACK_SHORTCUTS.map(([keys, what]) => row(keys, what))}
        </ul>
        <ul className="border-foreground/10 flex flex-col border-t pt-2">
          {APP_SHORTCUTS.map(([keys, what]) => row(keys, what))}
        </ul>
      </div>
    </Sheet>
  );
};
